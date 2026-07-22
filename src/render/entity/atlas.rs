//! Entity texture atlas - loads MC mob textures into a single atlas.

use image::GenericImageView;
use std::collections::HashMap;

/// Atlas dimensions. 1.8.9's mob variants, armor overlays and dynamic player
/// skins do not fit reliably in the old 1024x1024 allocation.
// Increased from 2048 to 4096 so servers with 200+ visible signs can all fit
// their rasterised text in the dedicated sign band (y=1024..1792).  The full
// 16 MiB → 64 MiB GPU memory increase is acceptable (sign/nametag atlas
// uploads are user-triggered, never per-frame).
pub const ENTITY_ATLAS_SIZE: u32 = 4096;
pub const PLAYER_SKIN_ATLAS_Y: u32 = 1792;
pub const PLAYER_SKIN_ATLAS_HEIGHT: u32 = ENTITY_ATLAS_SIZE - PLAYER_SKIN_ATLAS_Y;
const PLAYER_SKIN_CELL: u32 = 64;
pub const PLAYER_SKIN_CAPACITY: usize = ((ENTITY_ATLAS_SIZE / PLAYER_SKIN_CELL)
    * (PLAYER_SKIN_ATLAS_HEIGHT / PLAYER_SKIN_CELL))
    as usize;

// Sign text gets a dedicated band so it never competes with nametags for atlas
// space. Each sign cell is 180*80 with 1 px padding => 181*81.
// At y=1024: (1792-1024)/81 = 9 rows * (2048/181) 11 cols = 99 signs. If that
// overflows we fall back to searching the full runtime area (below y=1024) too.
const SIGN_BAND_Y_START: u32 = 1024;
const SIGN_TEXT_CELL_W: u32 = 180;
const SIGN_TEXT_CELL_H: u32 = 80;

/// UV region for a mob texture in the atlas.
#[derive(Clone, Copy, Debug)]
pub struct MobTextureRegion {
    /// Top-left U coordinate in atlas (0.0–1.0)
    pub u_min: f32,
    /// Top-left V coordinate in atlas (0.0–1.0)
    pub v_min: f32,
    /// Bottom-right U coordinate in atlas (0.0–1.0)
    pub u_max: f32,
    /// Bottom-right V coordinate in atlas (0.0–1.0)
    pub v_max: f32,
    /// Original texture width in pixels
    pub tex_width: u32,
    /// Original texture height in pixels
    pub tex_height: u32,
}

impl MobTextureRegion {
    /// Convert local UV (0.0-1.0 within the mob texture) to atlas UV.
    pub fn local_to_atlas(&self, u: f32, v: f32) -> (f32, f32) {
        (
            self.u_min + u * (self.u_max - self.u_min),
            self.v_min + v * (self.v_max - self.v_min),
        )
    }
}

/// Entity texture atlas.
pub struct EntityTextureAtlas {
    /// Atlas RGBA pixels.
    pub pixels: Vec<u8>,
    /// Entity type name → texture region in atlas
    pub regions: HashMap<String, MobTextureRegion>,
    player_skin_hashes: HashMap<String, u64>,
    player_cape_hashes: HashMap<String, u64>,
    full_upload_required: bool,
}

impl EntityTextureAtlas {
    fn find_free_region(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        self.find_free_region_in_band(width, height, 0, PLAYER_SKIN_ATLAS_Y)
    }

