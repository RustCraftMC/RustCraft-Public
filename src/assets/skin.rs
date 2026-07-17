use image::RgbaImage;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SkinLayout {
    Classic64x32,
    #[default]
    Modern64x64,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerSkin {
    pub pixels: RgbaImage,
    pub layout: SkinLayout,
    pub slim_arms: bool,
}

#[derive(Clone, Debug)]
pub struct SkinPreviewPixels {
    pub head: [[u8; 4]; 64],
    pub body: [[u8; 4]; 96],
    pub right_arm: [[u8; 4]; 48],
    pub left_arm: [[u8; 4]; 48],
    pub right_leg: [[u8; 4]; 48],
    pub left_leg: [[u8; 4]; 48],
}

impl Default for SkinPreviewPixels {
    fn default() -> Self {
        Self {
            head: [[0; 4]; 64],
            body: [[0; 4]; 96],
            right_arm: [[0; 4]; 48],
            left_arm: [[0; 4]; 48],
            right_leg: [[0; 4]; 48],
            left_leg: [[0; 4]; 48],
        }
    }
}

impl PlayerSkin {
    pub fn load(path: impl AsRef<Path>) -> image::ImageResult<Self> {
        let img = image::open(path)?.to_rgba8();
        Self::from_rgba(img)
    }

    pub fn from_bytes(bytes: &[u8]) -> image::ImageResult<Self> {
        let img = image::load_from_memory(bytes)?.to_rgba8();
        Self::from_rgba(img)
    }

    fn from_rgba(img: RgbaImage) -> image::ImageResult<Self> {
        let (w, h) = img.dimensions();
        let layout = if w == 64 && h == 32 {
            SkinLayout::Classic64x32
        } else {
            SkinLayout::Modern64x64
        };
        let mut pixels = if layout == SkinLayout::Classic64x32 {
            expand_classic_skin(&img)
        } else {
            img
        };
        normalize_skin_alpha(&mut pixels);
        let slim = detect_slim_from_pixels(&pixels);
        Ok(Self {
            pixels,
            layout,
            slim_arms: slim,
        })
    }

    pub fn with_slim_arms(mut self, slim: bool) -> Self {
        self.slim_arms = slim;
        self
    }

    /// Composite a cape PNG (64×32, front face at 1,1–10,16) into the
    /// skin texture.  The cape model uses box_uvs(0, 32, …) so the back
    /// face is at (0,32) and the front face at (10,32) — the second
    /// layer strip (y≥32) where cape pixels won't overwrite the face.
    pub fn composite_cape(&mut self, cape_pixels: &RgbaImage) {
        let cap_w = 10u32;
        let cap_h = 16u32;
        for offset_x in [0, 10] {
            let dst_x = offset_x;
            let dst_y = 32;
            for y in 0..cap_h {
                for x in 0..cap_w {
                    if x + 1 >= cape_pixels.width() || y + 1 >= cape_pixels.height() {
                        continue;
                    }
                    let src = cape_pixels.get_pixel(x + 1, y + 1);
                    if src[3] > 0 {
                        self.pixels.put_pixel(dst_x + x, dst_y + y, *src);
                    }
                }
            }
        }
    }

    pub fn default_steve() -> Self {
        let mut pixels = RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        fill_rect(&mut pixels, 8, 8, 8, 8, [189, 140, 102, 255]);
        fill_rect(&mut pixels, 8, 8, 8, 2, [55, 38, 25, 255]);
        fill_rect(&mut pixels, 10, 12, 1, 1, [35, 55, 85, 255]);
        fill_rect(&mut pixels, 13, 12, 1, 1, [35, 55, 85, 255]);
        fill_rect(&mut pixels, 11, 14, 2, 1, [110, 55, 45, 255]);
        // Clear hat overlay region (40,8) so face_pixels composite doesn't override with white
        fill_rect(&mut pixels, 40, 8, 8, 8, [0, 0, 0, 0]);
        // Clear body overlay (20,36) so preview_pixels composite works correctly
        fill_rect(&mut pixels, 20, 36, 16, 16, [0, 0, 0, 0]);
        Self {
            pixels,
            layout: SkinLayout::Modern64x64,
            slim_arms: false,
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.pixels.dimensions()
    }

    pub fn face_pixels(&self) -> [[u8; 4]; 64] {
        let mut out = [[255, 255, 255, 255]; 64];
        for y in 0..8 {
            for x in 0..8 {
                let base = self.sample(8 + x, 8 + y);
                let overlay = self.sample(40 + x, 8 + y);
                out[(y * 8 + x) as usize] = composite(base, overlay);
            }
        }
        out
    }

    pub fn preview_pixels(&self) -> SkinPreviewPixels {
        SkinPreviewPixels {
            head: self.region_with_overlay::<64>(8, 8, 8, 8, Some((40, 8))),
            body: self.region_with_overlay::<96>(
                8,
                12,
                20,
                20,
                matches!(self.layout, SkinLayout::Modern64x64).then_some((20, 36)),
            ),
            right_arm: self.region_with_overlay::<48>(
                4,
                12,
                44,
                20,
                matches!(self.layout, SkinLayout::Modern64x64).then_some((44, 36)),
            ),
            left_arm: if matches!(self.layout, SkinLayout::Modern64x64) {
                self.region_with_overlay::<48>(4, 12, 36, 52, Some((52, 52)))
            } else {
                self.region_with_overlay::<48>(4, 12, 44, 20, None)
            },
            right_leg: self.region_with_overlay::<48>(
                4,
                12,
                4,
                20,
                matches!(self.layout, SkinLayout::Modern64x64).then_some((4, 36)),
            ),
            left_leg: if matches!(self.layout, SkinLayout::Modern64x64) {
                self.region_with_overlay::<48>(4, 12, 20, 52, Some((4, 52)))
            } else {
                self.region_with_overlay::<48>(4, 12, 4, 20, None)
            },
        }
    }

    fn region_with_overlay<const N: usize>(
        &self,
        w: usize,
        h: usize,
        x: u32,
        y: u32,
        overlay: Option<(u32, u32)>,
    ) -> [[u8; 4]; N] {
        let mut out = [[0, 0, 0, 0]; N];
        for py in 0..h {
            for px in 0..w {
                let base = self.sample(x + px as u32, y + py as u32);
                let p = if let Some((ox, oy)) = overlay {
                    composite(base, self.sample(ox + px as u32, oy + py as u32))
                } else {
                    base
                };
                out[py * w + px] = p;
            }
        }
        out
    }

    pub fn sample(&self, x: u32, y: u32) -> [u8; 4] {
        let (w, h) = self.pixels.dimensions();
        if x >= w || y >= h {
            [0, 0, 0, 0]
        } else {
            self.pixels.get_pixel(x, y).0
        }
    }
}

fn composite(base: [u8; 4], overlay: [u8; 4]) -> [u8; 4] {
    let a = overlay[3] as f32 / 255.0;
    if a <= 0.0 {
        return base;
    }
    let inv = 1.0 - a;
    [
        (overlay[0] as f32 * a + base[0] as f32 * inv).round() as u8,
        (overlay[1] as f32 * a + base[1] as f32 * inv).round() as u8,
        (overlay[2] as f32 * a + base[2] as f32 * inv).round() as u8,
        255,
    ]
}

fn detect_slim_from_pixels(pixels: &RgbaImage) -> bool {
    if pixels.width() < 56 || pixels.height() < 32 {
        return false;
    }
    // Alex slim model: right arm is 3 px wide instead of 4 px.
    // The rightmost column of the right arm base (x=54-55, y=16-31)
    // is transparent for slim, opaque for classic.
    let opaques = (16u32..32)
        .flat_map(|y| (54u32..56).map(move |x| (x, y)))
        .filter(|(x, y)| pixels.get_pixel(*x, *y)[3] >= 128)
        .count();
    opaques < 4
}

fn fill_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
    for py in y..y + h {
        for px in x..x + w {
            img.put_pixel(px, py, image::Rgba(color));
        }
    }
}

fn expand_classic_skin(source: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::new(64, 64);
    image::imageops::overlay(&mut out, source, 0, 0);

    // ImageBufferDownload.parseUserSkin mirrors the old right limbs into the
    // modern left-limb regions.
    for &(sx, sy, w, h, dx, dy) in &[
        (4, 16, 4, 4, 20, 48),
        (8, 16, 4, 4, 24, 48),
        (8, 20, 4, 12, 16, 52),
        (4, 20, 4, 12, 20, 52),
        (0, 20, 4, 12, 24, 52),
        (12, 20, 4, 12, 28, 52),
        (44, 16, 4, 4, 36, 48),
        (48, 16, 4, 4, 40, 48),
        (48, 20, 4, 12, 32, 52),
        (44, 20, 4, 12, 36, 52),
        (40, 20, 4, 12, 40, 52),
        (52, 20, 4, 12, 44, 52),
    ] {
        copy_rect_mirrored_x(source, &mut out, sx, sy, w, h, dx, dy);
    }
    out
}

fn copy_rect_mirrored_x(
    source: &RgbaImage,
    target: &mut RgbaImage,
    sx: u32,
    sy: u32,
    w: u32,
    h: u32,
    dx: u32,
    dy: u32,
) {
    for y in 0..h {
        for x in 0..w {
            let pixel = *source.get_pixel(sx + x, sy + y);
            target.put_pixel(dx + w - 1 - x, dy + y, pixel);
        }
    }
}

fn normalize_skin_alpha(image: &mut RgbaImage) {
    set_area_opaque(image, 0, 0, 32, 16);
    set_area_transparent_if_fully_opaque(image, 32, 0, 64, 32);
    set_area_opaque(image, 0, 16, 64, 32);
    set_area_transparent_if_fully_opaque(image, 0, 32, 16, 48);
    set_area_transparent_if_fully_opaque(image, 16, 32, 40, 48);
    set_area_transparent_if_fully_opaque(image, 40, 32, 56, 48);
    set_area_transparent_if_fully_opaque(image, 0, 48, 16, 64);
    set_area_opaque(image, 16, 48, 48, 64);
    set_area_transparent_if_fully_opaque(image, 48, 48, 64, 64);
}

fn set_area_opaque(image: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..y1.min(image.height()) {
        for x in x0..x1.min(image.width()) {
            image.get_pixel_mut(x, y)[3] = 255;
        }
    }
}

fn set_area_transparent_if_fully_opaque(image: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) {
    let has_transparency = (y0..y1.min(image.height()))
        .any(|y| (x0..x1.min(image.width())).any(|x| image.get_pixel(x, y)[3] < 128));
    if has_transparency {
        return;
    }
    for y in y0..y1.min(image.height()) {
        for x in x0..x1.min(image.width()) {
            image.get_pixel_mut(x, y)[3] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_skin_regions_are_always_opaque() {
        let image = RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 32]));
        let skin = PlayerSkin::from_rgba(image).unwrap();
        assert_eq!(skin.sample(8, 8)[3], 255);
        assert_eq!(skin.sample(20, 20)[3], 255);
        assert_eq!(skin.sample(20, 52)[3], 255);
    }

    #[test]
    fn classic_skin_expands_the_left_limbs() {
        let mut image = RgbaImage::from_pixel(64, 32, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(4, 16, image::Rgba([255, 0, 0, 255]));
        let skin = PlayerSkin::from_rgba(image).unwrap();
        assert_eq!(skin.dimensions(), (64, 64));
        assert_eq!(skin.sample(23, 48)[0], 255);
    }
}
