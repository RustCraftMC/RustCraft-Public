//! Permission-trimmed client snapshot and safe visual command API.

use mlua::{Lua, Table};

use super::context::{
    CameraMode, ClientCommand, ClientSettingsSnapshot, ClientSnapshot, ConnectionSnapshot,
    SharedApiContext, WindowSnapshot,
};

pub const READ_PERMISSION: &str = "client.read";
pub const MODIFY_PERMISSION: &str = "client.modify";

/// Installs `game.client` when at least one client permission is granted.
/// Read functions and modify functions are installed independently so a mod
/// cannot discover data it did not request.
pub fn install(
    lua: &Lua,
    game: &Table,
    context: SharedApiContext,
    mod_id: &str,
    can_read: bool,
    can_modify: bool,
) -> mlua::Result<()> {
    if !can_read && !can_modify {
        return Ok(());
    }

    let client = lua.create_table()?;
    if can_read {
        install_read(lua, &client, context.clone())?;
    }
    if can_modify {
        install_modify(lua, &client, context, mod_id)?;
    }
    game.set("client", client)
}

fn install_read(lua: &Lua, client: &Table, context: SharedApiContext) -> mlua::Result<()> {
    let snapshot_context = context.clone();
    client.set(
        "snapshot",
        lua.create_function(move |lua, ()| {
            client_snapshot_table(lua, &snapshot_context.snapshot())
        })?,
    )?;

    let connected_context = context.clone();
    client.set(
        "is_connected",
        lua.create_function(move |_, ()| {
            Ok(connected_context.snapshot().connection.is_connected())
        })?,
    )?;

    let connection_context = context.clone();
    client.set(
        "connection",
        lua.create_function(move |lua, ()| {
            connection_table(lua, &connection_context.snapshot().connection)
        })?,
    )?;

    let window_context = context.clone();
    client.set(
        "window",
        lua.create_function(move |lua, ()| window_table(lua, &window_context.snapshot().window))?,
    )?;

    client.set(
        "settings",
        lua.create_function(move |lua, ()| settings_table(lua, &context.snapshot().settings))?,
    )
}

fn install_modify(
    lua: &Lua,
    client: &Table,
    context: SharedApiContext,
    mod_id: &str,
) -> mlua::Result<()> {
    let mod_id = mod_id.to_owned();

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_fov_override",
        lua.create_function(move |_, value: Option<f32>| {
            let value = validate_optional_number(value, 30.0, 110.0, "FOV")?;
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetFovOverride(value),
            )
        })?,
    )?;

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_view_bobbing_override",
        lua.create_function(move |_, value: Option<bool>| {
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetViewBobbingOverride(value),
            )
        })?,
    )?;

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_hud_visibility_override",
        lua.create_function(move |_, value: Option<bool>| {
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetHudVisibilityOverride(value),
            )
        })?,
    )?;

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_camera_mode_override",
        lua.create_function(move |_, value: Option<String>| {
            let mode = value
                .as_deref()
                .map(|value| {
                    CameraMode::parse(value).ok_or_else(|| {
                        mlua::Error::RuntimeError(
                            "camera mode must be 'first_person', 'third_person_back', or \
                             'third_person_front'"
                                .into(),
                        )
                    })
                })
                .transpose()?;
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetCameraModeOverride(mode),
            )
        })?,
    )?;

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_fullscreen",
        lua.create_function(move |_, fullscreen: bool| {
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetFullscreen(fullscreen),
            )
        })?,
    )?;

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_hurtcam_override",
        lua.create_function(move |_, value: Option<bool>| {
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetHurtcamOverride(value),
            )
        })?,
    )?;

    let command_context = context.clone();
    let command_mod = mod_id.clone();
    client.set(
        "set_fovchange_override",
        lua.create_function(move |_, value: Option<bool>| {
            enqueue(
                &command_context,
                &command_mod,
                ClientCommand::SetFovchangeOverride(value),
            )
        })?,
    )?;

    client.set(
        "set_window_title",
        lua.create_function(move |_, title: Option<String>| {
            let title = title
                .map(|title| validate_window_title(title))
                .transpose()?;
            enqueue(&context, &mod_id, ClientCommand::SetWindowTitle(title))
        })?,
    )?;

    Ok(())
}

fn enqueue(context: &SharedApiContext, mod_id: &str, command: ClientCommand) -> mlua::Result<()> {
    context
        .enqueue_client_command(mod_id, command)
        .map_err(|error| mlua::Error::RuntimeError(error.to_string()))
}

fn validate_optional_number(
    value: Option<f32>,
    min: f32,
    max: f32,
    name: &str,
) -> mlua::Result<Option<f32>> {
    value
        .map(|value| {
            if value.is_finite() && (min..=max).contains(&value) {
                Ok(value)
            } else {
                Err(mlua::Error::RuntimeError(format!(
                    "{name} must be finite and between {min} and {max}"
                )))
            }
        })
        .transpose()
}

