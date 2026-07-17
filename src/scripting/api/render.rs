//! Command-buffer-free rendering API boundary.

use mlua::{Lua, Table, UserData, UserDataMethods};
use std::cell::RefCell;
use std::rc::Rc;

use crate::render::hooks::{ScriptColor, ScriptDrawCommand, ScriptFrameContext};

pub const READ_PERMISSION: &str = "render.read";
pub const MODIFY_PERMISSION: &str = "render.modify";
pub const CUSTOM_DRAW_PERMISSION: &str = "render.custom_draw";

#[derive(Clone)]
pub struct ScriptDrawContext {
    commands: Rc<RefCell<Vec<ScriptDrawCommand>>>,
    allowed: bool,
}

impl ScriptDrawContext {
    pub fn new(commands: Rc<RefCell<Vec<ScriptDrawCommand>>>, allowed: bool) -> Self {
        Self { commands, allowed }
    }

    fn push(&self, command: ScriptDrawCommand) -> mlua::Result<()> {
        if !self.allowed {
            return Err(mlua::Error::RuntimeError(
                "permission 'render.custom_draw' is required".into(),
            ));
        }
        if self.commands.borrow().len() >= 4096 {
            return Err(mlua::Error::RuntimeError(
                "per-frame draw command limit exceeded (4096)".into(),
            ));
        }
        self.commands.borrow_mut().push(command);
        Ok(())
    }
}

impl UserData for ScriptDrawContext {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("text", |_, this, options: Table| {
            let text: String = options.get("text")?;
            if text.len() > 4096 {
                return Err(mlua::Error::RuntimeError(
                    "draw text exceeds 4096 bytes".into(),
                ));
            }
            this.push(ScriptDrawCommand::Text {
                text,
                x: finite(options.get("x")?)?,
                y: finite(options.get("y")?)?,
                scale: finite(options.get::<Option<f32>>("scale")?.unwrap_or(1.0))?
                    .clamp(0.1, 16.0),
                color: read_color(options.get::<Option<Table>>("color")?)?,
            })
        });
        methods.add_method("rect", |_, this, options: Table| {
            this.push(ScriptDrawCommand::Rect {
                x: finite(options.get("x")?)?,
                y: finite(options.get("y")?)?,
                width: finite(options.get("width")?)?,
                height: finite(options.get("height")?)?,
                color: read_color(options.get::<Option<Table>>("color")?)?,
            })
        });
        methods.add_method("line", |_, this, options: Table| {
            this.push(ScriptDrawCommand::Line {
                x1: finite(options.get("x1")?)?,
                y1: finite(options.get("y1")?)?,
                x2: finite(options.get("x2")?)?,
                y2: finite(options.get("y2")?)?,
                width: finite(options.get::<Option<f32>>("width")?.unwrap_or(1.0))?
                    .clamp(0.1, 128.0),
                color: read_color(options.get::<Option<Table>>("color")?)?,
            })
        });
        methods.add_method("crosshair", |_, this, options: Table| {
            this.push(ScriptDrawCommand::Crosshair {
                x_offset: finite(options.get::<Option<f32>>("x_offset")?.unwrap_or(0.0))?,
                y_offset: finite(options.get::<Option<f32>>("y_offset")?.unwrap_or(0.0))?,
                size: finite(options.get::<Option<f32>>("size")?.unwrap_or(4.0))?.clamp(0.0, 256.0),
                gap: finite(options.get::<Option<f32>>("gap")?.unwrap_or(1.0))?.clamp(0.0, 256.0),
                thickness: finite(options.get::<Option<f32>>("thickness")?.unwrap_or(1.0))?
                    .clamp(0.1, 128.0),
                color: read_color(options.get::<Option<Table>>("color")?)?,
            })
        });
        methods.add_method("image", |_, this, options: Table| {
            let resource: String = options.get("resource")?;
            validate_resource_id(&resource)?;
            this.push(ScriptDrawCommand::Image {
                resource,
                x: finite(options.get("x")?)?,
                y: finite(options.get("y")?)?,
                width: finite(options.get("width")?)?,
                height: finite(options.get("height")?)?,
                color: read_color(options.get::<Option<Table>>("color")?)?,
            })
        });
        methods.add_method("push_transform", |_, this, ()| {
            this.push(ScriptDrawCommand::PushTransform)
        });
        methods.add_method("pop_transform", |_, this, ()| {
            this.push(ScriptDrawCommand::PopTransform)
        });
        methods.add_method("translate", |_, this, (x, y): (f32, f32)| {
            this.push(ScriptDrawCommand::Translate {
                x: finite(x)?,
                y: finite(y)?,
            })
        });
        methods.add_method("rotate", |_, this, degrees: f32| {
            this.push(ScriptDrawCommand::Rotate {
                degrees: finite(degrees)?,
            })
        });
        methods.add_method("scale", |_, this, (x, y): (f32, f32)| {
            this.push(ScriptDrawCommand::Scale {
                x: finite(x)?,
                y: finite(y)?,
            })
        });
        methods.add_method("set_scissor", |_, this, options: Option<Table>| {
            let rect = options
                .map(|options| {
                    Ok::<[f32; 4], mlua::Error>([
                        finite(options.get("x")?)?,
                        finite(options.get("y")?)?,
                        finite(options.get("width")?)?,
                        finite(options.get("height")?)?,
                    ])
                })
                .transpose()?;
            this.push(ScriptDrawCommand::SetScissor(rect))
        });
    }
}

