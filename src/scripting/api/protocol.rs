//! Version translation API boundary; codecs and connection internals remain Rust-owned.

use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use std::cell::RefCell;
use std::rc::Rc;

use crate::net::dynamic_packet::{DynamicPacket, ProtocolVersion};
use crate::scripting::permissions::{Permission, PermissionSet};
use crate::scripting::protocol::{LuaTranslatorDescriptor, LuaTranslatorRegistry};

pub const INSPECT_PERMISSION: &str = "protocol.inspect";
pub const TRANSLATE_PERMISSION: &str = "protocol.translate";
const MAX_TRANSLATED_PACKETS: usize = 16;
const MAX_TRANSLATED_JSON_BYTES: usize = 1024 * 1024;

pub fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    translators: Rc<RefCell<LuaTranslatorRegistry>>,
) -> mlua::Result<()> {
    if !permissions.contains(Permission::ProtocolInspect)
        && !permissions.contains(Permission::ProtocolTranslate)
    {
        return Ok(());
    }
    let protocol = lua.create_table()?;
    if permissions.contains(Permission::ProtocolTranslate) {
        protocol.set(
            "register_translator",
            lua.create_function(move |lua, spec: Table| {
                let id: String = spec.get("id")?;
                validate_translator_id(&id)?;
                let source = ProtocolVersion(spec.get("source")?);
                let target = ProtocolVersion(spec.get("target")?);
                if source == target || source.0 < 0 || target.0 < 0 {
                    return Err(mlua::Error::RuntimeError(
                        "translator source and target must be distinct non-negative versions"
                            .into(),
                    ));
                }
                let inbound: Function = spec.get("inbound")?;
                let outbound: Function = spec.get("outbound")?;
                translators.borrow_mut().register(
                    lua,
                    LuaTranslatorDescriptor { id, source, target },
                    inbound,
                    outbound,
                )
            })?,
        )?;
    }
    game.set("protocol", protocol)
}

pub fn context_table(
    lua: &Lua,
    source: ProtocolVersion,
    target: ProtocolVersion,
) -> mlua::Result<Table> {
    let context = lua.create_table()?;
    context.set("source_version", source.0)?;
    context.set("target_version", target.0)?;
    Ok(context)
}

pub fn packets_from_value(
    lua: &Lua,
    value: Value,
    base: &DynamicPacket,
    output_version: ProtocolVersion,
) -> mlua::Result<Vec<DynamicPacket>> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::Table(table) if is_packet_sequence(&table)? => {
            let count = table.raw_len();
            if count > MAX_TRANSLATED_PACKETS {
                return Err(mlua::Error::RuntimeError(format!(
                    "translator returned more than {MAX_TRANSLATED_PACKETS} packets"
                )));
            }
            table
                .sequence_values::<Table>()
                .map(|entry| packet_from_table(lua, entry?, base, output_version))
                .collect()
        }
        Value::Table(table) => Ok(vec![packet_from_table(lua, table, base, output_version)?]),
        _ => Err(mlua::Error::RuntimeError(
            "translator must return nil, a packet table, or a packet sequence".into(),
        )),
    }
}

fn packet_from_table(
    lua: &Lua,
    table: Table,
    base: &DynamicPacket,
    output_version: ProtocolVersion,
) -> mlua::Result<DynamicPacket> {
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        if let Value::String(key) = key {
            let key = key.to_str()?;
            if !matches!(
                key.as_ref(),
                "direction" | "state" | "version" | "name" | "id" | "fields"
            ) {
                return Err(mlua::Error::RuntimeError(format!(
                    "unknown translated packet property '{key}'"
                )));
            }
        }
    }
    let name = table
        .get::<Option<String>>("name")?
        .or_else(|| base.packet_name.clone());
    let packet_id = table.get::<Option<i32>>("id")?.unwrap_or(base.packet_id);
    let fields = match table.get::<Option<Value>>("fields")? {
        Some(value) => lua.from_value(value)?,
        None => base.fields.clone(),
    };
    if !fields.is_object() {
        return Err(mlua::Error::RuntimeError(
            "translated packet fields must be a table".into(),
        ));
    }
    let encoded_size = serde_json::to_vec(&fields)
        .map_err(mlua::Error::external)?
        .len();
    if encoded_size > MAX_TRANSLATED_JSON_BYTES {
        return Err(mlua::Error::RuntimeError(format!(
            "translated packet fields exceed {MAX_TRANSLATED_JSON_BYTES} bytes"
        )));
    }
    Ok(DynamicPacket {
        direction: base.direction,
        state: base.state,
        version: output_version,
        packet_id,
        packet_name: name,
        fields,
        raw_payload: None,
    })
}

fn is_packet_sequence(table: &Table) -> mlua::Result<bool> {
    Ok(table.get::<Option<Value>>("name")?.is_none()
        && table.get::<Option<Value>>("fields")?.is_none()
        && table.raw_len() > 0)
}

fn validate_translator_id(id: &str) -> mlua::Result<()> {
    let Some((namespace, path)) = id.split_once(':') else {
        return Err(mlua::Error::RuntimeError(
            "translator id must be namespaced, for example 'example:legacy_chat'".into(),
        ));
    };
    let valid_part = |value: &str| {
        !value.is_empty()
            && value.len() <= 64
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.' | b'/')
            })
    };
    if !valid_part(namespace) || !valid_part(path) || id.len() > 129 {
        return Err(mlua::Error::RuntimeError(
            "translator id contains invalid characters".into(),
        ));
    }
    Ok(())
}
