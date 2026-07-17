//! Read-only local-player snapshots exposed with `client.read`.

use mlua::{Lua, Table, Value};

use super::context::{
    PlayerActionSnapshot, PlayerCapabilitiesSnapshot, PlayerExperienceSnapshot,
    PlayerMovementSnapshot, PlayerRotationSnapshot, PlayerSnapshot, PlayerVitalsSnapshot,
    SharedApiContext, Vec3Snapshot,
};

pub const READ_PERMISSION: &str = "client.read";

pub fn install(
    lua: &Lua,
    game: &Table,
    context: SharedApiContext,
    can_read: bool,
) -> mlua::Result<()> {
    if !can_read {
        return Ok(());
    }

    let player = lua.create_table()?;

    let exists_context = context.clone();
    player.set(
        "exists",
        lua.create_function(move |_, ()| Ok(exists_context.snapshot().player.is_some()))?,
    )?;

    let snapshot_context = context.clone();
    player.set(
        "snapshot",
        lua.create_function(move |lua, ()| {
            optional_player_table(lua, snapshot_context.snapshot().player.as_ref())
        })?,
    )?;

    let position_context = context.clone();
    player.set(
        "position",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, position_context.snapshot().player, |player| {
                vec3_table(lua, &player.position)
            })
        })?,
    )?;

    let velocity_context = context.clone();
    player.set(
        "velocity",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, velocity_context.snapshot().player, |player| {
                vec3_table(lua, &player.velocity)
            })
        })?,
    )?;

    let rotation_context = context.clone();
    player.set(
        "rotation",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, rotation_context.snapshot().player, |player| {
                rotation_table(lua, &player.rotation)
            })
        })?,
    )?;

    let movement_context = context.clone();
    player.set(
        "movement",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, movement_context.snapshot().player, |player| {
                movement_table(lua, &player.movement)
            })
        })?,
    )?;

    let action_context = context.clone();
    player.set(
        "action",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, action_context.snapshot().player, |player| {
                action_table(lua, &player.action)
            })
        })?,
    )?;

    let capabilities_context = context.clone();
    player.set(
        "capabilities",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, capabilities_context.snapshot().player, |player| {
                capabilities_table(lua, &player.capabilities)
            })
        })?,
    )?;

    player.set(
        "vitals",
        lua.create_function(move |lua, ()| {
            optional_projected_table(lua, context.snapshot().player, |player| {
                vitals_table(lua, &player.vitals)
            })
        })?,
    )?;

    game.set("player", player)
}

fn optional_player_table(lua: &Lua, player: Option<&PlayerSnapshot>) -> mlua::Result<Value> {
    match player {
        Some(player) => Ok(Value::Table(player_snapshot_table(lua, player)?)),
        None => Ok(Value::Nil),
    }
}

fn optional_projected_table<F>(
    _lua: &Lua,
    player: Option<PlayerSnapshot>,
    project: F,
) -> mlua::Result<Value>
where
    F: FnOnce(&PlayerSnapshot) -> mlua::Result<Table>,
{
    match player {
        Some(player) => Ok(Value::Table(project(&player)?)),
        None => Ok(Value::Nil),
    }
}

pub(crate) fn player_snapshot_table(lua: &Lua, player: &PlayerSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("entity_id", player.entity_id)?;
    table.set("name", player.name.clone())?;
    table.set("gamemode", player.gamemode)?;
    table.set("gamemode_name", gamemode_name(player.gamemode))?;
    table.set("dimension", player.dimension)?;
    table.set("position", vec3_table(lua, &player.position)?)?;
    table.set(
        "previous_position",
        vec3_table(lua, &player.previous_position)?,
    )?;
    table.set("velocity", vec3_table(lua, &player.velocity)?)?;
    table.set("rotation", rotation_table(lua, &player.rotation)?)?;
    table.set("movement", movement_table(lua, &player.movement)?)?;
    table.set("action", action_table(lua, &player.action)?)?;
    table.set(
        "capabilities",
        capabilities_table(lua, &player.capabilities)?,
    )?;
    table.set("vitals", vitals_table(lua, &player.vitals)?)?;
    table.set("experience", experience_table(lua, &player.experience)?)?;
    table.set("selected_hotbar_slot", player.selected_hotbar_slot)?;
    Ok(table)
}

