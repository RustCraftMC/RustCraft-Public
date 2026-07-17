//! Biome color maps — grass and foliage coloring.
//!
//! MC uses a 256×256 color lookup table (textures/colormap/grass.png).
//! The color is determined by biome temperature (X axis) and humidity (Y axis).
//! The grass_top texture is grayscale; the final color = texture × biome_color.

use image::GenericImageView;

/// Grass color map (256×256 RGBA).
pub struct ColorMap {
    pixels: Vec<u8>, // 256*256*4 RGBA
    width: u32,
    height: u32,
}

impl ColorMap {
    pub fn load(path: &str) -> Option<Self> {
        let img = image::open(path).ok()?;
        let (w, h) = img.dimensions();
        let rgba = img.to_rgba8().into_raw();
        Some(ColorMap {
            pixels: rgba,
            width: w,
            height: h,
        })
    }

    /// Look up color by temperature (0..1) and humidity (0..1).
    /// Returns [r, g, b] in 0..1 range.
    pub fn sample(&self, temperature: f32, humidity: f32) -> [f32; 3] {
        let t = temperature.clamp(0.0, 1.0);
        let h = (humidity * t).clamp(0.0, 1.0); // MC formula: humidity *= temperature
        let x = ((1.0 - t) * (self.width - 1) as f32) as u32;
        let y = ((1.0 - h) * (self.height - 1) as f32) as u32;
        let idx = ((y * self.width + x) * 4) as usize;
        if idx + 3 < self.pixels.len() {
            [
                self.pixels[idx] as f32 / 255.0,
                self.pixels[idx + 1] as f32 / 255.0,
                self.pixels[idx + 2] as f32 / 255.0,
            ]
        } else {
            [1.0, 1.0, 1.0] // fallback white
        }
    }

    /// Default plains biome color (temperature=0.8, humidity=0.4).
    pub fn plains_grass(&self) -> [f32; 3] {
        self.sample(0.8, 0.4)
    }

    /// Desert biome color (temperature=1.0, humidity=0.0).
    pub fn desert_grass(&self) -> [f32; 3] {
        self.sample(1.0, 0.0)
    }

    /// Forest biome color (temperature=0.7, humidity=0.8).
    pub fn forest_grass(&self) -> [f32; 3] {
        self.sample(0.7, 0.8)
    }

    /// Taiga/snowy biome color (temperature=0.05, humidity=0.8).
    pub fn taiga_grass(&self) -> [f32; 3] {
        self.sample(0.05, 0.8)
    }
}