fn validate_window_title(title: String) -> mlua::Result<String> {
    if title.len() > 256 {
        return Err(mlua::Error::RuntimeError(
            "window title exceeds 256 bytes".into(),
        ));
    }
    if title.chars().any(char::is_control) {
        return Err(mlua::Error::RuntimeError(
            "window title must not contain control characters".into(),
        ));
    }
    Ok(title)
}

pub(crate) fn client_snapshot_table(lua: &Lua, snapshot: &ClientSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("tick", snapshot.tick)?;
    table.set("frame_delta_seconds", snapshot.frame_delta_seconds)?;
    table.set("fps", snapshot.fps)?;
    table.set("active_screen", snapshot.active_screen.as_str())?;
    table.set("paused", snapshot.paused)?;
    table.set("window", window_table(lua, &snapshot.window)?)?;
    table.set("settings", settings_table(lua, &snapshot.settings)?)?;
    table.set("connection", connection_table(lua, &snapshot.connection)?)?;
    table.set("player_present", snapshot.player.is_some())?;
    Ok(table)
}

fn window_table(lua: &Lua, window: &WindowSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("width", window.width)?;
    table.set("height", window.height)?;
    table.set("framebuffer_width", window.framebuffer_width)?;
    table.set("framebuffer_height", window.framebuffer_height)?;
    table.set("scale_factor", window.scale_factor)?;
    table.set("focused", window.focused)?;
    table.set("fullscreen", window.fullscreen)?;
    Ok(table)
}

fn settings_table(lua: &Lua, settings: &ClientSettingsSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("fov_degrees", settings.fov_degrees)?;
    table.set("gui_scale", settings.gui_scale)?;
    table.set("view_bobbing", settings.view_bobbing)?;
    table.set("hud_visible", settings.hud_visible)?;
    table.set("camera_mode", settings.camera_mode.as_str())?;
    Ok(table)
}

fn connection_table(lua: &Lua, connection: &ConnectionSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("state", connection.state.as_str())?;
    table.set("connected", connection.is_connected())?;
    table.set("server_address", connection.server_address.clone())?;
    table.set("protocol_version", connection.protocol_version)?;
    table.set("protocol_name", connection.protocol_name.clone())?;
    table.set("latency_ms", connection.latency_ms)?;
    table.set("encrypted", connection.encrypted)?;
    table.set("server_brand", connection.server_brand.clone())?;
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Value;

    fn installed(context: SharedApiContext, can_read: bool, can_modify: bool) -> (Lua, Table) {
        let lua = Lua::new();
        let game = lua.create_table().unwrap();
        install(&lua, &game, context, "client_test", can_read, can_modify).unwrap();
        let client = game.get("client").unwrap();
        lua.globals().set("game", game).unwrap();
        (lua, client)
    }

    #[test]
    fn read_and_modify_surfaces_are_trimmed_independently() {
        let (_lua, read) = installed(SharedApiContext::default(), true, false);
        assert!(matches!(
            read.get::<Value>("set_fov_override").unwrap(),
            Value::Nil
        ));

        let (_lua, modify) = installed(SharedApiContext::default(), false, true);
        assert!(matches!(
            modify.get::<Value>("snapshot").unwrap(),
            Value::Nil
        ));
    }

    #[test]
    fn snapshot_functions_read_the_latest_rust_value() {
        let context = SharedApiContext::default();
        let (lua, _client) = installed(context.clone(), true, false);
        context.update_snapshot(ClientSnapshot {
            tick: 99,
            active_screen: "playing".into(),
            connection: ConnectionSnapshot {
                state: "play".into(),
                ..ConnectionSnapshot::default()
            },
            ..ClientSnapshot::default()
        });

        let (tick, screen, connected): (u64, String, bool) = lua
            .load(
                "local c = game.client.snapshot(); return c.tick, c.active_screen, \
                 game.client.is_connected()",
            )
            .eval()
            .unwrap();
        assert_eq!((tick, screen.as_str(), connected), (99, "playing", true));
    }

    #[test]
    fn modify_commands_are_validated_and_never_expose_player_movement() {
        let context = SharedApiContext::default();
        let (lua, client) = installed(context.clone(), false, true);
        assert!(matches!(
            client.get::<Value>("teleport").unwrap(),
            Value::Nil
        ));
        assert!(lua
            .load("game.client.set_fov_override(500)")
            .exec()
            .is_err());
        assert!(context.drain_client_commands().is_empty());

        lua.load(
            "game.client.set_fov_override(90); \
             game.client.set_camera_mode_override('third_person_back')",
        )
        .exec()
        .unwrap();
        assert_eq!(
            context.drain_client_commands(),
            vec![
                super::super::context::QueuedClientCommand {
                    mod_id: "client_test".into(),
                    command: ClientCommand::SetFovOverride(Some(90.0)),
                },
                super::super::context::QueuedClientCommand {
                    mod_id: "client_test".into(),
                    command: ClientCommand::SetCameraModeOverride(Some(
                        CameraMode::ThirdPersonBack
                    )),
                },
            ]
        );
    }
}