    /// Search for a free cell within a Y-band [y_start, y_end).
    fn find_free_region_in_band(
        &self,
        width: u32,
        height: u32,
        y_start: u32,
        y_end: u32,
    ) -> Option<(u32, u32)> {
        let columns = ENTITY_ATLAS_SIZE / width;
        let row_bottom = y_start / height;
        let row_top = y_end / height;
        for row in (row_bottom..row_top).rev() {
            let y = row * height;
            for column in 0..columns {
                let x = column * width;
                let occupied = self.regions.values().any(|region| {
                    let rx = (region.u_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
                    let ry = (region.v_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
                    let rw = region.tex_width;
                    let rh = region.tex_height;
                    x < rx + rw && x + width > rx && y < ry + rh && y + height > ry
                });
                if !occupied {
                    return Some((x, y));
                }
            }
        }
        None
    }

    fn clear_runtime_region(&mut self, region: MobTextureRegion) {
        let x = (region.u_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
        let y = (region.v_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
        for row in y..y + region.tex_height {
            let start = ((row * ENTITY_ATLAS_SIZE + x) * 4) as usize;
            let end = start + (region.tex_width * 4) as usize;
            self.pixels[start..end].fill(0);
        }
    }

    pub fn load_with_resolver(resolver: &mut crate::assets::resolver::AssetResolver) -> Self {
        Self::load_from_reader(|texture_path| {
            resolver.read_bytes(&format!("minecraft/textures/entity/{}", texture_path))
        })
    }

    pub fn load(base: &str) -> Self {
        Self::load_from_reader(|texture_path| {
            std::fs::read(format!("{}/textures/entity/{}", base, texture_path)).ok()
        })
    }

    fn load_from_reader(mut read_texture: impl FnMut(&str) -> Option<Vec<u8>>) -> Self {
        let mut atlas = vec![0u8; (ENTITY_ATLAS_SIZE * ENTITY_ATLAS_SIZE * 4) as usize];
        let mut regions = HashMap::new();

        // Pack textures by their real dimensions. No scaling, no UV drift.
        let mob_defs = get_mob_texture_definitions();

        let mut cursor_x = 0u32;
        let mut cursor_y = 0u32;
        let mut row_h = 0u32;
        for (mob_name, tex_path) in &mob_defs {
            if let Some(img) =
                read_texture(tex_path).and_then(|bytes| image::load_from_memory(&bytes).ok())
            {
                let (orig_w, orig_h) = img.dimensions();
                let rgba = img.to_rgba8();
                let pixels = rgba.into_raw();
                let pad = 1u32;
                let place_w = orig_w + pad;
                let place_h = orig_h + pad;

                if cursor_x + place_w > ENTITY_ATLAS_SIZE {
                    cursor_x = 0;
                    cursor_y += row_h;
                    row_h = 0;
                }
                if cursor_y + place_h > PLAYER_SKIN_ATLAS_Y {
                    log::warn!(
                        "entity texture atlas full; skipping mob='{}', texture='{}'",
                        mob_name,
                        tex_path
                    );
                    continue;
                }

                copy_image_to_atlas(&mut atlas, cursor_x, cursor_y, orig_w, orig_h, &pixels);

                let u_min = cursor_x as f32 / ENTITY_ATLAS_SIZE as f32;
                let v_min = cursor_y as f32 / ENTITY_ATLAS_SIZE as f32;
                let u_max = (cursor_x + orig_w) as f32 / ENTITY_ATLAS_SIZE as f32;
                let v_max = (cursor_y + orig_h) as f32 / ENTITY_ATLAS_SIZE as f32;

                regions.insert(
                    mob_name.clone(),
                    MobTextureRegion {
                        u_min,
                        v_min,
                        u_max,
                        v_max,
                        tex_width: orig_w,
                        tex_height: orig_h,
                    },
                );

                cursor_x += place_w;
                row_h = row_h.max(place_h);
            } else {
                log::warn!("failed to load entity texture: mob='{mob_name}', texture='{tex_path}'");
            }
        }

        // Add a solid white 2×2 pixel texture for untextured rendering (wireframe, shadows, etc.)
        // This ensures vertex colors are applied correctly without being multiplied by a dark texture.
        {
            let pad = 1u32;
            if cursor_x + 2 + pad > ENTITY_ATLAS_SIZE {
                cursor_x = 0;
                cursor_y += row_h;
            }
            let wx = cursor_x;
            let wy = cursor_y;
            // Write 2×2 white RGBA pixels
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let ax = wx + dx;
                    let ay = wy + dy;
                    let idx = ((ay * ENTITY_ATLAS_SIZE + ax) * 4) as usize;
                    if idx + 4 <= atlas.len() {
                        atlas[idx..idx + 4].copy_from_slice(&[255, 255, 255, 255]);
                    }
                }
            }
            regions.insert(
                "__white".into(),
                MobTextureRegion {
                    u_min: wx as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_min: wy as f32 / ENTITY_ATLAS_SIZE as f32,
                    u_max: (wx + 2) as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_max: (wy + 2) as f32 / ENTITY_ATLAS_SIZE as f32,
                    tex_width: 2,
                    tex_height: 2,
                },
            );
            cursor_x += 2 + pad;
            row_h = row_h.max(2 + pad);
        }

        // Cape slot — 64×32, shared by all players with the same cape.
        // Pixels are uploaded by `upload_cape` when the local player's
        // cape changes at runtime. cursor_x already advanced past __white.
        {
            const CAPE_PAD: u32 = 1;
            if cursor_x + 64 + CAPE_PAD > ENTITY_ATLAS_SIZE {
                cursor_x = 0;
                cursor_y += row_h;
            }
            regions.insert(
                "player/cape".into(),
                MobTextureRegion {
                    u_min: cursor_x as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_min: cursor_y as f32 / ENTITY_ATLAS_SIZE as f32,
                    u_max: (cursor_x + 64) as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_max: (cursor_y + 32) as f32 / ENTITY_ATLAS_SIZE as f32,
                    tex_width: 64,
                    tex_height: 32,
                },
            );
        }

        log::info!(
            "entity texture atlas ready: mob_textures={}, dimensions={}x{}",
            regions.len(),
            ENTITY_ATLAS_SIZE,
            ENTITY_ATLAS_SIZE
        );

        EntityTextureAtlas {
            pixels: atlas,
            regions,
            player_skin_hashes: HashMap::new(),
            player_cape_hashes: HashMap::new(),
            full_upload_required: false,
        }
    }

    pub fn sync_player_skins(&mut self, skins: &[crate::render::PendingPlayerSkin]) -> bool {
        let hashes: HashMap<_, _> = skins
            .iter()
            .map(|skin| (skin.key.clone(), skin.content_hash))
            .collect();
        let cape_hashes: HashMap<_, _> = skins
            .iter()
            .filter(|skin| skin.cape_pixels.is_some())
            .map(|skin| (skin.key.clone(), skin.cape_content_hash))
            .collect();
        let capes_changed = cape_hashes != self.player_cape_hashes;
        if hashes == self.player_skin_hashes && cape_hashes == self.player_cape_hashes {
            return false;
        }

        let old_cape_regions: Vec<_> = self
            .regions
            .iter()
            .filter(|(name, _)| name.starts_with("cape/"))
            .map(|(name, region)| (name.clone(), *region))
            .collect();
        for (name, region) in old_cape_regions {
            self.regions.remove(&name);
            self.clear_runtime_region(region);
        }

        for y in PLAYER_SKIN_ATLAS_Y..ENTITY_ATLAS_SIZE {
            let start = (y * ENTITY_ATLAS_SIZE * 4) as usize;
            let end = start + (ENTITY_ATLAS_SIZE * 4) as usize;
            self.pixels[start..end].fill(0);
        }
        self.regions
            .retain(|name, _| name == "player/cape" || !name.starts_with("player/"));

        let columns = ENTITY_ATLAS_SIZE / PLAYER_SKIN_CELL;
        for (slot, pending) in skins.iter().take(PLAYER_SKIN_CAPACITY).enumerate() {
            let key = &pending.key;
            let skin = pending.skin.as_ref();
            let x0 = slot as u32 % columns * PLAYER_SKIN_CELL;
            let y0 = PLAYER_SKIN_ATLAS_Y + slot as u32 / columns * PLAYER_SKIN_CELL;
            let (width, height) = skin.dimensions();
            let copy_width = width.min(PLAYER_SKIN_CELL);
            let copy_height = height.min(PLAYER_SKIN_CELL);
            for y in 0..copy_height {
                for x in 0..copy_width {
                    let pixel = skin.pixels.get_pixel(x, y).0;
                    let dst = (((y0 + y) * ENTITY_ATLAS_SIZE + x0 + x) * 4) as usize;
                    self.pixels[dst..dst + 4].copy_from_slice(&pixel);
                }
            }
            self.regions.insert(
                key.clone(),
                MobTextureRegion {
                    u_min: x0 as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_min: y0 as f32 / ENTITY_ATLAS_SIZE as f32,
                    u_max: (x0 + copy_width) as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_max: (y0 + copy_height) as f32 / ENTITY_ATLAS_SIZE as f32,
                    tex_width: copy_width,
                    tex_height: copy_height,
                },
            );
        }
        for pending in skins {
            let Some(cape_pixels) = pending.cape_pixels.as_deref() else {
                continue;
            };
            // Capes pack only in the sign band (below player skins), never in
            // the static mob shelf (0..SIGN_BAND_Y_START).
            let Some((x, y)) =
                self.find_free_region_in_band(64, 32, SIGN_BAND_Y_START, PLAYER_SKIN_ATLAS_Y)
            else {
                log::warn!(
                    "entity texture atlas has no room for cape '{}'; skipping",
                    pending.key
                );
                continue;
            };
            copy_image_to_atlas(&mut self.pixels, x, y, 64, 32, cape_pixels);
            self.regions.insert(
                format!("cape/{}", pending.key),
                MobTextureRegion {
                    u_min: x as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_min: y as f32 / ENTITY_ATLAS_SIZE as f32,
                    u_max: (x + 64) as f32 / ENTITY_ATLAS_SIZE as f32,
                    v_max: (y + 32) as f32 / ENTITY_ATLAS_SIZE as f32,
                    tex_width: 64,
                    tex_height: 32,
                },
            );
        }
        self.player_skin_hashes = hashes;
        self.player_cape_hashes = cape_hashes;
        self.full_upload_required |= capes_changed;
        true
    }

    pub fn take_full_upload_required(&mut self) -> bool {
        std::mem::take(&mut self.full_upload_required)
    }

    /// Upload cape pixels into the dedicated "player/cape" atlas slot.
    pub fn upload_cape(&mut self, pixels: &[u8], w: u32, h: u32) {
        let Some(region) = self.regions.get("player/cape") else {
            return;
        };
        let u_min = (region.u_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
        let v_min = (region.v_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
        for y in 0..h.min(region.tex_height) {
            for x in 0..w.min(region.tex_width) {
                let src = (y * w + x) as usize * 4;
                if src + 4 > pixels.len() {
                    break;
                }
                let dst = ((v_min + y) * ENTITY_ATLAS_SIZE + u_min + x) as usize * 4;
                if dst + 4 <= self.pixels.len() {
                    self.pixels[dst] = pixels[src];
                    self.pixels[dst + 1] = pixels[src + 1];
                    self.pixels[dst + 2] = pixels[src + 2];
                    self.pixels[dst + 3] = pixels[src + 3];
                }
            }
        }
    }

    pub fn player_skin_pixels(&self) -> &[u8] {
        let start = (PLAYER_SKIN_ATLAS_Y * ENTITY_ATLAS_SIZE * 4) as usize;
        &self.pixels[start..]
    }

    /// Get texture region for a mob type.
    pub fn region_for(&self, mob_name: &str) -> Option<&MobTextureRegion> {
        self.regions.get(mob_name)
    }

    /// Pack a runtime-generated sign text texture into the atlas.
    /// Signs use a dedicated band to avoid competing with nametags for space.
    pub fn pack_sign_text(
        &mut self,
        key: &str,
        pixels: &[u8],
        w: u32,
        h: u32,
    ) -> Option<MobTextureRegion> {
        if w > SIGN_TEXT_CELL_W || h > SIGN_TEXT_CELL_H {
            return None;
        }

        // An existing sign keeps its assigned cell.  Counting regions again
        // placed a changed sign into a new cell and left its mesh sampling the
        // stale texture from the old one.
        let (x, y) = if let Some(region) = self.regions.get(key) {
            (
                (region.u_min * ENTITY_ATLAS_SIZE as f32).round() as u32,
                (region.v_min * ENTITY_ATLAS_SIZE as f32).round() as u32,
            )
        } else {
            // Pack signs only in the dedicated band. Never fall back into
            // 0..SIGN_BAND_Y_START — that region holds shelf-packed mob skins,
            // and overwriting them scrambles every entity texture.
            self.find_free_region_in_band(
                SIGN_TEXT_CELL_W,
                SIGN_TEXT_CELL_H,
                SIGN_BAND_Y_START,
                PLAYER_SKIN_ATLAS_Y,
            )?
        };
        for row in y..y + SIGN_TEXT_CELL_H {
            let start = ((row * ENTITY_ATLAS_SIZE + x) * 4) as usize;
            let end = start + (SIGN_TEXT_CELL_W * 4) as usize;
            self.pixels[start..end].fill(0);
        }
        copy_image_to_atlas(&mut self.pixels, x, y, w, h, pixels);
        let region = MobTextureRegion {
            u_min: x as f32 / ENTITY_ATLAS_SIZE as f32,
            v_min: y as f32 / ENTITY_ATLAS_SIZE as f32,
            u_max: (x + w) as f32 / ENTITY_ATLAS_SIZE as f32,
            v_max: (y + h) as f32 / ENTITY_ATLAS_SIZE as f32,
            tex_width: w,
            tex_height: h,
        };
        self.regions.insert(key.to_string(), region);
        Some(region)
    }

    /// Pack a nametag text texture into the atlas. Returns the region if successful.
    /// Nametags use the area below the sign band.
    pub fn pack_nametag_text(
        &mut self,
        key: &str,
        pixels: &[u8],
        w: u32,
        h: u32,
    ) -> Option<MobTextureRegion> {
        const NAMETAG_CELL_W: u32 = 192;
        const NAMETAG_CELL_H: u32 = 20;
        if w > NAMETAG_CELL_W || h > NAMETAG_CELL_H {
            return None;
        }
        let (x, y) = self.find_free_region_in_band(
            NAMETAG_CELL_W,
            NAMETAG_CELL_H,
            0,
            SIGN_BAND_Y_START,
        )?;
        for row in y..y + NAMETAG_CELL_H {
            let start = ((row * ENTITY_ATLAS_SIZE + x) * 4) as usize;
            let end = start + (NAMETAG_CELL_W * 4) as usize;
            self.pixels[start..end].fill(0);
        }
        copy_image_to_atlas(&mut self.pixels, x, y, w, h, pixels);
        let region = MobTextureRegion {
            u_min: x as f32 / ENTITY_ATLAS_SIZE as f32,
            v_min: y as f32 / ENTITY_ATLAS_SIZE as f32,
            u_max: (x + w) as f32 / ENTITY_ATLAS_SIZE as f32,
            v_max: (y + h) as f32 / ENTITY_ATLAS_SIZE as f32,
            tex_width: w,
            tex_height: h,
        };
        self.regions.insert(key.to_string(), region);
        Some(region)
    }

    /// Remove all nametag regions (called at start of frame to rebuild).
    pub fn clear_nametag_texts(&mut self) {
        let regions: Vec<(String, MobTextureRegion)> = self
            .regions
            .iter()
            .filter(|(key, _)| key.starts_with("nametag_"))
            .map(|(key, region)| (key.clone(), *region))
            .collect();
        for (key, region) in regions {
            self.clear_runtime_region(region);
            self.regions.remove(&key);
        }
    }

    pub fn clear_sign_texts(&mut self) {
        let regions: Vec<(String, MobTextureRegion)> = self
            .regions
            .iter()
            .filter(|(key, _)| key.starts_with("sign_"))
            .map(|(key, region)| (key.clone(), *region))
            .collect();
        for (key, region) in regions {
            self.clear_runtime_region(region);
            self.regions.remove(&key);
        }
    }
}

fn copy_image_to_atlas(atlas: &mut [u8], dst_x: u32, dst_y: u32, w: u32, h: u32, pixels: &[u8]) {
    for py in 0..h as usize {
        for px in 0..w as usize {
            let src = (py * w as usize + px) * 4;
            let ax = dst_x as usize + px;
            let ay = dst_y as usize + py;
            let dst = (ay * ENTITY_ATLAS_SIZE as usize + ax) * 4;
            if src + 4 <= pixels.len() && dst + 4 <= atlas.len() {
                atlas[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
            }
        }
    }
}

/// Returns (mob_name, texture_relative_path) pairs for all supported mobs.
fn get_mob_texture_definitions() -> Vec<(String, String)> {
    vec![
        ("arrow".into(), "arrow.png".into()),
        ("chest_normal".into(), "chest/normal.png".into()),
        ("chest_trapped".into(), "chest/trapped.png".into()),
        ("chest_ender".into(), "chest/ender.png".into()),
        (
            "chest_normal_double".into(),
            "chest/normal_double.png".into(),
        ),
        (
            "chest_trapped_double".into(),
            "chest/trapped_double.png".into(),
        ),
        // Humanoid mobs
        ("zombie".into(), "zombie/zombie.png".into()),
        (
            "zombie_villager".into(),
            "zombie/zombie_villager.png".into(),
        ),
        ("skeleton".into(), "skeleton/skeleton.png".into()),
        (
            "wither_skeleton".into(),
            "skeleton/wither_skeleton.png".into(),
        ),
        ("zombie_pigman".into(), "zombie_pigman.png".into()),
        ("witch".into(), "witch.png".into()),
        ("minecart".into(), "minecart.png".into()),
        ("boat".into(), "boat.png".into()),
        ("player".into(), "steve.png".into()),
        ("player_slim".into(), "alex.png".into()),
        // Hostile mobs
        ("creeper".into(), "creeper/creeper.png".into()),
        ("spider".into(), "spider/spider.png".into()),
        ("cave_spider".into(), "spider/cave_spider.png".into()),
        ("enderman".into(), "enderman/enderman.png".into()),
        ("slime".into(), "slime/slime.png".into()),
        ("magma_cube".into(), "slime/magmacube.png".into()),
        ("ghast".into(), "ghast/ghast.png".into()),
        ("blaze".into(), "blaze.png".into()),
        ("silverfish".into(), "silverfish.png".into()),
        ("endermite".into(), "endermite.png".into()),
        ("guardian".into(), "guardian.png".into()),
        ("guardian_elder".into(), "guardian_elder.png".into()),
        ("wither".into(), "wither/wither.png".into()),
        ("ender_dragon".into(), "enderdragon/dragon.png".into()),
        // Passive mobs
        ("pig".into(), "pig/pig.png".into()),
        ("cow".into(), "cow/cow.png".into()),
        ("mooshroom".into(), "cow/mooshroom.png".into()),
        ("sheep".into(), "sheep/sheep.png".into()),
        ("sheep_fur".into(), "sheep/sheep_fur.png".into()),
        ("chicken".into(), "chicken.png".into()),
        ("wolf".into(), "wolf/wolf.png".into()),
        ("wolf_tame".into(), "wolf/wolf_tame.png".into()),
        ("wolf_angry".into(), "wolf/wolf_angry.png".into()),
        ("wolf_collar".into(), "wolf/wolf_collar.png".into()),
        ("ocelot".into(), "cat/ocelot.png".into()),
        ("ocelot_black".into(), "cat/black.png".into()),
        ("ocelot_red".into(), "cat/red.png".into()),
        ("ocelot_siamese".into(), "cat/siamese.png".into()),
        ("horse_white".into(), "horse/horse_white.png".into()),
        ("horse_creamy".into(), "horse/horse_creamy.png".into()),
        ("horse_chestnut".into(), "horse/horse_chestnut.png".into()),
        ("horse_brown".into(), "horse/horse_brown.png".into()),
        ("horse_black".into(), "horse/horse_black.png".into()),
        ("horse_gray".into(), "horse/horse_gray.png".into()),
        ("horse_darkbrown".into(), "horse/horse_darkbrown.png".into()),
        ("horse_donkey".into(), "horse/donkey.png".into()),
        ("horse_mule".into(), "horse/mule.png".into()),
        ("horse_zombie".into(), "horse/horse_zombie.png".into()),
        ("horse_skeleton".into(), "horse/horse_skeleton.png".into()),
        (
            "horse_armor_iron".into(),
            "horse/armor/horse_armor_iron.png".into(),
        ),
        (
            "horse_armor_gold".into(),
            "horse/armor/horse_armor_gold.png".into(),
        ),
        (
            "horse_armor_diamond".into(),
            "horse/armor/horse_armor_diamond.png".into(),
        ),
        ("rabbit_white".into(), "rabbit/white.png".into()),
        ("rabbit_brown".into(), "rabbit/brown.png".into()),
        ("rabbit_black".into(), "rabbit/black.png".into()),
        ("rabbit_gold".into(), "rabbit/gold.png".into()),
        ("rabbit_salt".into(), "rabbit/salt.png".into()),
        (
            "rabbit_white_splotched".into(),
            "rabbit/white_splotched.png".into(),
        ),
        ("rabbit_toast".into(), "rabbit/toast.png".into()),
        ("rabbit_caerbannog".into(), "rabbit/caerbannog.png".into()),
        ("bat".into(), "bat.png".into()),
        ("snowman".into(), "snowman.png".into()),
        ("iron_golem".into(), "iron_golem.png".into()),
        ("villager".into(), "villager/villager.png".into()),
        ("villager_farmer".into(), "villager/farmer.png".into()),
        ("villager_librarian".into(), "villager/librarian.png".into()),
        ("villager_priest".into(), "villager/priest.png".into()),
        ("villager_smith".into(), "villager/smith.png".into()),
        ("villager_butcher".into(), "villager/butcher.png".into()),
        ("armor_stand".into(), "armorstand/wood.png".into()),
        ("squid".into(), "squid.png".into()),
        // Experience orb texture (64x64, 4x4 grid of 16x16 sprites)
        ("experience_orb".into(), "experience_orb.png".into()),
        // Armor overlay textures (MC 1.8.9: textures/models/armor/)
        (
            "leather_layer_1".into(),
            "../models/armor/leather_layer_1.png".into(),
        ),
        (
            "leather_layer_2".into(),
            "../models/armor/leather_layer_2.png".into(),
        ),
        (
            "chainmail_layer_1".into(),
            "../models/armor/chainmail_layer_1.png".into(),
        ),
        (
            "chainmail_layer_2".into(),
            "../models/armor/chainmail_layer_2.png".into(),
        ),
        (
            "iron_layer_1".into(),
            "../models/armor/iron_layer_1.png".into(),
        ),
        (
            "iron_layer_2".into(),
            "../models/armor/iron_layer_2.png".into(),
        ),
        (
            "diamond_layer_1".into(),
            "../models/armor/diamond_layer_1.png".into(),
        ),
        (
            "diamond_layer_2".into(),
            "../models/armor/diamond_layer_2.png".into(),
        ),
        (
            "gold_layer_1".into(),
            "../models/armor/gold_layer_1.png".into(),
        ),
        (
            "gold_layer_2".into(),
            "../models/armor/gold_layer_2.png".into(),
        ),
        // Destroy stage overlays (block breaking crack textures)
        (
            "destroy_0".into(),
            "../../textures/blocks/destroy_stage_0.png".into(),
        ),
        (
            "destroy_1".into(),
            "../../textures/blocks/destroy_stage_1.png".into(),
        ),
        (
            "destroy_2".into(),
            "../../textures/blocks/destroy_stage_2.png".into(),
        ),
        (
            "destroy_3".into(),
            "../../textures/blocks/destroy_stage_3.png".into(),
        ),
        (
            "destroy_4".into(),
            "../../textures/blocks/destroy_stage_4.png".into(),
        ),
        (
            "destroy_5".into(),
            "../../textures/blocks/destroy_stage_5.png".into(),
        ),
        (
            "destroy_6".into(),
            "../../textures/blocks/destroy_stage_6.png".into(),
        ),
        (
            "destroy_7".into(),
            "../../textures/blocks/destroy_stage_7.png".into(),
        ),
        (
            "destroy_8".into(),
            "../../textures/blocks/destroy_stage_8.png".into(),
        ),
        (
            "destroy_9".into(),
            "../../textures/blocks/destroy_stage_9.png".into(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn empty_atlas() -> EntityTextureAtlas {
        EntityTextureAtlas {
            pixels: vec![0; (ENTITY_ATLAS_SIZE * ENTITY_ATLAS_SIZE * 4) as usize],
            regions: HashMap::new(),
            player_skin_hashes: HashMap::new(),
            player_cape_hashes: HashMap::new(),
            full_upload_required: false,
        }
    }

    fn pending_skin(content_hash: u64) -> crate::render::PendingPlayerSkin {
        crate::render::PendingPlayerSkin {
            key: "player/test".to_string(),
            skin: Arc::new(crate::assets::skin::PlayerSkin::default_steve()),
            content_hash,
            cape_pixels: None,
            cape_content_hash: 0,
        }
    }

    #[test]
    fn unchanged_player_skin_set_skips_repack() {
        let mut atlas = empty_atlas();
        let skins = vec![pending_skin(7)];

        assert!(atlas.sync_player_skins(&skins));
        assert!(!atlas.sync_player_skins(&skins));
        assert!(atlas.region_for("player/test").is_some());
    }

    #[test]
    fn changed_player_skin_content_rebuilds_existing_slot() {
        let mut atlas = empty_atlas();
        assert!(atlas.sync_player_skins(&[pending_skin(7)]));
        assert!(atlas.sync_player_skins(&[pending_skin(8)]));
    }

    #[test]
    fn player_cape_gets_an_independent_region_and_full_upload() {
        let mut atlas = empty_atlas();
        let mut skin = pending_skin(7);
        skin.cape_pixels = Some(Arc::new(vec![37; 64 * 32 * 4]));
        skin.cape_content_hash = 11;

        assert!(atlas.sync_player_skins(&[skin]));
        assert!(atlas.take_full_upload_required());
        let region = *atlas.region_for("cape/player/test").unwrap();
        let x = (region.u_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
        let y = (region.v_min * ENTITY_ATLAS_SIZE as f32).round() as u32;
        let offset = ((y * ENTITY_ATLAS_SIZE + x) * 4) as usize;
        assert_eq!(&atlas.pixels[offset..offset + 4], &[37; 4]);
    }

    #[test]
    fn player_skin_repack_keeps_the_local_cape_slot() {
        let mut atlas = empty_atlas();
        atlas.regions.insert(
            "player/cape".to_string(),
            MobTextureRegion {
                u_min: 0.0,
                v_min: 0.0,
                u_max: 64.0 / ENTITY_ATLAS_SIZE as f32,
                v_max: 32.0 / ENTITY_ATLAS_SIZE as f32,
                tex_width: 64,
                tex_height: 32,
            },
        );

        assert!(atlas.sync_player_skins(&[pending_skin(1)]));
        assert!(atlas.region_for("player/cape").is_some());
    }

    #[test]
    fn player_skin_upload_slice_only_contains_reserved_band() {
        let atlas = empty_atlas();
        assert_eq!(
            atlas.player_skin_pixels().len(),
            (ENTITY_ATLAS_SIZE * PLAYER_SKIN_ATLAS_HEIGHT * 4) as usize
        );
    }

    #[test]
    fn sign_text_band_fits_a_dense_area_and_reuses_slots_after_clear() {
        let mut atlas = empty_atlas();
        let pixels = vec![255; 180 * 80 * 4];
        // The sign raster cell leaves room for a dense dynamic sign area
        // atlas band after static entity textures, comfortably covering a
        // dense visible sign area without reducing glyph resolution.
        for index in 0..50 {
            atlas
                .pack_sign_text(&format!("sign_{index}"), &pixels, 180, 80)
                .unwrap();
        }
        assert!(atlas.region_for("sign_49").is_some());

        atlas.clear_sign_texts();
        atlas
            .pack_sign_text("sign_reloaded", &pixels, 180, 80)
            .unwrap();
        let region = atlas.region_for("sign_reloaded").unwrap();
        assert_eq!(region.tex_width, 180);
        assert_eq!(region.tex_height, 80);
        assert!(atlas.region_for("sign_49").is_none());
    }

    #[test]
    fn runtime_text_clear_never_erases_existing_entity_textures() {
        let mut atlas = empty_atlas();
        let static_x = 0u32;
        let static_y = 1760u32;
        let static_width = 64u32;
        let static_height = 32u32;
        for y in static_y..static_y + static_height {
            let start = ((y * ENTITY_ATLAS_SIZE + static_x) * 4) as usize;
            let end = start + (static_width * 4) as usize;
            atlas.pixels[start..end].fill(17);
        }
        atlas.regions.insert(
            "wither_skeleton".into(),
            MobTextureRegion {
                u_min: static_x as f32 / ENTITY_ATLAS_SIZE as f32,
                v_min: static_y as f32 / ENTITY_ATLAS_SIZE as f32,
                u_max: (static_x + static_width) as f32 / ENTITY_ATLAS_SIZE as f32,
                v_max: (static_y + static_height) as f32 / ENTITY_ATLAS_SIZE as f32,
                tex_width: static_width,
                tex_height: static_height,
            },
        );

        atlas
            .pack_sign_text("sign_test", &vec![255; 180 * 80 * 4], 180, 80)
            .unwrap();
        atlas
            .pack_nametag_text("nametag_test", &vec![255; 64 * 20 * 4], 64, 20)
            .unwrap();
        atlas.clear_sign_texts();
        atlas.clear_nametag_texts();

        for y in static_y..static_y + static_height {
            let start = ((y * ENTITY_ATLAS_SIZE + static_x) * 4) as usize;
            let end = start + (static_width * 4) as usize;
            assert!(atlas.pixels[start..end].iter().all(|pixel| *pixel == 17));
        }
    }
}
