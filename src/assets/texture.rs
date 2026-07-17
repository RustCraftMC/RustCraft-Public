//! Dynamic texture atlas for block, item, particle, and special block textures.
//!
//! Textures keep their resource-pack resolution. Rectangles are packed at runtime
//! and each atlas index publishes its own UV region, so a 128x128 texture is never
//! reduced to Minecraft's vanilla 16x16 resolution.

use image::{DynamicImage, GenericImageView, RgbaImage};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use super::resolver::AssetResolver;

pub const TILE_SIZE: u32 = 16;
pub const ATLAS_TILES_X: u32 = 64;
pub const ATLAS_TILES_Y: u32 = 64;
pub const ATLAS_TILES: u32 = ATLAS_TILES_X;
pub const ATLAS_WIDTH: u32 = TILE_SIZE * ATLAS_TILES_X;
pub const ATLAS_HEIGHT: u32 = TILE_SIZE * ATLAS_TILES_Y;
pub const MAX_TILES: usize = (ATLAS_TILES_X * ATLAS_TILES_Y) as usize;
pub const MISSING_TILE_INDEX: usize = MAX_TILES - 2;
pub const WHITE_TILE_INDEX: usize = MAX_TILES - 1;

const CONTENT_TILE_LIMIT: usize = MISSING_TILE_INDEX;
const ATLAS_PADDING: u32 = 1;
const MAX_ATLAS_DIMENSION: u32 = 16_384;

/// Cap texture dimensions to prevent a single resource-pack texture from
/// ballooning the atlas. 256 px covers standard high-res packs; anything
/// larger provides negligible visual gain in a block game.
const MAX_TEX_DIM: u32 = 256;
const UV_COMPONENT_COUNT: usize = MAX_TILES * 4;

static TEXTURE_MAP: std::sync::Mutex<Option<HashMap<String, usize>>> = std::sync::Mutex::new(None);
static ATLAS_UV_BITS: [AtomicU32; UV_COMPONENT_COUNT] =
    [const { AtomicU32::new(0) }; UV_COMPONENT_COUNT];

/// Publish texture names and packed UVs for mesh and UI code.
pub fn init_texture_map(atlas: &TextureAtlas) {
    if let Ok(mut map) = TEXTURE_MAP.lock() {
        *map = Some(atlas.name_to_index.clone());
    }

    let missing = atlas
        .regions
        .get(MISSING_TILE_INDEX)
        .copied()
        .unwrap_or_default();
    for index in 0..MAX_TILES {
        let region = atlas.regions.get(index).copied().unwrap_or(missing);
        let uv = region.uv_rect(atlas.width, atlas.height);
        let offset = index * 4;
        for component in 0..4 {
            ATLAS_UV_BITS[offset + component].store(uv[component].to_bits(), Ordering::Relaxed);
        }
    }
}

/// Look up an atlas index by texture name (for example `stone` or `blocks/stone`).
pub fn tex_idx(name: &str) -> usize {
    TEXTURE_MAP
        .lock()
        .ok()
        .and_then(|map| {
            map.as_ref().and_then(|map| {
                map.get(name)
                    .or_else(|| map.get(name.strip_prefix("blocks/").unwrap_or(name)))
                    .or_else(|| map.get("__missing"))
                    .copied()
            })
        })
        .unwrap_or(MISSING_TILE_INDEX)
}

