//! First-person animation API boundary.

use mlua::{Lua, Table, UserData, UserDataMethods};
use nalgebra::{Matrix4, Rotation3, Unit, Vector3};
use std::cell::RefCell;
use std::rc::Rc;

use crate::render::first_person::{
    AnimationOverrides, FirstPersonAnimationContext, ItemType, ReequipPolicy, UseAction,
    VanillaTransformFlags,
};

pub const READ_PERMISSION: &str = "animation.read";
pub const MODIFY_PERMISSION: &str = "animation.modify";

#[derive(Clone, Debug)]
pub struct ScriptTransform {
    matrix: Rc<RefCell<Matrix4<f32>>>,
    stage_entry: Matrix4<f32>,
}

impl Default for ScriptTransform {
    fn default() -> Self {
        Self {
            matrix: Rc::new(RefCell::new(Matrix4::identity())),
            stage_entry: Matrix4::identity(),
        }
    }
}

impl ScriptTransform {
    pub fn new(stage_entry: Matrix4<f32>) -> Self {
        Self {
            matrix: Rc::new(RefCell::new(Matrix4::identity())),
            stage_entry,
        }
    }

    pub fn matrix(&self) -> Matrix4<f32> {
        *self.matrix.borrow()
    }

    fn prepend(&self, operation: Matrix4<f32>) {
        let mut matrix = self.matrix.borrow_mut();
        *matrix = operation * *matrix;
    }

    fn append(&self, operation: Matrix4<f32>) {
        let mut matrix = self.matrix.borrow_mut();
        *matrix = *matrix * operation;
    }
}

