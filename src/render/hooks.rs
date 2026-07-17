//! Renderer-neutral command types produced by scripts for the current frame only.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ScriptColor {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub fn array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScriptDrawCommand {
    Text {
        text: String,
        x: f32,
        y: f32,
        scale: f32,
        color: ScriptColor,
    },
    Image {
        resource: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: ScriptColor,
    },
    Crosshair {
        x_offset: f32,
        y_offset: f32,
        size: f32,
        gap: f32,
        thickness: f32,
        color: ScriptColor,
    },
    PushTransform,
    PopTransform,
    Translate {
        x: f32,
        y: f32,
    },
    Rotate {
        degrees: f32,
    },
    Scale {
        x: f32,
        y: f32,
    },
    SetScissor(Option<[f32; 4]>),
}

#[derive(Clone, Copy, Debug)]
pub struct ScriptFrameContext {
    pub delta_time: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
}
