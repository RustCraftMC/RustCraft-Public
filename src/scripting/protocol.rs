//! Lua protocol translator registrations owned by one isolated mod runtime.

use mlua::{Function, Lua, RegistryKey};
use std::collections::BTreeMap;

use crate::net::dynamic_packet::ProtocolVersion;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LuaTranslatorDescriptor {
    pub id: String,
    pub source: ProtocolVersion,
    pub target: ProtocolVersion,
}

struct LuaTranslatorEntry {
    descriptor: LuaTranslatorDescriptor,
    inbound: RegistryKey,
    outbound: RegistryKey,
}

#[derive(Default)]
pub struct LuaTranslatorRegistry {
    entries: BTreeMap<String, LuaTranslatorEntry>,
}

impl LuaTranslatorRegistry {
    pub fn register(
        &mut self,
        lua: &Lua,
        descriptor: LuaTranslatorDescriptor,
        inbound: Function,
        outbound: Function,
    ) -> mlua::Result<()> {
        if self.entries.contains_key(&descriptor.id) {
            return Err(mlua::Error::RuntimeError(format!(
                "protocol translator '{}' is already registered",
                descriptor.id
            )));
        }
        self.entries.insert(
            descriptor.id.clone(),
            LuaTranslatorEntry {
                descriptor,
                inbound: lua.create_registry_value(inbound)?,
                outbound: lua.create_registry_value(outbound)?,
            },
        );
        Ok(())
    }

    pub fn descriptors(&self) -> Vec<LuaTranslatorDescriptor> {
        self.entries
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    pub fn function(&self, lua: &Lua, id: &str, inbound: bool) -> mlua::Result<Option<Function>> {
        let Some(entry) = self.entries.get(id) else {
            return Ok(None);
        };
        lua.registry_value(if inbound {
            &entry.inbound
        } else {
            &entry.outbound
        })
        .map(Some)
    }

    pub fn clear(&mut self, lua: &Lua) {
        for (_, entry) in std::mem::take(&mut self.entries) {
            let _ = lua.remove_registry_value(entry.inbound);
            let _ = lua.remove_registry_value(entry.outbound);
        }
    }
}
