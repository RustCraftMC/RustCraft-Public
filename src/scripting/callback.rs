use mlua::{Function, Lua, RegistryKey, UserData, UserDataMethods};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub type CallbackId = u64;

#[derive(Clone, Debug)]
pub struct CallbackDescriptor {
    pub id: CallbackId,
    pub event: String,
    pub priority: i32,
}

#[derive(Clone)]
pub struct ListenerHandle {
    active: Rc<Cell<bool>>,
}

impl ListenerHandle {
    pub fn remove(&self) {
        self.active.set(false);
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }
}

impl UserData for ListenerHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("remove", |_, this, ()| {
            this.remove();
            Ok(())
        });
        methods.add_method("is_active", |_, this, ()| Ok(this.is_active()));
    }
}

struct CallbackEntry {
    event: String,
    priority: i32,
    function: RegistryKey,
    active: Rc<Cell<bool>>,
}

#[derive(Default)]
pub struct CallbackRegistry {
    next_id: CallbackId,
    entries: BTreeMap<CallbackId, CallbackEntry>,
}

impl CallbackRegistry {
    pub fn register(
        &mut self,
        lua: &Lua,
        event: String,
        priority: i32,
        function: Function,
    ) -> mlua::Result<ListenerHandle> {
        self.next_id = self.next_id.saturating_add(1);
        let active = Rc::new(Cell::new(true));
        self.entries.insert(
            self.next_id,
            CallbackEntry {
                event,
                priority,
                function: lua.create_registry_value(function)?,
                active: active.clone(),
            },
        );
        Ok(ListenerHandle { active })
    }

    pub fn descriptors(&self, event: &str) -> Vec<CallbackDescriptor> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.active.get() && entry.event == event)
            .map(|(&id, entry)| CallbackDescriptor {
                id,
                event: entry.event.clone(),
                priority: entry.priority,
            })
            .collect()
    }

    pub fn has_active(&self, event: &str) -> bool {
        self.entries
            .values()
            .any(|entry| entry.active.get() && entry.event == event)
    }

    pub fn function(&self, lua: &Lua, id: CallbackId) -> mlua::Result<Option<Function>> {
        let Some(entry) = self.entries.get(&id) else {
            return Ok(None);
        };
        if !entry.active.get() {
            return Ok(None);
        }
        lua.registry_value(&entry.function).map(Some)
    }

    pub fn clear(&mut self, lua: &Lua) {
        for (_, entry) in std::mem::take(&mut self.entries) {
            entry.active.set(false);
            let _ = lua.remove_registry_value(entry.function);
        }
    }

    pub fn prune_removed(&mut self, lua: &Lua) {
        let removed: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(&id, entry)| (!entry.active.get()).then_some(id))
            .collect();
        for id in removed {
            if let Some(entry) = self.entries.remove(&id) {
                let _ = lua.remove_registry_value(entry.function);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_callback_probe_tracks_listener_removal() {
        let lua = Lua::new();
        let mut registry = CallbackRegistry::default();
        let function = lua.create_function(|_, ()| Ok(())).unwrap();
        let handle = registry
            .register(&lua, "network.packet.inbound".to_owned(), 0, function)
            .unwrap();

        assert!(registry.has_active("network.packet.inbound"));
        assert!(!registry.has_active("client.tick"));
        handle.remove();
        assert!(!registry.has_active("network.packet.inbound"));
    }
}