#[derive(Deserialize, Debug)]
pub struct AnimationMeta {
    pub animation: AnimationData,
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct AnimationData {
    pub frametime: Option<u32>,
    pub interpolate: Option<bool>,
    pub frames: Option<Vec<serde_json::Value>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct AnimatedTexture {
    pub frames: Vec<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub frametime: u32,
    pub interpolate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasAnimationUpload {
    pub pixel_x: u32,
    pub pixel_y: u32,
    pub width: u32,
    pub height: u32,
    pub buffer_offset: u64,
}

#[derive(Clone, Debug)]
pub struct TileInfo {
    pub texture_name: String,
    pub is_animated: bool,
    pub frame_count: u32,
}

pub type UvRect = [f32; 4];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl AtlasRegion {
    fn uv_rect(self, atlas_width: u32, atlas_height: u32) -> UvRect {
        if self.width == 0 || self.height == 0 || atlas_width == 0 || atlas_height == 0 {
            return [0.0; 4];
        }
        [
            self.x as f32 / atlas_width as f32,
            self.y as f32 / atlas_height as f32,
            (self.x + self.width) as f32 / atlas_width as f32,
            (self.y + self.height) as f32 / atlas_height as f32,
        ]
    }
}

pub struct TextureAtlas {
    pub pixels: Vec<u8>,
    pub animated: HashMap<usize, AnimatedTexture>,
    pub tiles: HashMap<usize, TileInfo>,
    pub name_to_index: HashMap<String, usize>,
    pub regions: Vec<AtlasRegion>,
    pub tick: u32,
    pub width: u32,
    pub height: u32,
}

struct PendingTexture {
    index: usize,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl TextureAtlas {
    pub fn load_with_resolver(resolver: &mut AssetResolver) -> Self {
        let mut pending = Vec::new();
        let mut animated = HashMap::new();
        let mut tiles = HashMap::new();
        let mut name_to_index = HashMap::new();
        let mut next_index = 0usize;

        let mut item_texture_names = resolver.list_textures("items");
        item_texture_names.sort_unstable();
        for tex_name in item_texture_names {
            if next_index >= CONTENT_TILE_LIMIT {
                break;
            }
            let path = format!("minecraft/textures/items/{tex_name}");
            let Some(image) = decode_resolved_image(resolver, &path) else {
                continue;
            };
            let name = strip_png_ext(&tex_name);
            add_pending_texture(
                &mut pending,
                &mut tiles,
                &mut name_to_index,
                next_index,
                name,
                image.to_rgba8(),
                false,
                1,
            );
            next_index += 1;
        }

        let mut block_texture_names = resolver.list_textures("blocks");
        block_texture_names.sort_unstable();
        for tex_name in block_texture_names {
            if next_index >= CONTENT_TILE_LIMIT {
                log::warn!(
                    "texture atlas full at {} tiles; skipping remaining block textures",
                    MAX_TILES
                );
                break;
            }
            let path = format!("minecraft/textures/blocks/{tex_name}");
            let meta = resolver
                .read_string(&format!("{path}.mcmeta"))
                .and_then(|json| serde_json::from_str::<AnimationMeta>(&json).ok());
            let name = strip_png_ext(&tex_name);

            if let Some(meta) = meta {
                // Animated textures (e.g. 128x4096 water_flow) must skip the
                // global MAX_TEX_DIM downscale applied to the sprite sheet.
                // Extract frames first from the full-resolution source, then
                // cap each individual frame so it stays at a visible size
                // (e.g. 64x64 instead of 8x8).
                let Some(source_image) = decode_resolved_image_full(resolver, &path) else {
                    continue;
                };
                let (source_width, source_height) = source_image.dimensions();
                let mut frame_width = meta.animation.width.unwrap_or(source_width).max(1);
                let mut frame_height = meta.animation.height.unwrap_or(frame_width).max(1);
                let columns = (source_width / frame_width).max(1);
                let rows = (source_height / frame_height).max(1);
                let mut frames = Vec::with_capacity((columns * rows) as usize);
                for row in 0..rows {
                    for column in 0..columns {
                        let frame = source_image.crop_imm(
                            column * frame_width,
                            row * frame_height,
                            frame_width.min(source_width - column * frame_width),
                            frame_height.min(source_height - row * frame_height),
                        );
                        let capped = if frame.width() > MAX_TEX_DIM || frame.height() > MAX_TEX_DIM
                        {
                            let ratio =
                                MAX_TEX_DIM as f64 / (frame.width().max(frame.height()) as f64);
                            let nw = (frame.width() as f64 * ratio).ceil() as u32;
                            let nh = (frame.height() as f64 * ratio).ceil() as u32;
                            frame.resize_exact(nw, nh, image::imageops::FilterType::Lanczos3)
                        } else {
                            frame
                        };
                        frames.push(capped.to_rgba8().into_raw());
                    }
                }
                // Pin logical dimensions to the first (and largest) capped frame.
                if let Some(first) = &frames.first() {
                    frame_width = ((first.len() / 4) as f64).sqrt().ceil() as u32;
                    frame_height = frame_width;
                }
                if let Some(first) = frames.first() {
                    pending.push(PendingTexture {
                        index: next_index,
                        pixels: first.clone(),
                        width: frame_width,
                        height: frame_height,
                    });
                }
                let frame_count = frames.len() as u32;
                animated.insert(
                    next_index,
                    AnimatedTexture {
                        frames,
                        width: frame_width,
                        height: frame_height,
                        frametime: meta.animation.frametime.unwrap_or(1).max(1),
                        interpolate: meta.animation.interpolate.unwrap_or(false),
                    },
                );
                tiles.insert(
                    next_index,
                    TileInfo {
                        texture_name: name.clone(),
                        is_animated: true,
                        frame_count,
                    },
                );
                name_to_index.insert(name, next_index);
            } else {
                let Some(image) = decode_resolved_image(resolver, &path) else {
                    continue;
                };
                add_pending_texture(
                    &mut pending,
                    &mut tiles,
                    &mut name_to_index,
                    next_index,
                    name,
                    image.to_rgba8(),
                    false,
                    1,
                );
            }
            next_index += 1;
        }

        // EntityRenderer.renderRainSnow uses environment/rain.png. Include it
        // in the world atlas so weather particles share the normal render path.
        if next_index < CONTENT_TILE_LIMIT {
            if let Some(image) =
                decode_resolved_image(resolver, "minecraft/textures/environment/rain.png")
            {
                add_pending_texture(
                    &mut pending,
                    &mut tiles,
                    &mut name_to_index,
                    next_index,
                    "environment/rain".to_string(),
                    image.to_rgba8(),
                    false,
                    1,
                );
                next_index += 1;
            }
        }

        // Chest textures use a 64x64 vanilla layout. Scale every UV island by
        // the resource pack's actual dimensions before placing it in a 16x16
        // logical canvas so the existing ModelChest face UVs remain correct.
        for (variant, relative_path) in [
            ("normal", "chest/normal.png"),
            ("trapped", "chest/trapped.png"),
            ("ender", "chest/ender.png"),
        ] {
            let path = format!("minecraft/textures/entity/{relative_path}");
            let Some(image) = decode_resolved_image(resolver, &path) else {
                continue;
            };
            for (part, x, y, width, height) in chest_face_regions() {
                if next_index >= CONTENT_TILE_LIMIT {
                    break;
                }
                let face = extract_scaled_chest_face(&image, x, y, width, height);
                let name = format!("chest_{variant}_{part}");
                add_pending_texture(
                    &mut pending,
                    &mut tiles,
                    &mut name_to_index,
                    next_index,
                    name,
                    face,
                    false,
                    1,
                );
                next_index += 1;
            }
        }

        for (variant, relative_path) in [
            ("normal", "chest/normal_double.png"),
            ("trapped", "chest/trapped_double.png"),
        ] {
            let path = format!("minecraft/textures/entity/{relative_path}");
            let Some(image) = decode_resolved_image(resolver, &path) else {
                continue;
            };
            for (part, texture_x, texture_y, width, height, depth) in [
                ("lid", 0, 0, 30, 5, 14),
                ("body", 0, 19, 30, 10, 14),
                ("knob", 0, 0, 2, 4, 1),
            ] {
                for (face, x, y, face_width, face_height) in
                    model_box_face_regions(texture_x, texture_y, width, height, depth)
                {
                    if next_index >= CONTENT_TILE_LIMIT {
                        break;
                    }
                    add_pending_texture(
                        &mut pending,
                        &mut tiles,
                        &mut name_to_index,
                        next_index,
                        format!("chest_{variant}_double_{part}_{face}"),
                        extract_scaled_model_face(&image, x, y, face_width, face_height, 128, 64),
                        false,
                        1,
                    );
                    next_index += 1;
                }
            }
        }

        // TileEntitySignRenderer binds entity/sign.png. ModelSign uses two
        // ModelBox instances, so preserve their exact per-face UV islands
        // instead of falling back to the plank block texture.
        if let Some(image) = decode_resolved_image(resolver, "minecraft/textures/entity/sign.png") {
            for (part, texture_x, texture_y, width, height, depth) in
                [("board", 0, 0, 24, 12, 2), ("stick", 0, 14, 2, 14, 2)]
            {
                for (face, x, y, face_width, face_height) in
                    model_box_face_regions(texture_x, texture_y, width, height, depth)
                {
                    if next_index >= CONTENT_TILE_LIMIT {
                        break;
                    }
                    let face_image =
                        extract_scaled_model_face(&image, x, y, face_width, face_height, 64, 32);
                    add_pending_texture(
                        &mut pending,
                        &mut tiles,
                        &mut name_to_index,
                        next_index,
                        format!("sign_{part}_{face}"),
                        face_image,
                        false,
                        1,
                    );
                    next_index += 1;
                }
            }
        }

        if let Some(image) =
            decode_resolved_image(resolver, "minecraft/textures/particle/particles.png")
        {
            let (width, height) = image.dimensions();
            if width % 16 == 0 && height % 16 == 0 {
                let sprite_width = width / 16;
                let sprite_height = height / 16;
                for row in 0..16 {
                    for column in 0..16 {
                        if next_index >= CONTENT_TILE_LIMIT {
                            break;
                        }
                        let sprite = image
                            .crop_imm(
                                column * sprite_width,
                                row * sprite_height,
                                sprite_width,
                                sprite_height,
                            )
                            .to_rgba8();
                        add_pending_texture(
                            &mut pending,
                            &mut tiles,
                            &mut name_to_index,
                            next_index,
                            format!("particle_{}", row * 16 + column),
                            sprite,
                            false,
                            1,
                        );
                        next_index += 1;
                    }
                }
                log::info!("loaded 256 particle sprites at {sprite_width}x{sprite_height}");
            }
        }

        if let Some(image) =
            decode_resolved_image(resolver, "minecraft/textures/entity/explosion.png")
        {
            let (width, height) = image.dimensions();
            if width == height && width % 4 == 0 {
                let frame_size = width / 4;
                for frame in 0..16u32 {
                    if next_index >= CONTENT_TILE_LIMIT {
                        break;
                    }
                    let frame_image = image
                        .crop_imm(
                            frame % 4 * frame_size,
                            frame / 4 * frame_size,
                            frame_size,
                            frame_size,
                        )
                        .to_rgba8();
                    add_pending_texture(
                        &mut pending,
                        &mut tiles,
                        &mut name_to_index,
                        next_index,
                        format!("explosion_{frame}"),
                        frame_image,
                        false,
                        1,
                    );
                    next_index += 1;
                }
            }
        }

        let mut missing = RgbaImage::new(TILE_SIZE, TILE_SIZE);
        for (x, y, pixel) in missing.enumerate_pixels_mut() {
            *pixel = if (x / 4 + y / 4) % 2 == 0 {
                image::Rgba([255, 0, 255, 255])
            } else {
                image::Rgba([24, 24, 24, 255])
            };
        }
        add_pending_texture(
            &mut pending,
            &mut tiles,
            &mut name_to_index,
            MISSING_TILE_INDEX,
            "__missing".into(),
            missing,
            false,
            1,
        );
        add_pending_texture(
            &mut pending,
            &mut tiles,
            &mut name_to_index,
            WHITE_TILE_INDEX,
            "__white".into(),
            RgbaImage::from_pixel(TILE_SIZE, TILE_SIZE, image::Rgba([255, 255, 255, 255])),
            false,
            1,
        );

        let (width, height, mut regions) = pack_regions(&pending);
        let missing_region = regions[MISSING_TILE_INDEX];
        for region in &mut regions {
            if region.width == 0 || region.height == 0 {
                *region = missing_region;
            }
        }
        let mut pixels = vec![0; (width * height * 4) as usize];
        for texture in &pending {
            copy_sprite_with_gutter(&mut pixels, width, regions[texture.index], &texture.pixels);
        }

        log::info!(
            "texture atlas ready: textures={}, animated={}, dimensions={}x{}",
            name_to_index.len(),
            animated.len(),
            width,
            height,
        );

        Self {
            pixels,
            animated,
            tiles,
            name_to_index,
            regions,
            tick: 0,
            width,
            height,
        }
    }

    pub fn load(base: &str) -> Self {
        let mut resolver = AssetResolver::new(base);
        Self::load_with_resolver(&mut resolver)
    }

    pub fn animate_tick_into(
        &mut self,
        upload_bytes: &mut Vec<u8>,
        uploads: &mut Vec<AtlasAnimationUpload>,
    ) {
        let previous_tick = self.tick;
        self.tick = self.tick.wrapping_add(1);

        for (index, animation) in &self.animated {
            let previous_frame = frame_at_tick(previous_tick, animation);
            let frame_index = frame_at_tick(self.tick, animation);
            if frame_index == previous_frame {
                continue;
            }
            let Some(frame) = animation.frames.get(frame_index as usize) else {
                continue;
            };
            if frame.len() != (animation.width * animation.height * 4) as usize {
                continue;
            }
            let Some(region) = self.regions.get(*index).copied() else {
                continue;
            };
            if region.width != animation.width || region.height != animation.height {
                continue;
            }

            let buffer_offset = upload_bytes.len() as u64;
            append_sprite_with_gutter(upload_bytes, region, frame);
            uploads.push(AtlasAnimationUpload {
                pixel_x: region.x - ATLAS_PADDING,
                pixel_y: region.y - ATLAS_PADDING,
                width: region.width + ATLAS_PADDING * 2,
                height: region.height + ATLAS_PADDING * 2,
                buffer_offset,
            });
            copy_sprite_with_gutter(&mut self.pixels, self.width, region, frame);
        }
    }

    pub fn tile_index(&self, name: &str) -> usize {
        self.name_to_index
            .get(name)
            .copied()
            .unwrap_or(MISSING_TILE_INDEX)
    }

    pub fn uv_rect(&self, name: &str) -> UvRect {
        self.uv_for_index(self.tile_index(name))
    }

    pub fn uv_for_index(&self, index: usize) -> UvRect {
        self.regions
            .get(index)
            .or_else(|| self.regions.get(MISSING_TILE_INDEX))
            .copied()
            .unwrap_or_default()
            .uv_rect(self.width, self.height)
    }
}

fn decode_resolved_image(resolver: &mut AssetResolver, path: &str) -> Option<DynamicImage> {
    let bytes = resolver.read_bytes(path)?;
    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(error) => {
            log::warn!("failed to decode texture '{path}': {error}");
            return None;
        }
    };
    let (w, h) = image.dimensions();
    if w > MAX_TEX_DIM || h > MAX_TEX_DIM {
        let ratio = MAX_TEX_DIM as f64 / (w.max(h) as f64);
        let new_w = (w as f64 * ratio).ceil() as u32;
        let new_h = (h as f64 * ratio).ceil() as u32;
        log::warn!("texture '{path}' is {w}×{h}, downscaled to {new_w}×{new_h}");
        Some(image.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3))
    } else {
        Some(image)
    }
}

/// Load a texture without the `MAX_TEX_DIM` cap so animated sprite sheets
/// stay at full resolution until individual frames are extracted.
fn decode_resolved_image_full(resolver: &mut AssetResolver, path: &str) -> Option<DynamicImage> {
    let bytes = resolver.read_bytes(path)?;
    match image::load_from_memory(&bytes) {
        Ok(image) => Some(image),
        Err(error) => {
            log::warn!("failed to decode texture '{path}': {error}");
            None
        }
    }
}

fn add_pending_texture(
    pending: &mut Vec<PendingTexture>,
    tiles: &mut HashMap<usize, TileInfo>,
    names: &mut HashMap<String, usize>,
    index: usize,
    name: String,
    image: RgbaImage,
    is_animated: bool,
    frame_count: u32,
) {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return;
    }
    pending.push(PendingTexture {
        index,
        pixels: image.into_raw(),
        width,
        height,
    });
    tiles.insert(
        index,
        TileInfo {
            texture_name: name.clone(),
            is_animated,
            frame_count,
        },
    );
    names.insert(name, index);
}

fn chest_face_regions() -> [(&'static str, u32, u32, u32, u32); 18] {
    [
        ("body_top", 28, 19, 14, 14),
        ("body_bottom", 14, 19, 14, 14),
        ("body_north", 14, 33, 14, 10),
        ("body_south", 42, 33, 14, 10),
        ("body_west", 0, 33, 14, 10),
        ("body_east", 28, 33, 14, 10),
        ("lid_top", 28, 0, 14, 14),
        ("lid_bottom", 14, 0, 14, 14),
        ("lid_north", 14, 14, 14, 5),
        ("lid_south", 42, 14, 14, 5),
        ("lid_west", 0, 14, 14, 5),
        ("lid_east", 28, 14, 14, 5),
        ("knob_top", 3, 0, 2, 1),
        ("knob_bottom", 1, 0, 2, 1),
        ("knob_north", 1, 1, 2, 4),
        ("knob_south", 4, 1, 2, 4),
        ("knob_west", 0, 1, 1, 4),
        ("knob_east", 3, 1, 1, 4),
    ]
}

fn model_box_face_regions(
    texture_x: u32,
    texture_y: u32,
    width: u32,
    height: u32,
    depth: u32,
) -> [(&'static str, u32, u32, u32, u32); 6] {
    [
        ("top", texture_x + depth + width, texture_y, width, depth),
        ("bottom", texture_x + depth, texture_y, width, depth),
        ("north", texture_x + depth, texture_y + depth, width, height),
        (
            "south",
            texture_x + depth + width + depth,
            texture_y + depth,
            width,
            height,
        ),
        ("west", texture_x, texture_y + depth, depth, height),
        (
            "east",
            texture_x + depth + width,
            texture_y + depth,
            depth,
            height,
        ),
    ]
}

fn extract_scaled_model_face(
    image: &DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    texture_width: u32,
    texture_height: u32,
) -> RgbaImage {
    let (source_width, source_height) = image.dimensions();
    let x0 = x * source_width / texture_width;
    let y0 = y * source_height / texture_height;
    let x1 = (x + width) * source_width / texture_width;
    let y1 = (y + height) * source_height / texture_height;
    image
        .crop_imm(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1))
        .to_rgba8()
}

fn extract_scaled_chest_face(
    image: &DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> RgbaImage {
    let (source_width, source_height) = image.dimensions();
    let x0 = x * source_width / 64;
    let y0 = y * source_height / 64;
    let x1 = (x + width) * source_width / 64;
    let y1 = (y + height) * source_height / 64;
    let canvas_width = (TILE_SIZE * source_width / 64).max(1);
    let canvas_height = (TILE_SIZE * source_height / 64).max(1);
    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    let face = image
        .crop_imm(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1))
        .to_rgba8();
    image::imageops::replace(&mut canvas, &face, 0, 0);
    canvas
}

fn pack_regions(textures: &[PendingTexture]) -> (u32, u32, Vec<AtlasRegion>) {
    let total_area: u64 = textures
        .iter()
        .map(|texture| {
            (texture.width + ATLAS_PADDING * 2) as u64 * (texture.height + ATLAS_PADDING * 2) as u64
        })
        .sum();
    let widest = textures
        .iter()
        .map(|texture| texture.width + ATLAS_PADDING * 2)
        .max()
        .unwrap_or(1);
    let approximate_side = (total_area as f64).sqrt().ceil() as u32;
    let mut atlas_width = approximate_side.max(widest).max(64).next_power_of_two();

    loop {
        assert!(
            atlas_width <= MAX_ATLAS_DIMENSION,
            "texture pack requires an atlas wider than {MAX_ATLAS_DIMENSION}px"
        );
        if let Some((needed_height, regions)) = try_pack_at_width(textures, atlas_width) {
            let atlas_height = needed_height.max(1).next_power_of_two();
            if atlas_height <= MAX_ATLAS_DIMENSION {
                return (atlas_width, atlas_height, regions);
            }
        }
        atlas_width *= 2;
    }
}

fn try_pack_at_width(
    textures: &[PendingTexture],
    atlas_width: u32,
) -> Option<(u32, Vec<AtlasRegion>)> {
    let mut order: Vec<_> = textures.iter().collect();
    order.sort_unstable_by(|left, right| {
        right
            .height
            .cmp(&left.height)
            .then_with(|| right.width.cmp(&left.width))
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut regions = vec![AtlasRegion::default(); MAX_TILES];
    let mut x = ATLAS_PADDING;
    let mut y = ATLAS_PADDING;
    let mut shelf_height = 0;

    for texture in order {
        let packed_width = texture.width + ATLAS_PADDING * 2;
        let packed_height = texture.height + ATLAS_PADDING * 2;
        if packed_width > atlas_width {
            return None;
        }
        if x + texture.width + ATLAS_PADDING > atlas_width {
            x = ATLAS_PADDING;
            y += shelf_height;
            shelf_height = 0;
        }
        if y + packed_height > MAX_ATLAS_DIMENSION {
            return None;
        }
        regions[texture.index] = AtlasRegion {
            x,
            y,
            width: texture.width,
            height: texture.height,
        };
        x += packed_width;
        shelf_height = shelf_height.max(packed_height);
    }

    Some((y + shelf_height + ATLAS_PADDING, regions))
}

fn copy_sprite_with_gutter(atlas: &mut [u8], atlas_width: u32, region: AtlasRegion, sprite: &[u8]) {
    for offset_y in -1i32..=region.height as i32 {
        for offset_x in -1i32..=region.width as i32 {
            let source_x = offset_x.clamp(0, region.width as i32 - 1) as u32;
            let source_y = offset_y.clamp(0, region.height as i32 - 1) as u32;
            let destination_x = (region.x as i32 + offset_x) as u32;
            let destination_y = (region.y as i32 + offset_y) as u32;
            let source = ((source_y * region.width + source_x) * 4) as usize;
            let destination = ((destination_y * atlas_width + destination_x) * 4) as usize;
            if source + 4 <= sprite.len() && destination + 4 <= atlas.len() {
                atlas[destination..destination + 4].copy_from_slice(&sprite[source..source + 4]);
            }
        }
    }
}

fn append_sprite_with_gutter(output: &mut Vec<u8>, region: AtlasRegion, sprite: &[u8]) {
    for offset_y in -1i32..=region.height as i32 {
        for offset_x in -1i32..=region.width as i32 {
            let source_x = offset_x.clamp(0, region.width as i32 - 1) as u32;
            let source_y = offset_y.clamp(0, region.height as i32 - 1) as u32;
            let source = ((source_y * region.width + source_x) * 4) as usize;
            if source + 4 <= sprite.len() {
                output.extend_from_slice(&sprite[source..source + 4]);
            }
        }
    }
}

fn frame_at_tick(tick: u32, animation: &AnimatedTexture) -> u32 {
    let total_ticks = animation.frames.len() as u32 * animation.frametime;
    (tick % total_ticks.max(1)) / animation.frametime.max(1)
}

pub fn tile_uv_rect(index: usize) -> UvRect {
    let index = if index < MAX_TILES {
        index
    } else {
        MISSING_TILE_INDEX
    };
    let offset = index * 4;
    let uv = [
        f32::from_bits(ATLAS_UV_BITS[offset].load(Ordering::Relaxed)),
        f32::from_bits(ATLAS_UV_BITS[offset + 1].load(Ordering::Relaxed)),
        f32::from_bits(ATLAS_UV_BITS[offset + 2].load(Ordering::Relaxed)),
        f32::from_bits(ATLAS_UV_BITS[offset + 3].load(Ordering::Relaxed)),
    ];
    if uv[2] > uv[0] && uv[3] > uv[1] {
        uv
    } else {
        legacy_tile_uv_rect(index)
    }
}

fn legacy_tile_uv_rect(index: usize) -> UvRect {
    let x = index as u32 % ATLAS_TILES_X;
    let y = index as u32 / ATLAS_TILES_X;
    [
        x as f32 / ATLAS_TILES_X as f32,
        y as f32 / ATLAS_TILES_Y as f32,
        (x + 1) as f32 / ATLAS_TILES_X as f32,
        (y + 1) as f32 / ATLAS_TILES_Y as f32,
    ]
}

pub fn tile_uv(index: usize) -> ([f32; 2], [f32; 2]) {
    let region = tile_uv_rect(index);
    ([region[0], region[1]], [region[2], region[3]])
}

fn strip_png_ext(name: &str) -> String {
    name.strip_suffix(".png").unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation_atlas(width: u32, height: u32) -> TextureAtlas {
        let index = 7;
        let frame_len = (width * height * 4) as usize;
        let region = AtlasRegion {
            x: 3,
            y: 5,
            width,
            height,
        };
        let mut regions = vec![region; MAX_TILES];
        regions[index] = region;
        TextureAtlas {
            pixels: vec![0; (32 * 32 * 4) as usize],
            animated: HashMap::from([(
                index,
                AnimatedTexture {
                    frames: vec![vec![0; frame_len], vec![7; frame_len]],
                    width,
                    height,
                    frametime: 2,
                    interpolate: false,
                },
            )]),
            tiles: HashMap::new(),
            name_to_index: HashMap::new(),
            regions,
            tick: 0,
            width: 32,
            height: 32,
        }
    }

    #[test]
    fn animation_upload_preserves_native_frame_dimensions() {
        let mut atlas = animation_atlas(8, 12);
        let mut bytes = Vec::new();
        let mut uploads = Vec::new();

        atlas.animate_tick_into(&mut bytes, &mut uploads);
        assert!(uploads.is_empty());
        atlas.animate_tick_into(&mut bytes, &mut uploads);

        assert_eq!(bytes.len(), 10 * 14 * 4);
        assert_eq!(
            uploads,
            vec![AtlasAnimationUpload {
                pixel_x: 2,
                pixel_y: 4,
                width: 10,
                height: 14,
                buffer_offset: 0,
            }]
        );
        let atlas_offset = ((5 * 32 + 3) * 4) as usize;
        assert_eq!(&atlas.pixels[atlas_offset..atlas_offset + 4], &[7; 4]);
    }

    #[test]
    fn packer_keeps_high_resolution_pixels_and_distinct_uv_regions() {
        let textures = vec![
            PendingTexture {
                index: 0,
                pixels: vec![1; 32 * 32 * 4],
                width: 32,
                height: 32,
            },
            PendingTexture {
                index: MISSING_TILE_INDEX,
                pixels: vec![2; 16 * 16 * 4],
                width: 16,
                height: 16,
            },
        ];
        let (width, height, regions) = pack_regions(&textures);
        assert_eq!(regions[0].width, 32);
        assert_eq!(regions[0].height, 32);
        assert_ne!(regions[0], regions[MISSING_TILE_INDEX]);
        let mut pixels = vec![0; (width * height * 4) as usize];
        copy_sprite_with_gutter(&mut pixels, width, regions[0], &textures[0].pixels);
        let final_pixel = (((regions[0].y + 31) * width + regions[0].x + 31) * 4) as usize;
        assert_eq!(&pixels[final_pixel..final_pixel + 4], &[1; 4]);
    }

    #[test]
    fn uv_region_keeps_the_full_vanilla_texture_extent() {
        let region = AtlasRegion {
            x: 17,
            y: 33,
            width: 16,
            height: 16,
        };
        let uv = region.uv_rect(256, 256);

        assert_eq!(uv[2] - uv[0], 16.0 / 256.0);
        assert_eq!(uv[3] - uv[1], 16.0 / 256.0);
    }

    #[test]
    fn chest_face_crop_scales_with_resource_pack_resolution() {
        let mut source = RgbaImage::new(128, 128);
        source.put_pixel(56, 38, image::Rgba([9, 8, 7, 255]));
        let face = extract_scaled_chest_face(&DynamicImage::ImageRgba8(source), 28, 19, 14, 14);
        assert_eq!(face.dimensions(), (32, 32));
        assert_eq!(face.get_pixel(0, 0).0, [9, 8, 7, 255]);
    }
}