impl UserData for ScriptTransform {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Resets to the stage entry matrix (backward compatible).
        methods.add_method("reset", |_, this, ()| {
            *this.matrix.borrow_mut() = this.stage_entry;
            Ok(())
        });
        methods.add_method("reset_to_stage", |_, this, ()| {
            *this.matrix.borrow_mut() = this.stage_entry;
            Ok(())
        });
        methods.add_method("set_identity", |_, this, ()| {
            *this.matrix.borrow_mut() = Matrix4::identity();
            Ok(())
        });
        methods.add_method("translate", |_, this, (x, y, z): (f32, f32, f32)| {
            validate_components(&[x, y, z])?;
            this.prepend(Matrix4::new_translation(&Vector3::new(x, y, z)));
            Ok(())
        });
        methods.add_method("local_translate", |_, this, (x, y, z): (f32, f32, f32)| {
            validate_components(&[x, y, z])?;
            this.append(Matrix4::new_translation(&Vector3::new(x, y, z)));
            Ok(())
        });
        methods.add_method("rotate_x", |_, this, degrees: f32| {
            validate_components(&[degrees])?;
            this.prepend(
                Rotation3::from_axis_angle(&Vector3::x_axis(), degrees.to_radians())
                    .to_homogeneous(),
            );
            Ok(())
        });
        methods.add_method("local_rotate_x", |_, this, degrees: f32| {
            validate_components(&[degrees])?;
            this.append(
                Rotation3::from_axis_angle(&Vector3::x_axis(), degrees.to_radians())
                    .to_homogeneous(),
            );
            Ok(())
        });
        methods.add_method("rotate_y", |_, this, degrees: f32| {
            validate_components(&[degrees])?;
            this.prepend(
                Rotation3::from_axis_angle(&Vector3::y_axis(), degrees.to_radians())
                    .to_homogeneous(),
            );
            Ok(())
        });
        methods.add_method("local_rotate_y", |_, this, degrees: f32| {
            validate_components(&[degrees])?;
            this.append(
                Rotation3::from_axis_angle(&Vector3::y_axis(), degrees.to_radians())
                    .to_homogeneous(),
            );
            Ok(())
        });
        methods.add_method("rotate_z", |_, this, degrees: f32| {
            validate_components(&[degrees])?;
            this.prepend(
                Rotation3::from_axis_angle(&Vector3::z_axis(), degrees.to_radians())
                    .to_homogeneous(),
            );
            Ok(())
        });
        methods.add_method("local_rotate_z", |_, this, degrees: f32| {
            validate_components(&[degrees])?;
            this.append(
                Rotation3::from_axis_angle(&Vector3::z_axis(), degrees.to_radians())
                    .to_homogeneous(),
            );
            Ok(())
        });
        methods.add_method(
            "rotate_axis",
            |_, this, (x, y, z, degrees): (f32, f32, f32, f32)| {
                validate_components(&[x, y, z, degrees])?;
                let axis = Unit::try_new(Vector3::new(x, y, z), 1.0e-6).ok_or_else(|| {
                    mlua::Error::RuntimeError("rotation axis must be non-zero".into())
                })?;
                this.prepend(
                    Rotation3::from_axis_angle(&axis, degrees.to_radians()).to_homogeneous(),
                );
                Ok(())
            },
        );
        methods.add_method(
            "local_rotate_axis",
            |_, this, (x, y, z, degrees): (f32, f32, f32, f32)| {
                validate_components(&[x, y, z, degrees])?;
                let axis = Unit::try_new(Vector3::new(x, y, z), 1.0e-6).ok_or_else(|| {
                    mlua::Error::RuntimeError("rotation axis must be non-zero".into())
                })?;
                this.append(
                    Rotation3::from_axis_angle(&axis, degrees.to_radians()).to_homogeneous(),
                );
                Ok(())
            },
        );
        methods.add_method("scale", |_, this, (x, y, z): (f32, f32, f32)| {
            validate_components(&[x, y, z])?;
            if x.abs() < 1.0e-6 || y.abs() < 1.0e-6 || z.abs() < 1.0e-6 {
                return Err(mlua::Error::RuntimeError(
                    "transform scale components must be non-zero".into(),
                ));
            }
            this.prepend(nalgebra::Scale3::new(x, y, z).to_homogeneous());
            Ok(())
        });
        methods.add_method("local_scale", |_, this, (x, y, z): (f32, f32, f32)| {
            validate_components(&[x, y, z])?;
            if x.abs() < 1.0e-6 || y.abs() < 1.0e-6 || z.abs() < 1.0e-6 {
                return Err(mlua::Error::RuntimeError(
                    "transform scale components must be non-zero".into(),
                ));
            }
            this.append(nalgebra::Scale3::new(x, y, z).to_homogeneous());
            Ok(())
        });
        methods.add_method("mul", |_, this, values: Table| {
            let mut components = [0.0f32; 16];
            for (index, component) in components.iter_mut().enumerate() {
                *component = values.get(index + 1)?;
            }
            validate_components(&components)?;
            this.prepend(Matrix4::from_column_slice(&components));
            Ok(())
        });
        methods.add_method("local_mul", |_, this, values: Table| {
            let mut components = [0.0f32; 16];
            for (index, component) in components.iter_mut().enumerate() {
                *component = values.get(index + 1)?;
            }
            validate_components(&components)?;
            this.append(Matrix4::from_column_slice(&components));
            Ok(())
        });
    }
}

fn validate_components(values: &[f32]) -> mlua::Result<()> {
    if values
        .iter()
        .all(|value| value.is_finite() && value.abs() <= 1.0e6)
    {
        Ok(())
    } else {
        Err(mlua::Error::RuntimeError(
            "transform components must be finite and within +/-1000000".into(),
        ))
    }
}

