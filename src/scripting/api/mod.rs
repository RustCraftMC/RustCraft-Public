//! Lua API registration. APIs expose owned values and command queues only; no Vulkan handles,
//! pointers, authentication secrets, filesystem paths, or sockets are placed in Lua.

pub mod animation;
pub mod client;
pub mod config;
pub mod context;
pub mod input;
pub mod network;
pub mod player;
pub mod protocol;
pub mod render;
pub mod resources;
pub mod storage;
pub mod ui;
pub mod world;

use mlua::{Function, Lua, Table, Value};
use std::cell::RefCell;
use std::rc::Rc;

use crate::net::dynamic_packet::DynamicPacket;
use crate::scripting::callback::CallbackRegistry;
use crate::scripting::config::SharedModConfig;
use crate::scripting::manifest::ModManifest;
use crate::scripting::permissions::{permission_for_event, Permission, PermissionSet};
use crate::scripting::protocol::LuaTranslatorRegistry;

#[derive(Clone)]
pub struct RuntimeApiState {
    pub client: context::SharedApiContext,
    pub world: world::SharedWorldState,
    pub input: input::SharedInputState,
    pub ui: ui::UiApiState,
    pub resources: resources::ResourceApiState,
}

pub fn install(
    lua: &Lua,
    manifest: &ModManifest,
    permissions: &PermissionSet,
    callbacks: Rc<RefCell<CallbackRegistry>>,
    outbound_packets: Rc<RefCell<Vec<DynamicPacket>>>,
    translators: Rc<RefCell<LuaTranslatorRegistry>>,
    config_store: SharedModConfig,
    storage_state: storage::StorageApiState,
    state: RuntimeApiState,
) -> mlua::Result<()> {
    let game = lua.create_table()?;
    install_mod_info(lua, &game, manifest)?;
    install_log(lua, &game, manifest.id.as_str())?;
    install_events(lua, &game, permissions.clone(), callbacks.clone())?;
    config::install(lua, &game, config_store)?;
    storage::install(lua, &game, permissions, storage_state)?;
    client::install(
        lua,
        &game,
        state.client.clone(),
        manifest.id.as_str(),
        permissions.contains(Permission::ClientRead),
        permissions.contains(Permission::ClientModify),
    )?;
    player::install(
        lua,
        &game,
        state.client,
        permissions.contains(Permission::ClientRead),
    )?;
    world::install(lua, &game, permissions, state.world)?;
    input::install(lua, &game, permissions, state.input, manifest.id.as_str())?;
    ui::install(lua, &game, permissions, state.ui)?;
    resources::install(lua, &game, permissions, state.resources)?;
    network::install(lua, &game, permissions, callbacks, outbound_packets)?;
    protocol::install(lua, &game, permissions, translators)?;
    lua.globals().set("game", game)
}

fn install_mod_info(lua: &Lua, game: &Table, manifest: &ModManifest) -> mlua::Result<()> {
    let info = lua.create_table()?;
    info.set("id", manifest.id.as_str())?;
    info.set("name", manifest.name.as_str())?;
    info.set("version", manifest.version.as_str())?;
    info.set("api_version", manifest.api_version)?;
    game.set("mod", info)
}

fn install_log(lua: &Lua, game: &Table, mod_id: &str) -> mlua::Result<()> {
    let log = lua.create_table()?;
    for (name, level) in [
        ("debug", "DEBUG"),
        ("info", "INFO"),
        ("warn", "WARN"),
        ("error", "ERROR"),
    ] {
        let id = mod_id.to_owned();
        log.set(
            name,
            lua.create_function(move |_, message: String| {
                match level {
                    "DEBUG" => ::log::debug!(target: "rustcraft::lua_mod", "[{id}] {message}"),
                    "INFO" => ::log::info!(target: "rustcraft::lua_mod", "[{id}] {message}"),
                    "WARN" => ::log::warn!(target: "rustcraft::lua_mod", "[{id}] {message}"),
                    "ERROR" => ::log::error!(target: "rustcraft::lua_mod", "[{id}] {message}"),
                    _ => ::log::info!(target: "rustcraft::lua_mod", "[{id}] {message}"),
                }
                Ok(())
            })?,
        )?;
    }
    game.set("log", log)
}

fn install_events(
    lua: &Lua,
    game: &Table,
    permissions: PermissionSet,
    callbacks: Rc<RefCell<CallbackRegistry>>,
) -> mlua::Result<()> {
    let events = lua.create_table()?;
    let on = lua.create_function(move |lua, (event_name, spec): (String, Value)| {
        if event_name.is_empty() || event_name.len() > 128 {
            return Err(mlua::Error::RuntimeError(
                "event name must contain between 1 and 128 characters".into(),
            ));
        }
        if let Some(required) = permission_for_event(&event_name) {
            if !permissions.contains(required) {
                return Err(mlua::Error::RuntimeError(format!(
                    "permission '{}' is required to subscribe to '{event_name}'",
                    required.as_str()
                )));
            }
        }
        let (priority, callback): (i32, Function) = match spec {
            Value::Function(callback) => (0, callback),
            Value::Table(options) => (
                options.get::<Option<i32>>("priority")?.unwrap_or(0),
                options.get("callback")?,
            ),
            _ => {
                return Err(mlua::Error::RuntimeError(
                    "events.on expects a function or { priority, callback } table".into(),
                ))
            }
        };
        if !(-10_000..=10_000).contains(&priority) {
            return Err(mlua::Error::RuntimeError(
                "event priority must be between -10000 and 10000".into(),
            ));
        }
        callbacks
            .borrow_mut()
            .register(lua, event_name, priority, callback)
    })?;
    events.set("on", on)?;
    game.set("events", events)
}