pub fn event_table(
    lua: &Lua,
    event_name: &str,
    frame: ScriptFrameContext,
    draw: ScriptDrawContext,
) -> mlua::Result<Table> {
    let event = lua.create_table()?;
    event.set("name", event_name)?;
    let frame_table = lua.create_table()?;
    frame_table.set("delta_time", frame.delta_time)?;
    frame_table.set("viewport_width", frame.viewport_width)?;
    frame_table.set("viewport_height", frame.viewport_height)?;
    event.set("frame", frame_table)?;
    event.set("draw", draw)?;
    Ok(event)
}

fn finite(value: f32) -> mlua::Result<f32> {
    if value.is_finite() && value.abs() <= 1.0e6 {
        Ok(value)
    } else {
        Err(mlua::Error::RuntimeError(
            "draw values must be finite and within +/-1000000".into(),
        ))
    }
}

fn read_color(table: Option<Table>) -> mlua::Result<ScriptColor> {
    let Some(table) = table else {
        return Ok(ScriptColor::WHITE);
    };
    Ok(ScriptColor {
        r: finite(table.get::<Option<f32>>("r")?.unwrap_or(1.0))?.clamp(0.0, 1.0),
        g: finite(table.get::<Option<f32>>("g")?.unwrap_or(1.0))?.clamp(0.0, 1.0),
        b: finite(table.get::<Option<f32>>("b")?.unwrap_or(1.0))?.clamp(0.0, 1.0),
        a: finite(table.get::<Option<f32>>("a")?.unwrap_or(1.0))?.clamp(0.0, 1.0),
    })
}

fn validate_resource_id(resource: &str) -> mlua::Result<()> {
    let Some((namespace, path)) = resource.split_once(':') else {
        return Err(mlua::Error::RuntimeError(
            "resource must be a namespaced id such as 'example:textures/gui/icon.png'".into(),
        ));
    };
    let valid = !namespace.is_empty()
        && !path.is_empty()
        && !path.contains("..")
        && namespace.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        });
    if valid {
        Ok(())
    } else {
        Err(mlua::Error::RuntimeError(
            "resource id contains invalid characters".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosshair_draw_command_is_center_relative_and_bounded() {
        let lua = Lua::new();
        let commands = Rc::new(RefCell::new(Vec::new()));
        let draw = ScriptDrawContext::new(commands.clone(), true);
        let event = event_table(
            &lua,
            "render.hud.after",
            ScriptFrameContext {
                delta_time: 0.016,
                viewport_width: 1280,
                viewport_height: 720,
            },
            draw,
        )
        .unwrap();
        lua.globals().set("event", &event).unwrap();

        lua.load("event.draw:crosshair({x_offset=3, y_offset=-2, size=6, gap=2, thickness=1.5})")
            .call::<()>(event)
            .unwrap();

        assert_eq!(
            commands.borrow().as_slice(),
            [ScriptDrawCommand::Crosshair {
                x_offset: 3.0,
                y_offset: -2.0,
                size: 6.0,
                gap: 2.0,
                thickness: 1.5,
                color: ScriptColor::WHITE,
            }]
        );
    }
}