pub fn event_table(
    lua: &Lua,
    event_name: &str,
    context: &FirstPersonAnimationContext,
    transform: ScriptTransform,
) -> mlua::Result<Table> {
    let event = lua.create_table()?;
    event.set("name", event_name)?;
    event.set("hand", context.hand.as_str())?;
    event.set("item_id", context.item_id.as_str())?;
    event.set("numeric_item_id", context.numeric_item_id)?;
    event.set("item_type", context.item_type.as_str())?;
    event.set("use_action", context.use_action.as_str())?;
    event.set("transform", transform)?;

    let state = lua.create_table()?;
    state.set("item_type", context.item_type.as_str())?;
    state.set("use_action", context.use_action.as_str())?;
    state.set("equip_progress", context.equip_progress)?;
    state.set("previous_equip_progress", context.previous_equip_progress)?;
    state.set("swing_progress", context.swing_progress)?;
    state.set("previous_swing_progress", context.previous_swing_progress)?;
    state.set("swinging", context.swinging)?;
    state.set("swing_duration_ticks", context.swing_duration_ticks)?;
    state.set("use_progress", context.use_progress)?;
    state.set("use_ticks", context.use_ticks)?;
    state.set("remaining_use_ticks", context.remaining_use_ticks)?;
    state.set("max_use_ticks", context.max_use_ticks)?;
    state.set("attack_cooldown", context.attack_cooldown)?;
    state.set("using_item", context.using_item)?;
    state.set("blocking", context.blocking)?;
    state.set("attack_pressed", context.attack_pressed)?;
    state.set("attack_held", context.attack_held)?;
    state.set("use_pressed", context.use_pressed)?;
    state.set("use_held", context.use_held)?;
    state.set("sneaking", context.sneaking)?;
    state.set("yaw", context.yaw)?;
    state.set("pitch", context.pitch)?;
    state.set("partial_tick", context.partial_tick)?;
    state.set("fov", context.fov)?;
    state.set("aspect_ratio", context.aspect_ratio)?;
    event.set("state", state)?;
    Ok(event)
}

/// Installed only on `animation.first_person.calculate`. Provides setters for animation overrides.
pub fn event_table_calculate(
    lua: &Lua,
    context: &FirstPersonAnimationContext,
    overrides: Rc<RefCell<AnimationOverrides>>,
    transform: ScriptTransform,
) -> mlua::Result<Table> {
    let event = event_table(lua, "animation.first_person.calculate", context, transform)?;

    let overrides_clone = overrides.clone();
    event.set(
        "set_swing_progress",
        lua.create_function(move |_, (_self, value): (Table, f32)| {
            validate_finite(value, "swing_progress")?;
            overrides_clone.borrow_mut().swing_progress = Some(value.clamp(0.0, 1.0));
            Ok(())
        })?,
    )?;

    let overrides_swinging = overrides.clone();
    event.set(
        "set_swinging",
        lua.create_function(move |_, (_self, value): (Table, bool)| {
            overrides_swinging.borrow_mut().swinging = Some(value);
            Ok(())
        })?,
    )?;

    let overrides_dur = overrides.clone();
    event.set(
        "set_swing_duration_ticks",
        lua.create_function(move |_, (_self, ticks): (Table, u16)| {
            if ticks < 1 || ticks > 200 {
                return Err(mlua::Error::RuntimeError(
                    "swing_duration_ticks must be between 1 and 200".into(),
                ));
            }
            overrides_dur.borrow_mut().swing_duration_ticks = Some(ticks);
            Ok(())
        })?,
    )?;

    let overrides_ep = overrides.clone();
    event.set(
        "set_equip_progress",
        lua.create_function(move |_, (_self, value): (Table, f32)| {
            validate_finite(value, "equip_progress")?;
            overrides_ep.borrow_mut().equip_progress = Some(value.clamp(0.0, 1.0));
            Ok(())
        })?,
    )?;

    let overrides_up = overrides.clone();
    event.set(
        "set_use_progress",
        lua.create_function(move |_, (_self, value): (Table, f32)| {
            validate_finite(value, "use_progress")?;
            overrides_up.borrow_mut().use_progress = Some(value.clamp(0.0, 1.0));
            Ok(())
        })?,
    )?;

    let overrides_blocking = overrides.clone();
    event.set(
        "set_blocking",
        lua.create_function(move |_, (_self, value): (Table, bool)| {
            overrides_blocking.borrow_mut().blocking = Some(value);
            Ok(())
        })?,
    )?;

    let overrides_ui = overrides.clone();
    event.set(
        "set_using_item",
        lua.create_function(move |_, (_self, value): (Table, bool)| {
            overrides_ui.borrow_mut().using_item = Some(value);
            Ok(())
        })?,
    )?;

    // Reequip policy setter (no overrides capture needed)
    event.set(
        "set_reequip_policy",
        lua.create_function(move |_, (_self, value): (Table, String)| {
            match value.as_str() {
                "vanilla" | "always" | "skip_same_item" | "skip_same_slot" | "never" => Ok(()),
                _ => Err(mlua::Error::RuntimeError(
                    "reequip policy must be 'vanilla', 'always', 'skip_same_item', 'skip_same_slot', or 'never'".into(),
                ))
            }
        })?,
    )?;

    // Vanilla transform control
    event.set("vanilla", vanilla_transform_table(lua, overrides)?)?;

    Ok(event)
}

