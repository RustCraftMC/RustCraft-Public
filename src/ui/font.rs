//! Font rendering using `fontdue` for TTF/OTF fonts with CJK support.

use fontdue::Font;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    ch: char,
    size_16: u32,
}

impl GlyphKey {
    fn new(ch: char, size: f32) -> Self {
        Self {
            ch,
            size_16: (size.max(1.0) * 16.0).round() as u32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GlyphInfo {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub width: u32,
    pub height: u32,
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance: f32,
}

pub struct FontRenderer {
    pub font: Font,
    pub base_size: f32,
    pub atlas_pixels: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    atlas_cursor_x: u32,
    atlas_cursor_y: u32,
    max_row_height: u32,
    glyphs: HashMap<GlyphKey, GlyphInfo>,
    /// True when atlas_pixels has been modified and needs GPU re-upload
    pub atlas_dirty: bool,
}

impl FontRenderer {
    pub fn new() -> Self {
        let font = Font::from_bytes(EMBEDDED_FONT, fontdue::FontSettings::default())
            .expect("Failed to load embedded font");

        FontRenderer {
            font,
            base_size: 16.0,
            atlas_pixels: vec![0u8; 2048 * 2048 * 4],
            atlas_width: 2048,
            atlas_height: 2048,
            atlas_cursor_x: 0,
            atlas_cursor_y: 0,
            max_row_height: 0,
            glyphs: HashMap::new(),
            atlas_dirty: false,
        }
    }

    /// Render a glyph (or retrieve cached). Returns UV info for the atlas.
    pub fn get_or_render(&mut self, ch: char, size: f32) -> GlyphInfo {
        let key = GlyphKey::new(ch, size);
        if let Some(info) = self.glyphs.get(&key) {
            return info.clone();
        }

        let (metrics, bitmap) = self.font.rasterize(ch, size);
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;

        if gw == 0 || gh == 0 {
            let info = GlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: 0,
                height: 0,
                bearing_x: 0,
                bearing_y: 0,
                advance: metrics.advance_width,
            };
            self.glyphs.insert(key, info.clone());
            return info;
        }

        if self.atlas_cursor_x + gw > self.atlas_width {
            self.atlas_cursor_x = 0;
            self.atlas_cursor_y += self.max_row_height + 2;
            self.max_row_height = 0;
        }

        if self.atlas_cursor_y + gh > self.atlas_height {
            return GlyphInfo {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: 0,
                height: 0,
                bearing_x: 0,
                bearing_y: 0,
                advance: metrics.advance_width,
            };
        }

        let ax = self.atlas_cursor_x;
        let ay = self.atlas_cursor_y;

        for py in 0..gh as usize {
            for px in 0..gw as usize {
                let src = py * gw as usize + px;
                let alpha = if src < bitmap.len() { bitmap[src] } else { 0 };
                let dst_x = ax + px as u32;
                let dst_y = ay + py as u32;
                let dst = ((dst_y * self.atlas_width + dst_x) * 4) as usize;
                if dst + 3 < self.atlas_pixels.len() {
                    self.atlas_pixels[dst] = 255;
                    self.atlas_pixels[dst + 1] = 255;
                    self.atlas_pixels[dst + 2] = 255;
                    self.atlas_pixels[dst + 3] = alpha;
                }
            }
        }

        let info = GlyphInfo {
            uv_min: [
                ax as f32 / self.atlas_width as f32,
                ay as f32 / self.atlas_height as f32,
            ],
            uv_max: [
                (ax + gw) as f32 / self.atlas_width as f32,
                (ay + gh) as f32 / self.atlas_height as f32,
            ],
            width: gw,
            height: gh,
            bearing_x: metrics.xmin,
            bearing_y: metrics.ymin,
            advance: metrics.advance_width,
        };

        self.glyphs.insert(key, info.clone());
        self.atlas_cursor_x += gw + 1;
        self.max_row_height = self.max_row_height.max(gh);
        self.atlas_dirty = true;

        info
    }

    /// Distance from a line's top edge to its baseline at the requested size.
    pub fn ascent(&self, size: f32) -> f32 {
        self.font
            .horizontal_line_metrics(size)
            .map(|metrics| metrics.ascent)
            .unwrap_or(size * 0.8)
    }

    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        let mut total = 0.0f32;
        let mut chars = text.chars();
        while let Some(ch) = chars.next() {
            if ch == '\u{00a7}' {
                chars.next();
                continue;
            }
            total += self
                .glyphs
                .get(&GlyphKey::new(ch, size))
                .map(|glyph| glyph.advance)
                .unwrap_or_else(|| self.font.metrics(ch, size).advance_width);
        }
        total
    }

    pub fn line_height(&self, _size: f32) -> f32 {
        // fontdue doesn't have line_height, use a reasonable default
        self.base_size * 1.4
    }

    /// Pre-populate the atlas with ASCII printable characters at common sizes.
    /// Call once at startup so the atlas is ready before first frame.
    pub fn preload_ascii(&mut self) {
        let common_sizes = [18.0, 24.0, 27.0, 36.0];
        for size in common_sizes {
            for ch in ' '..='~' {
                self.get_or_render(ch, size);
            }
        }
        let cjk = "主菜单游戏设置选项退出返回断开连接语言中文英文单人多人服务器地址保存\
            选择资源包可用已选打开文件夹将放此处正在连接加载世界中重新连接回主菜单断开\
            计分板快捷栏生命值饥饿度经验值护甲耐久攻击伤害移动速度\
            由于一些弱智把库存金偷了因此我们永久停止发放井盖欢迎加入测试服的官方Q群\
            可以使用指令来获取物品地面上的所有物品将会在秒后被清除";
        let cjk_sizes = [18.0, 24.0, 27.0, 36.0, 40.0];
        for size in cjk_sizes {
            for ch in cjk.chars() {
                self.get_or_render(ch, size);
            }
        }
        // Preload digits and punctuation at all common sizes for sign rendering
        let common = "0123456789.:!?, ";
        for size in cjk_sizes {
            for ch in common.chars() {
                self.get_or_render(ch, size);
            }
        }
        self.atlas_dirty = true;
    }
}

/// Embedded font data (loaded from assets/fonts/default.ttf at compile time).
const EMBEDDED_FONT: &[u8] = include_bytes!("../../assets/fonts/default.ttf");

#[cfg(test)]
mod tests {
    use super::FontRenderer;

    #[test]
    fn text_width_matches_the_advances_used_to_draw_uncached_cjk() {
        let renderer = FontRenderer::new();
        let size = 18.0;
        let text = "CCBlueX的测试服务";
        let expected: f32 = text
            .chars()
            .map(|ch| renderer.font.metrics(ch, size).advance_width)
            .sum();

        assert!((renderer.text_width(text, size) - expected).abs() < 0.001);
        assert!((renderer.text_width("§bCCBlueX§f的测试服务", size) - expected).abs() < 0.001);
    }
}
