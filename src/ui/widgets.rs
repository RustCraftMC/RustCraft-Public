//! Minecraft-style GUI system — buttons, labels, progress bars.
//!
//! Based on MC 1.8.9 `GuiButton`, `GuiLabel`, `GuiSlot`, etc.
//! Uses `textures/gui/widgets.png` for button textures and `icons.png` for HUD.
//!
//! Button texture layout (256x256 widgets.png):
//!   - Standard button: left half (0,46) right half (100,46), 200x20
//!   - Disabled: Y offset +40 (46+20*2=86)
//!   - Hovered: Y offset +20 (46+20=66)
//!   - Normal: Y offset 0 (46)
//!   - For custom widths: left side is 0..100/200, right side is (200-w/2)..200

/// GUI widget types
#[derive(Debug)]
pub enum Widget {
    Button {
        id: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: String,
        enabled: bool,
    },
    Label {
        x: f32,
        y: f32,
        text: String,
        scale: f32,
        color: [f32; 4],
    },
    ProgressBar {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        progress: f32, // 0..1
        label: String,
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex_x: f32,
        tex_y: f32,
        tex_w: f32,
        tex_h: f32,
    },
}

impl Widget {
    pub fn button(id: u32, x: f32, y: f32, w: f32, h: f32, text: &str) -> Self {
        Widget::Button {
            id,
            x,
            y,
            w,
            h,
            text: text.to_string(),
            enabled: true,
        }
    }

    pub fn label(x: f32, y: f32, text: &str, scale: f32, color: [f32; 4]) -> Self {
        Widget::Label {
            x,
            y,
            text: text.to_string(),
            scale,
            color,
        }
    }

    pub fn centered_button(id: u32, y: f32, w: f32, h: f32, text: &str) -> Self {
        Widget::Button {
            id,
            x: -w / 2.0,
            y,
            w,
            h,
            text: text.to_string(),
            enabled: true,
        }
    }
}

/// MC-style GUI screen
pub struct GuiScreen {
    pub widgets: Vec<Widget>,
    pub background: GuiBackground,
}

#[derive(Clone, Copy)]
pub enum GuiBackground {
    /// Title screen panorama (6-face cubemap)
    Panorama,
    /// Options background (tilable stone texture)
    Options,
    /// Solid color
    Solid([f32; 4]),
    /// Transparent (no background)
    Transparent,
}

impl GuiScreen {
    pub fn new(background: GuiBackground) -> Self {
        GuiScreen {
            widgets: Vec::new(),
            background,
        }
    }

    pub fn add_widget(&mut self, widget: Widget) {
        self.widgets.push(widget);
    }

    /// Get all rendered elements for this screen
    pub fn elements(&self, i18n: &crate::ui::i18n::I18n) -> Vec<GuiElement> {
        let mut elems = Vec::new();

        match self.background {
            GuiBackground::Options => {
                // Tile the options background
                elems.push(GuiElement {
                    kind: GuiElementKind::BackgroundTiled,
                    x: -1.0,
                    y: -1.0,
                    w: 2.0,
                    h: 2.0,
                });
            }
            GuiBackground::Panorama => {
                elems.push(GuiElement {
                    kind: GuiElementKind::BackgroundPanorama,
                    x: -1.0,
                    y: -1.0,
                    w: 2.0,
                    h: 2.0,
                });
                // Dark overlay
                elems.push(GuiElement {
                    kind: GuiElementKind::Rect([0.0, 0.0, 0.0, 0.65]),
                    x: -1.0,
                    y: -1.0,
                    w: 2.0,
                    h: 2.0,
                });
            }
            GuiBackground::Solid(c) => {
                elems.push(GuiElement {
                    kind: GuiElementKind::Rect(c),
                    x: -1.0,
                    y: -1.0,
                    w: 2.0,
                    h: 2.0,
                });
            }
            GuiBackground::Transparent => {}
        }

        for w in &self.widgets {
            match w {
                Widget::Button {
                    id: _,
                    x,
                    y,
                    w: bw,
                    h: bh,
                    text,
                    enabled,
                } => {
                    let display = i18n.t(text);
                    elems.push(GuiElement {
                        kind: GuiElementKind::Button {
                            text: display,
                            width: *bw,
                            height: *bh,
                            hovered: false, // TODO: mouse hover
                            enabled: *enabled,
                        },
                        x: *x,
                        y: *y,
                        w: *bw,
                        h: *bh,
                    });
                }
                Widget::Label {
                    x,
                    y,
                    text,
                    scale,
                    color,
                } => {
                    elems.push(GuiElement {
                        kind: GuiElementKind::Text {
                            text: text.clone(),
                            size: *scale,
                            color: *color,
                        },
                        x: *x,
                        y: *y,
                        w: 0.0,
                        h: 0.0,
                    });
                }
                Widget::ProgressBar {
                    x,
                    y,
                    w,
                    h,
                    progress,
                    label: _,
                } => {
                    elems.push(GuiElement {
                        kind: GuiElementKind::ProgressBar {
                            progress: *progress,
                        },
                        x: *x,
                        y: *y,
                        w: *w,
                        h: *h,
                    });
                }
                Widget::Image {
                    x,
                    y,
                    w,
                    h,
                    tex_x,
                    tex_y,
                    tex_w,
                    tex_h,
                } => {
                    elems.push(GuiElement {
                        kind: GuiElementKind::Image {
                            tex_x: *tex_x,
                            tex_y: *tex_y,
                            tex_w: *tex_w,
                            tex_h: *tex_h,
                        },
                        x: *x,
                        y: *y,
                        w: *w,
                        h: *h,
                    });
                }
            }
        }

        elems
    }
}

/// Rendered GUI element
#[derive(Clone, Debug)]
pub struct GuiElement {
    pub kind: GuiElementKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug)]
pub enum GuiElementKind {
    /// Solid rectangle
    Rect([f32; 4]),
    /// MC-style button (from widgets.png texture)
    Button {
        text: String,
        width: f32,
        height: f32,
        hovered: bool,
        enabled: bool,
    },
    /// Text
    Text {
        text: String,
        size: f32,
        color: [f32; 4],
    },
    /// Progress bar
    ProgressBar { progress: f32 },
    /// Textured quad from a GUI texture
    Image {
        tex_x: f32,
        tex_y: f32,
        tex_w: f32,
        tex_h: f32,
    },
    /// Background (tiled stone texture)
    BackgroundTiled,
    /// Background (panorama)
    BackgroundPanorama,
}