fn vec3_table(lua: &Lua, vector: &Vec3Snapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("x", vector.x)?;
    table.set("y", vector.y)?;
    table.set("z", vector.z)?;
    Ok(table)
}

fn rotation_table(lua: &Lua, rotation: &PlayerRotationSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("yaw", rotation.yaw)?;
    table.set("pitch", rotation.pitch)?;
    table.set("body_yaw", rotation.body_yaw)?;
    table.set("head_yaw", rotation.head_yaw)?;
    Ok(table)
}

fn movement_table(lua: &Lua, movement: &PlayerMovementSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("on_ground", movement.on_ground)?;
    table.set("collided_horizontally", movement.collided_horizontally)?;
    table.set("sneaking", movement.sneaking)?;
    table.set("sprinting", movement.sprinting)?;
    table.set("jumping", movement.jumping)?;
    table.set("in_water", movement.in_water)?;
    table.set("in_lava", movement.in_lava)?;
    table.set("fall_distance", movement.fall_distance)?;
    table.set("input_strafe", movement.input_strafe)?;
    table.set("input_forward", movement.input_forward)?;
    Ok(table)
}

fn action_table(lua: &Lua, action: &PlayerActionSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("using_item", action.using_item)?;
    table.set("use_action", action.use_action.clone())?;
    table.set("use_ticks", action.use_ticks)?;
    table.set("blocking", action.blocking)?;
    table.set("swinging", action.swinging)?;
    table.set("swing_progress", action.swing_progress)?;
    Ok(table)
}

fn capabilities_table(lua: &Lua, capabilities: &PlayerCapabilitiesSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("invulnerable", capabilities.invulnerable)?;
    table.set("creative_mode", capabilities.creative_mode)?;
    table.set("allow_flying", capabilities.allow_flying)?;
    table.set("flying", capabilities.flying)?;
    table.set("walk_speed", capabilities.walk_speed)?;
    table.set("fly_speed", capabilities.fly_speed)?;
    Ok(table)
}

fn vitals_table(lua: &Lua, vitals: &PlayerVitalsSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("health", vitals.health)?;
    table.set("max_health", vitals.max_health)?;
    table.set("absorption", vitals.absorption)?;
    table.set("food", vitals.food)?;
    table.set("saturation", vitals.saturation)?;
    table.set("oxygen", vitals.oxygen)?;
    Ok(table)
}

fn experience_table(lua: &Lua, experience: &PlayerExperienceSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("level", experience.level)?;
    table.set("progress", experience.progress)?;
    table.set("total", experience.total)?;
    Ok(table)
}

fn gamemode_name(gamemode: u8) -> &'static str {
    match gamemode & 0x07 {
        0 => "survival",
        1 => "creative",
        2 => "adventure",
        3 => "spectator",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::super::context::{ClientSnapshot, PlayerVitalsSnapshot};
    use super::*;

    fn installed(context: SharedApiContext) -> Lua {
        let lua = Lua::new();
        let game = lua.create_table().unwrap();
        install(&lua, &game, context, true).unwrap();
        lua.globals().set("game", game).unwrap();
        lua
    }

    #[test]
    fn absent_player_is_reported_as_nil_without_a_stale_table() {
        let context = SharedApiContext::default();
        let lua = installed(context.clone());
        assert!(!lua
            .load("return game.player.exists()")
            .eval::<bool>()
            .unwrap());
        assert!(matches!(
            lua.load("return game.player.snapshot()")
                .eval::<Value>()
                .unwrap(),
            Value::Nil
        ));

        context.update_snapshot(ClientSnapshot {
            player: Some(PlayerSnapshot {
                position: [12.0, 64.0, -3.0].into(),
                vitals: PlayerVitalsSnapshot {
                    health: 7.5,
                    ..PlayerVitalsSnapshot::default()
                },
                ..PlayerSnapshot::default()
            }),
            ..ClientSnapshot::default()
        });
        let (x, health): (f64, f32) = lua
            .load("local p = game.player.snapshot(); return p.position.x, p.vitals.health")
            .eval()
            .unwrap();
        assert_eq!((x, health), (12.0, 7.5));

        context.update_snapshot(ClientSnapshot::default());
        assert!(!lua
            .load("return game.player.exists()")
            .eval::<bool>()
            .unwrap());
    }
}