/// Creates the `event.vanilla` control table.
pub fn vanilla_transform_table(
    lua: &Lua,
    overrides: Rc<RefCell<AnimationOverrides>>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let flags = [
        ("base", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.base = value;
                Ok(())
            })?
        }),
        ("equip", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.equip = value;
                Ok(())
            })?
        }),
        ("swing", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.swing = value;
                Ok(())
            })?
        }),
        ("use", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.use_transform = value;
                Ok(())
            })?
        }),
        ("block", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.block_transform = value;
                Ok(())
            })?
        }),
        ("bow", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.bow_transform = value;
                Ok(())
            })?
        }),
        ("eat_drink", {
            let o = overrides.clone();
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                o.borrow_mut().vanilla.eat_drink_transform = value;
                Ok(())
            })?
        }),
        ("bob", {
            lua.create_function(move |_, (_self, value): (Table, bool)| {
                overrides.borrow_mut().vanilla.bob = value;
                Ok(())
            })?
        }),
    ];

    for (name, func) in flags {
        table.set(format!("set_{name}_enabled"), func)?;
    }

    Ok(table)
}

fn validate_finite(value: f32, name: &str) -> mlua::Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(mlua::Error::RuntimeError(format!(
            "{name} must be a finite number"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_call_order_is_application_order() {
        let lua = Lua::new();
        let transform = ScriptTransform::default();
        lua.globals().set("transform", transform.clone()).unwrap();
        lua.load(
            r#"
                transform:translate(1, 0, 0)
                transform:rotate_z(90)
                transform:scale(2, 2, 2)
            "#,
        )
        .exec()
        .unwrap();
        let point = transform.matrix() * nalgebra::Vector4::new(0.0, 0.0, 0.0, 1.0);
        assert!((point.x - 0.0).abs() < 1.0e-5);
        assert!((point.y - 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn reset_returns_to_identity_by_default() {
        let transform = ScriptTransform::default();
        let lua = Lua::new();
        lua.globals().set("t", transform.clone()).unwrap();
        lua.load(r#"t:translate(5, 5, 5); t:reset()"#)
            .exec()
            .unwrap();
        assert_eq!(transform.matrix(), Matrix4::identity());
    }

    #[test]
    fn reset_to_stage_preserves_entry_point() {
        let entry = Matrix4::new_translation(&Vector3::new(3.0f32, 0.0f32, 0.0f32));
        let transform = ScriptTransform::new(entry);
        let lua = Lua::new();
        lua.globals().set("t", transform.clone()).unwrap();
        lua.load(r#"t:translate(2, 0, 0); t:reset_to_stage()"#)
            .exec()
            .unwrap();
        let mat = transform.matrix();
        assert!(
            (mat[(0, 3)] - 3.0f32).abs() < 1.0e-5,
            "stage_entry not restored"
        );
    }

    #[test]
    fn set_identity_always_gives_unit_matrix() {
        let entry = Matrix4::new_translation(&Vector3::new(3.0f32, 0.0f32, 0.0f32));
        let transform = ScriptTransform::new(entry);
        let lua = Lua::new();
        lua.globals().set("t", transform.clone()).unwrap();
        lua.load(r#"t:translate(2, 0, 0); t:set_identity()"#)
            .exec()
            .unwrap();
        assert_eq!(transform.matrix(), Matrix4::identity());
    }

    #[test]
    fn translate_and_local_translate_use_different_spaces() {
        let lua = Lua::new();
        let parent = ScriptTransform::default();
        let local = ScriptTransform::default();
        lua.globals().set("parent", parent.clone()).unwrap();
        lua.globals().set("local_transform", local.clone()).unwrap();
        lua.load(
            r#"
                parent:rotate_z(90)
                parent:translate(1, 0, 0)
                local_transform:rotate_z(90)
                local_transform:local_translate(1, 0, 0)
            "#,
        )
        .exec()
        .unwrap();

        let origin = nalgebra::Vector4::new(0.0, 0.0, 0.0, 1.0);
        let parent_point = parent.matrix() * origin;
        let local_point = local.matrix() * origin;
        assert!((parent_point.x - 1.0).abs() < 1.0e-5);
        assert!(parent_point.y.abs() < 1.0e-5);
        assert!(local_point.x.abs() < 1.0e-5);
        assert!((local_point.y - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn rotate_y_and_local_rotate_y_use_different_spaces() {
        let lua = Lua::new();
        let parent = ScriptTransform::default();
        let local = ScriptTransform::default();
        lua.globals().set("parent", parent.clone()).unwrap();
        lua.globals().set("local_transform", local.clone()).unwrap();
        lua.load(
            r#"
                parent:translate(2, 0, 0)
                parent:rotate_y(90)
                local_transform:translate(2, 0, 0)
                local_transform:local_rotate_y(90)
            "#,
        )
        .exec()
        .unwrap();

        let origin = nalgebra::Vector4::new(0.0, 0.0, 0.0, 1.0);
        let parent_point = parent.matrix() * origin;
        let local_point = local.matrix() * origin;
        assert!(parent_point.x.abs() < 1.0e-5);
        assert!((parent_point.z + 2.0).abs() < 1.0e-5);
        assert!((local_point.x - 2.0).abs() < 1.0e-5);
        assert!(local_point.z.abs() < 1.0e-5);
    }

    #[test]
    fn local_rotation_uses_item_axes_after_first_person_base_matrix() {
        let base = Matrix4::new_translation(&Vector3::new(0.56, -0.52, -0.72))
            * Rotation3::from_axis_angle(&Vector3::y_axis(), 45.0_f32.to_radians())
                .to_homogeneous();
        let transform = ScriptTransform::new(base);
        let lua = Lua::new();
        lua.globals().set("t", transform.clone()).unwrap();
        lua.load("t:reset_to_stage(); t:local_rotate_x(90)")
            .exec()
            .unwrap();

        let local_rotation =
            Rotation3::from_axis_angle(&Vector3::x_axis(), 90.0_f32.to_radians()).to_homogeneous();
        let expected = base * local_rotation;
        assert!((transform.matrix() - expected).abs().max() < 1.0e-5);

        let anchor = transform.matrix() * nalgebra::Vector4::new(0.0, 0.0, 0.0, 1.0);
        assert!((anchor.x - 0.56).abs() < 1.0e-5);
        assert!((anchor.y + 0.52).abs() < 1.0e-5);
        assert!((anchor.z + 0.72).abs() < 1.0e-5);
    }

    #[test]
    fn local_rotate_axis_rejects_zero_axis_without_panicking() {
        let lua = Lua::new();
        lua.globals().set("t", ScriptTransform::default()).unwrap();
        let error = lua
            .load("t:local_rotate_axis(0, 0, 0, 45)")
            .exec()
            .unwrap_err();
        assert!(error.to_string().contains("rotation axis must be non-zero"));
    }

    #[test]
    fn local_transforms_reject_non_finite_components() {
        let scripts = [
            "t:local_translate(math.huge, 0, 0)",
            "t:local_rotate_x(math.huge)",
            "t:local_rotate_y(math.huge)",
            "t:local_rotate_z(math.huge)",
            "t:local_rotate_axis(1, 0, 0, math.huge)",
            "t:local_scale(1, math.huge, 1)",
            concat!(
                "t:local_mul({math.huge, 0, 0, 0, ",
                "0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1})"
            ),
        ];

        for script in scripts {
            let lua = Lua::new();
            lua.globals().set("t", ScriptTransform::default()).unwrap();
            assert!(lua.load(script).exec().is_err(), "accepted: {script}");
        }
    }
}
