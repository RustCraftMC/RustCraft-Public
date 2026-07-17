//! Structured packet API boundary; raw sockets and session credentials are never exposed.

use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::net::dynamic_packet::{DynamicPacket, DynamicProtocolState, PacketDirection};
use crate::scripting::callback::CallbackRegistry;
use crate::scripting::permissions::{Permission, PermissionSet};

pub const OBSERVE_PERMISSION: &str = "network.observe";
pub const MODIFY_PERMISSION: &str = "network.modify";
pub const CANCEL_PERMISSION: &str = "network.cancel";
pub const SEND_PERMISSION: &str = "network.send";
const MAX_ACTIVE_PACKETS_PER_TICK: usize = 20;

pub fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    callbacks: Rc<RefCell<CallbackRegistry>>,
    outbound_packets: Rc<RefCell<Vec<DynamicPacket>>>,
) -> mlua::Result<()> {
    if !permissions.contains(Permission::NetworkObserve)
        && !permissions.contains(Permission::NetworkSend)
    {
        return Ok(());
    }
    let network = lua.create_table()?;
    if permissions.contains(Permission::NetworkObserve) {
        network.set(
            "on_packet",
            lua.create_function(move |lua, options: Table| {
                let direction = options
                    .get::<Option<String>>("direction")?
                    .unwrap_or_else(|| "inbound".into());
                let event_name = match direction.as_str() {
                    "inbound" => "network.packet.inbound",
                    "outbound" => "network.packet.outbound",
                    _ => {
                        return Err(mlua::Error::RuntimeError(
                            "packet direction must be 'inbound' or 'outbound'".into(),
                        ))
                    }
                };
                let names_table: Table = options.get("names")?;
                let names = names_table
                    .sequence_values::<String>()
                    .collect::<mlua::Result<HashSet<_>>>()?;
                if names.is_empty() || names.len() > 256 {
                    return Err(mlua::Error::RuntimeError(
                        "packet filter must contain between 1 and 256 names".into(),
                    ));
                }
                let callback: mlua::Function = options.get("callback")?;
                let priority = options.get::<Option<i32>>("priority")?.unwrap_or(0);
                let filtered = lua.create_function(move |_, event: Table| {
                    let packet: Table = event.get("packet")?;
                    let name = packet.get::<Option<String>>("name")?;
                    if name.as_ref().is_some_and(|name| names.contains(name)) {
                        callback.call::<()>(event)?;
                    }
                    Ok(())
                })?;
                callbacks
                    .borrow_mut()
                    .register(lua, event_name.into(), priority, filtered)
            })?,
        )?;
    }
    if permissions.contains(Permission::NetworkSend) {
        network.set(
            "send",
            lua.create_function(move |lua, value: Value| {
                if outbound_packets.borrow().len() >= MAX_ACTIVE_PACKETS_PER_TICK {
                    return Err(mlua::Error::RuntimeError(format!(
                        "active packet limit exceeded ({MAX_ACTIVE_PACKETS_PER_TICK} per tick)"
                    )));
                }
                let value: serde_json::Value = lua.from_value(value)?;
                let object = value.as_object().ok_or_else(|| {
                    mlua::Error::RuntimeError("network.send expects a packet table".into())
                })?;
                let name = object
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        mlua::Error::RuntimeError("packet requires string field 'name'".into())
                    })?;
                let fields = object.get("fields").cloned().ok_or_else(|| {
                    mlua::Error::RuntimeError("packet requires table field 'fields'".into())
                })?;
                let packet = DynamicPacket::v47_serverbound_named(name, fields)
                    .map_err(mlua::Error::external)?;
                outbound_packets.borrow_mut().push(packet);
                Ok(())
            })?,
        )?;
    }
    game.set("network", network)
}

#[derive(Clone, Debug, Default)]
pub struct PacketControl {
    pub cancelled: bool,
    pub replacement: Option<DynamicPacket>,
}

pub fn event_table(
    lua: &Lua,
    event_name: &str,
    packet: &DynamicPacket,
    can_modify: bool,
    can_cancel: bool,
    control: Rc<RefCell<PacketControl>>,
) -> mlua::Result<(Table, Table)> {
    let event = lua.create_table()?;
    event.set("name", event_name)?;
    let packet_table = packet_table(lua, packet)?;
    event.set("packet", packet_table.clone())?;

    let cancel_control = control.clone();
    event.set(
        "cancel",
        lua.create_function(move |_, _: Table| {
            if !can_cancel {
                return Err(mlua::Error::RuntimeError(
                    "permission 'network.cancel' is required".into(),
                ));
            }
            cancel_control.borrow_mut().cancelled = true;
            Ok(())
        })?,
    )?;

    let replacement_base = packet.clone();
    let replacement_control = control;
    event.set(
        "replace",
        lua.create_function(move |lua, (_self, value): (Table, Value)| {
            if !can_modify {
                return Err(mlua::Error::RuntimeError(
                    "permission 'network.modify' is required".into(),
                ));
            }
            let value: serde_json::Value = lua.from_value(value)?;
            let object = value.as_object().ok_or_else(|| {
                mlua::Error::RuntimeError("replacement packet must be a table".into())
            })?;
            let name = object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    mlua::Error::RuntimeError("replacement packet requires 'name'".into())
                })?;
            let fields = object.get("fields").cloned().ok_or_else(|| {
                mlua::Error::RuntimeError("replacement packet requires 'fields'".into())
            })?;
            let replacement = replacement_base
                .replacement(name.to_owned(), fields)
                .map_err(mlua::Error::external)?;
            replacement_control.borrow_mut().replacement = Some(replacement);
            Ok(())
        })?,
    )?;
    Ok((event, packet_table))
}

pub fn fields_from_table(lua: &Lua, packet_table: &Table) -> mlua::Result<serde_json::Value> {
    lua.from_value(packet_table.get("fields")?)
}

pub(crate) fn packet_table(lua: &Lua, packet: &DynamicPacket) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set(
        "direction",
        match packet.direction {
            PacketDirection::Inbound => "inbound",
            PacketDirection::Outbound => "outbound",
        },
    )?;
    table.set(
        "state",
        match packet.state {
            DynamicProtocolState::Handshake => "handshake",
            DynamicProtocolState::Status => "status",
            DynamicProtocolState::Login => "login",
            DynamicProtocolState::Configuration => "configuration",
            DynamicProtocolState::Play => "play",
        },
    )?;
    table.set("version", packet.version.0)?;
    table.set("id", packet.packet_id)?;
    table.set("name", packet.packet_name.clone())?;
    table.set("fields", lua.to_value(&packet.fields)?)?;
    Ok(table)
}
