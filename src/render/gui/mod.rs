//! GUI rendering helpers — vertex generation for MC-style buttons/text.
//!
//! The actual Vulkan draw calls are in the main renderer.
//! This module handles building the vertex/index buffers.

mod runtime;
pub mod widgets;

use crate::ui::font::FontRenderer;

/// GUI vertex: position(2f) + uv(2f) + color(4f) = 32 bytes
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GuiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl GuiVertex {
    pub const STRIDE: u32 = std::mem::size_of::<GuiVertex>() as u32;
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GuiUniforms {
    pub screen_size: [f32; 2],
}

impl GuiUniforms {
    pub fn new(w: f32, h: f32) -> Self {
        Self {
            screen_size: [w, h],
        }
    }
}

/// A clickable button's screen-space rectangle for hit testing.
#[derive(Clone, Copy, Debug)]
pub struct ButtonHit {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Frame-level GUI vertex builder
pub struct GuiVertexBuilder {
    pub vertices: Vec<GuiVertex>,
    pub indices: Vec<u32>,
    pub button_hits: Vec<ButtonHit>,
    content_generation: u64,
}

impl GuiVertexBuilder {
    fn next_content_generation() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
        NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
    }

    pub fn new() -> Self {
        GuiVertexBuilder {
            vertices: Vec::new(),
            indices: Vec::new(),
            button_hits: Vec::new(),
            content_generation: Self::next_content_generation(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        GuiVertexBuilder {
            vertices: Vec::with_capacity(cap),
            indices: Vec::with_capacity(cap * 3 / 2),
            button_hits: Vec::with_capacity(16),
            content_generation: Self::next_content_generation(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.button_hits.clear();
        self.content_generation = Self::next_content_generation();
    }

    pub(crate) fn content_generation(&self) -> u64 {
        self.content_generation
    }

    /// Register a clickable button region. Call after drawing the button background.
    pub fn register_button(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32) {
        self.button_hits.push(ButtonHit { id, x, y, w, h });
    }

    /// Test if a screen-space point (mx, my) hits any registered button.
    pub fn hit_test(&self, mx: f32, my: f32) -> Option<u32> {
        for btn in self.button_hits.iter().rev() {
            if mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h {
                return Some(btn.id);
            }
        }
        None
    }

    /// Add a textured quad (screen coords: -1..1)
    pub fn add_quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_x: f32,
        uv_y: f32,
        uv_w: f32,
        uv_h: f32,
        color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(GuiVertex {
            pos: [x, y],
            uv: [uv_x, uv_y],
            color,
        });
        self.vertices.push(GuiVertex {
            pos: [x + w, y],
            uv: [uv_x + uv_w, uv_y],
            color,
        });
        self.vertices.push(GuiVertex {
            pos: [x + w, y + h],
            uv: [uv_x + uv_w, uv_y + uv_h],
            color,
        });
        self.vertices.push(GuiVertex {
            pos: [x, y + h],
            uv: [uv_x, uv_y + uv_h],
            color,
        });
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Draw a solid colored rectangle
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.fill_rect_gradient(x, y, w, h, color, color);
    }

    /// Draw a vertical-gradient rectangle (vanilla `Gui.drawGradientRect`).
    pub fn fill_rect_gradient(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        top_color: [f32; 4],
        bottom_color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(GuiVertex {
            pos: [x, y],
            uv: [-1.0, -1.0],
            color: top_color,
        });
        self.vertices.push(GuiVertex {
            pos: [x + w, y],
            uv: [-1.0, -1.0],
            color: top_color,
        });
        self.vertices.push(GuiVertex {
            pos: [x + w, y + h],
            uv: [-1.0, -1.0],
            color: bottom_color,
        });
        self.vertices.push(GuiVertex {
            pos: [x, y + h],
            uv: [-1.0, -1.0],
            color: bottom_color,
        });
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Full-screen vanilla `GuiScreen.drawWorldBackground` gradient
    /// (0xC0101010 -> 0xD0101010).
    pub fn fill_world_background(&mut self, w: f32, h: f32) {
        self.fill_rect_gradient(
            0.0,
            0.0,
            w,
            h,
            [16.0 / 255.0, 16.0 / 255.0, 16.0 / 255.0, 192.0 / 255.0],
            [16.0 / 255.0, 16.0 / 255.0, 16.0 / 255.0, 208.0 / 255.0],
        );
    }

    pub fn draw_line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: [f32; 4]) {
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.001 {
            return;
        }
        let nx = -dy / len * width * 0.5;
        let ny = dx / len * width * 0.5;
        self.fill_quad(
            from[0] + nx,
            from[1] + ny,
            to[0] + nx,
            to[1] + ny,
            to[0] - nx,
            to[1] - ny,
            from[0] - nx,
            from[1] - ny,
            color,
        );
    }

    pub fn draw_pixel_face(&mut self, x: f32, y: f32, pixel_size: f32, pixels: &[[u8; 4]; 64]) {
        for py in 0..8 {
            for px in 0..8 {
                let p = pixels[py * 8 + px];
                if p[3] == 0 {
                    continue;
                }
                self.fill_rect(
                    x + px as f32 * pixel_size,
                    y + py as f32 * pixel_size,
                    pixel_size,
                    pixel_size,
                    [
                        p[0] as f32 / 255.0,
                        p[1] as f32 / 255.0,
                        p[2] as f32 / 255.0,
                        p[3] as f32 / 255.0,
                    ],
                );
            }
        }
    }

    pub fn draw_pixel_region<const N: usize>(
        &mut self,
        x: f32,
        y: f32,
        w: usize,
        h: usize,
        pixel_size: f32,
        pixels: &[[u8; 4]; N],
    ) {
        for py in 0..h {
            for px in 0..w {
                let p = pixels[py * w + px];
                if p[3] == 0 {
                    continue;
                }
                self.fill_rect(
                    x + px as f32 * pixel_size,
                    y + py as f32 * pixel_size,
                    pixel_size,
                    pixel_size,
                    [
                        p[0] as f32 / 255.0,
                        p[1] as f32 / 255.0,
                        p[2] as f32 / 255.0,
                        p[3] as f32 / 255.0,
                    ],
                );
            }
        }
    }

    /// Draw a quadrilateral from 4 corner points (p0-p1-p2-p3).
    pub fn fill_quad(
        &mut self,
        p0x: f32,
        p0y: f32,
        p1x: f32,
        p1y: f32,
        p2x: f32,
        p2y: f32,
        p3x: f32,
        p3y: f32,
        color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(GuiVertex {
            pos: [p0x, p0y],
            uv: [-1.0, -1.0],
            color,
        });
        self.vertices.push(GuiVertex {
            pos: [p1x, p1y],
            uv: [-1.0, -1.0],
            color,
        });
        self.vertices.push(GuiVertex {
            pos: [p2x, p2y],
            uv: [-1.0, -1.0],
            color,
        });
        self.vertices.push(GuiVertex {
            pos: [p3x, p3y],
            uv: [-1.0, -1.0],
            color,
        });
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Draw a textured quadrilateral from 4 corner points and matching UVs.
    pub fn textured_quad(&mut self, points: [[f32; 2]; 4], uvs: [[f32; 2]; 4], color: [f32; 4]) {
        let base = self.vertices.len() as u32;
        for i in 0..4 {
            self.vertices.push(GuiVertex {
                pos: points[i],
                uv: uvs[i],
                color,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Draw MC-style button using widgets.png texture (256×256)
    /// Button is 200×20 pixels in the texture.
    /// `x,y,w,h` in NDC coordinates (-1..1).
    pub fn draw_button(&mut self, x: f32, y: f32, w: f32, h: f32, state: u32) {
        let tex_h = 20.0 / 256.0; // button texture height in UV
        let half_tex_w = 100.0 / 256.0; // half button (100px) in UV
        let state_v = (46.0 + state as f32 * 20.0) / 256.0; // Y offset in UV

        let half_screen_w = w / 2.0;

        // Left half of button (texture x: 0..100, screen x: 0..half)
        self.add_quad(
            x,
            y,
            half_screen_w,
            h,
            0.0,
            state_v,
            half_tex_w,
            tex_h,
            [1.0, 1.0, 1.0, 1.0],
        );
        // Right half of button (texture x: (200-100)=100..200, screen x: half..full)
        self.add_quad(
            x + half_screen_w,
            y,
            half_screen_w,
            h,
            (200.0 - 100.0) / 256.0,
            state_v,
            half_tex_w,
            tex_h,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    /// Draw text in pixel coordinates using the font atlas texture.
    /// Supports Minecraft § color codes (§0-§f) and §r reset.
    pub fn draw_text(
        &mut self,
        font: &mut FontRenderer,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        color: [f32; 4],
    ) {
        // Callers supply a line's top edge. Derive the baseline from font
        // metrics so ASCII and CJK glyphs share the same vertical anchor.
        let baseline = y + font.ascent(size);
        let mut cursor_x = x;
        let mut cur_color = color;
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{00a7}' {
                if let Some(&code) = chars.peek() {
                    chars.next();
                    cur_color = match code {
                        '0'..='9' | 'a'..='f' | 'A'..='F' => Self::mc_color(code),
                        'r' | 'R' => color,
                        _ => cur_color,
                    };
                }
                continue;
            }
            let glyph = font.get_or_render(ch, size);
            if glyph.width > 0 && glyph.height > 0 {
                let gx = cursor_x + glyph.bearing_x as f32;
                let gy = baseline - glyph.bearing_y as f32 - glyph.height as f32;
                self.add_quad(
                    gx,
                    gy,
                    glyph.width as f32,
                    glyph.height as f32,
                    glyph.uv_min[0],
                    glyph.uv_min[1],
                    glyph.uv_max[0] - glyph.uv_min[0],
                    glyph.uv_max[1] - glyph.uv_min[1],
                    cur_color,
                );
            }
            cursor_x += glyph.advance;
        }
    }

    /// Map MC § color code to RGBA.
    fn mc_color(code: char) -> [f32; 4] {
        match code {
            '0' => [0.0, 0.0, 0.0, 1.0],
            '1' => [0.0, 0.0, 0.67, 1.0],
            '2' => [0.0, 0.67, 0.0, 1.0],
            '3' => [0.0, 0.67, 0.67, 1.0],
            '4' => [0.67, 0.0, 0.0, 1.0],
            '5' => [0.67, 0.0, 0.67, 1.0],
            '6' => [1.0, 0.67, 0.0, 1.0],
            '7' => [0.67, 0.67, 0.67, 1.0],
            '8' => [0.33, 0.33, 0.33, 1.0],
            '9' => [0.33, 0.33, 1.0, 1.0],
            'a' | 'A' => [0.33, 1.0, 0.33, 1.0],
            'b' | 'B' => [0.33, 1.0, 1.0, 1.0],
            'c' | 'C' => [1.0, 0.33, 0.33, 1.0],
            'd' | 'D' => [1.0, 0.33, 1.0, 1.0],
            'e' | 'E' => [1.0, 1.0, 0.33, 1.0],
            'f' | 'F' => [1.0, 1.0, 1.0, 1.0],
            _ => [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Draw centered text using font atlas. cx = center x, y = top of text.
    pub fn draw_text_centered(
        &mut self,
        font: &mut FontRenderer,
        cx: f32,
        y: f32,
        text: &str,
        size: f32,
        color: [f32; 4],
    ) {
        let mut text_w = 0.0f32;
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{00a7}' {
                chars.next();
                continue;
            }
            text_w += font.get_or_render(ch, size).advance;
        }
        self.draw_text(font, cx - text_w / 2.0, y, text, size, color);
    }

    /// Draw centered UI text without a drop shadow.
    pub fn draw_text_shadowed(
        &mut self,
        font: &mut FontRenderer,
        cx: f32,
        y: f32,
        text: &str,
        size: f32,
        color: [f32; 4],
        _gui_scale: f32,
    ) {
        self.draw_text_centered(font, cx, y, text, size, color);
    }

    /// Draw MC-style button using widgets.png texture.
    /// `x,y,w,h` in pixels. MC button = 200×20 px from widgets.png.
    /// State: 0=normal, 1=hovered, 2=disabled.
    pub fn draw_button_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.draw_button_rect_state(x, y, w, h, 0);
    }

    pub fn draw_button_rect_state(&mut self, x: f32, y: f32, w: f32, h: f32, state: u32) {
        let u_scale = 1.0 / 256.0;
        let v_scale = 1.0 / 256.0;
        // MC 1.8.9 widgets.png button states:
        //   0 = normal: Y=66, 1 = hovered: Y=86, 2 = disabled: Y=46
        let tex_y = match state {
            0 => 66.0,
            1 => 86.0,
            2 => 46.0,
            _ => 66.0,
        };
        let scale = (h / 20.0).max(0.5);
        let border_x = (4.0 * scale).min(w * 0.5);
        let border_y = (2.0 * scale).min(h * 0.5);
        let columns = [
            (x, border_x, 0.0, 4.0),
            (x + border_x, (w - border_x * 2.0).max(0.0), 4.0, 192.0),
            (x + w - border_x, border_x, 196.0, 4.0),
        ];
        let rows = [
            (y, border_y, tex_y, 2.0),
            (
                y + border_y,
                (h - border_y * 2.0).max(0.0),
                tex_y + 2.0,
                16.0,
            ),
            (y + h - border_y, border_y, tex_y + 18.0, 2.0),
        ];
        for (dst_y, dst_h, src_y, src_h) in rows {
            for (dst_x, dst_w, src_x, src_w) in columns {
                if dst_w > 0.0 && dst_h > 0.0 {
                    self.add_quad(
                        dst_x,
                        dst_y,
                        dst_w,
                        dst_h,
                        src_x * u_scale,
                        src_y * v_scale,
                        src_w * u_scale,
                        src_h * v_scale,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
            }
        }
    }

    /// Draw a vanilla-style horizontal slider using widgets.png.
    pub fn draw_slider_rect_state(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        value: f32,
        thumb_hovered: u32,
    ) {
        // The track is fixed at V=46. Only the movable thumb changes material
        // while hovered, keeping the slider's background visually stable.
        self.draw_button_rect_state(x, y, w, h, 2);

        let u_scale = 1.0 / 256.0;
        let v_scale = 1.0 / 256.0;
        // GuiOptionSlider always draws its 8 px thumb from (0, 66), even
        // while the underlying button is hovered. Only the button background
        // changes hover state.
        let tex_y = if thumb_hovered == 1 { 86.0 } else { 66.0 } * v_scale;
        let tex_h = 20.0 * v_scale;
        let gs = (h / 20.0).max(0.5);
        let knob_w = 8.0 * gs;
        let knob_x = x + (w - knob_w).max(0.0) * value.clamp(0.0, 1.0);
        let half = knob_w * 0.5;

        self.add_quad(
            knob_x,
            y,
            half,
            h,
            0.0,
            tex_y,
            4.0 * u_scale,
            tex_h,
            [1.0, 1.0, 1.0, 1.0],
        );
        self.add_quad(
            knob_x + half,
            y,
            half,
            h,
            196.0 * u_scale,
            tex_y,
            4.0 * u_scale,
            tex_h,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    /// Draw hotbar background from widgets.png (182×22 at 0,0).
    pub fn draw_hotbar_bg(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let u0 = 0.0 / 256.0;
        let v0 = 0.0 / 256.0;
        let u1 = 182.0 / 256.0;
        let v1 = 22.0 / 256.0;
        self.add_quad(x, y, w, h, u0, v0, u1 - u0, v1 - v0, [1.0, 1.0, 1.0, 1.0]);
    }

    /// Draw hotbar selection box from widgets.png (24×24 at 0,22).
    pub fn draw_hotbar_select(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let u0 = 0.0 / 256.0;
        let v0 = 22.0 / 256.0;
        let u1 = 24.0 / 256.0;
        let v1 = 46.0 / 256.0;
        self.add_quad(x, y, w, h, u0, v0, u1 - u0, v1 - v0, [1.0, 1.0, 1.0, 1.0]);
    }
}
