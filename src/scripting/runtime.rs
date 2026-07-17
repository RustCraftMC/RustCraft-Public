use mlua::{Function, HookTriggers, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::api;
use super::api::animation::ScriptTransform;
use super::callback::{CallbackDescriptor, CallbackId, CallbackRegistry};
use super::config::{ConfigEntrySnapshot, ConfigValue, ModConfig, SharedModConfig};
use super::errors::{ScriptError, ScriptResult};
use super::event_bus::{EventOutcome, ScriptEvent};
use super::loader::LoadedMod;
use super::manifest::{ModId, ModManifest};
use super::permissions::{Permission, PermissionSet};
use super::protocol::{LuaTranslatorDescriptor, LuaTranslatorRegistry};
use crate::net::dynamic_packet::DynamicPacket;
use crate::render::first_person::{AnimationOverrides, FirstPersonAnimationContext};
use crate::render::hooks::{ScriptDrawCommand, ScriptFrameContext};

const HOOK_GRANULARITY: u32 = 1_000;

#[derive(Clone, Copy, Debug)]
pub struct RuntimeLimits {
    pub memory_bytes: usize,
    pub instructions_per_call: u64,
    pub consecutive_error_limit: u32,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 16 * 1024 * 1024,
            instructions_per_call: 250_000,
            consecutive_error_limit: 3,
        }
    }
}

pub struct LuaModRuntime {
    pub id: ModId,
    pub lua: Lua,
    pub manifest: ModManifest,
    pub permissions: PermissionSet,
    pub enabled: bool,
    pub root: PathBuf,
    callbacks: Rc<RefCell<CallbackRegistry>>,
    outbound_packets: Rc<RefCell<Vec<DynamicPacket>>>,
    translators: Rc<RefCell<LuaTranslatorRegistry>>,
    config: SharedModConfig,
    ui_state: api::ui::UiApiState,
    instruction_count: Rc<Cell<u64>>,
    limits: RuntimeLimits,
    consecutive_errors: u32,
}

impl LuaModRuntime {
    pub fn load(
        loaded: LoadedMod,
        permissions: PermissionSet,
        limits: RuntimeLimits,
        api_state: api::RuntimeApiState,
    ) -> ScriptResult<Self> {
        let libraries =
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8 | StdLib::COROUTINE;
        let lua = Lua::new_with(libraries, LuaOptions::default())?;
        lua.set_memory_limit(limits.memory_bytes)?;

        for forbidden in [
            "os", "io", "debug", "package", "require", "dofile", "loadfile",
        ] {
            lua.globals().set(forbidden, Value::Nil)?;
        }

        let instruction_count = Rc::new(Cell::new(0u64));
        let hook_count = instruction_count.clone();
        let instruction_limit = limits.instructions_per_call;
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(HOOK_GRANULARITY),
            move |_, _| {
                let next = hook_count.get().saturating_add(HOOK_GRANULARITY as u64);
                hook_count.set(next);
                if next > instruction_limit {
                    Err(mlua::Error::RuntimeError(format!(
                        "script instruction budget exceeded ({instruction_limit})"
                    )))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )?;

        let callbacks = Rc::new(RefCell::new(CallbackRegistry::default()));
        let outbound_packets = Rc::new(RefCell::new(Vec::new()));
        let translators = Rc::new(RefCell::new(LuaTranslatorRegistry::default()));
        let config = Rc::new(RefCell::new(ModConfig::load(&loaded.root)?));
        let storage = api::storage::StorageApiState::open(&loaded.root)?;
        if permissions.contains(Permission::StorageRead)
            || permissions.contains(Permission::StorageWrite)
        {
            // A mod that can reach storage fails closed before its entrypoint runs if the
            // persistent document is corrupt or violates the current safety limits.
            storage.validate()?;
        }
        let ui_state = api_state.ui.clone();
        api::install(
            &lua,
            &loaded.manifest,
            &permissions,
            callbacks.clone(),
            outbound_packets.clone(),
            translators.clone(),
            config.clone(),
            storage,
            api_state,
        )?;

        let mut runtime = Self {
            id: loaded.manifest.id.clone(),
            lua,
            manifest: loaded.manifest,
            permissions,
            enabled: true,
            root: loaded.root,
            callbacks,
            outbound_packets,
            translators,
            config,
            ui_state,
            instruction_count,
            limits,
            consecutive_errors: 0,
        };
        runtime.run_source(&loaded.source)?;
        runtime.call_lifecycle("on_load")?;
        Ok(runtime)
    }

    fn begin_call(&self) {
        self.instruction_count.set(0);
    }

    fn run_source(&self, source: &str) -> ScriptResult<()> {
        self.begin_call();
        self.lua
            .load(source)
            .set_name(format!("@{}/{}", self.id, self.manifest.entrypoints.client))
            .exec()?;
        Ok(())
    }

    fn call_lifecycle(&mut self, name: &str) -> ScriptResult<()> {
        let function = self.lua.globals().get::<Option<Function>>(name)?;
        if let Some(function) = function {
            self.begin_call();
            function.call::<()>(())?;
        }
        Ok(())
    }

    pub fn descriptors(&self, event: &str) -> Vec<CallbackDescriptor> {
        if !self.enabled {
            return Vec::new();
        }
        self.callbacks.borrow().descriptors(event)
    }

    pub fn has_callbacks(&self, event: &str) -> bool {
        self.enabled && self.callbacks.borrow().has_active(event)
    }

    pub fn dispatch_callback(
        &self,
        id: CallbackId,
        event: &ScriptEvent,
        outcome: Rc<RefCell<EventOutcome>>,
    ) -> mlua::Result<Duration> {
        let function = self.callbacks.borrow().function(&self.lua, id)?;
        let Some(function) = function else {
            return Ok(Duration::ZERO);
        };
        let table = self.event_table(event, outcome)?;
        self.begin_call();
        let started = Instant::now();
        function.call::<()>(table)?;
        Ok(started.elapsed())
    }

    pub fn dispatch_animation_callback(
        &self,
        id: CallbackId,
        event_name: &str,
        context: &FirstPersonAnimationContext,
        transform: ScriptTransform,
    ) -> mlua::Result<Duration> {
        let function = self.callbacks.borrow().function(&self.lua, id)?;
        let Some(function) = function else {
            return Ok(Duration::ZERO);
        };
        let table = api::animation::event_table(&self.lua, event_name, context, transform)?;
        self.begin_call();
        let started = Instant::now();
        function.call::<()>(table)?;
        Ok(started.elapsed())
    }

    pub fn dispatch_animation_calculate(
        &self,
        id: CallbackId,
        context: &FirstPersonAnimationContext,
        transform: ScriptTransform,
        overrides: Rc<RefCell<AnimationOverrides>>,
    ) -> mlua::Result<Duration> {
        let function = self.callbacks.borrow().function(&self.lua, id)?;
        let Some(function) = function else {
            return Ok(Duration::ZERO);
        };
        let table =
            api::animation::event_table_calculate(&self.lua, context, overrides, transform)?;
        self.begin_call();
        let started = Instant::now();
        function.call::<()>(table)?;
        Ok(started.elapsed())
    }

    pub fn dispatch_render_callback(
        &self,
        id: CallbackId,
        event_name: &str,
        frame: ScriptFrameContext,
        commands: Rc<RefCell<Vec<ScriptDrawCommand>>>,
    ) -> mlua::Result<Duration> {
        let function = self.callbacks.borrow().function(&self.lua, id)?;
        let Some(function) = function else {
            return Ok(Duration::ZERO);
        };
        let draw = api::render::ScriptDrawContext::new(
            commands,
            self.permissions.contains(Permission::RenderCustomDraw),
        );
        let table = api::render::event_table(&self.lua, event_name, frame, draw)?;
        self.begin_call();
        let started = Instant::now();
        function.call::<()>(table)?;
        Ok(started.elapsed())
    }

    pub fn dispatch_packet_callback(
        &self,
        id: CallbackId,
        event_name: &str,
        packet: &mut DynamicPacket,
    ) -> mlua::Result<(Duration, api::network::PacketControl)> {
        let function = self.callbacks.borrow().function(&self.lua, id)?;
        let Some(function) = function else {
            return Ok((Duration::ZERO, api::network::PacketControl::default()));
        };
        let control = Rc::new(RefCell::new(api::network::PacketControl::default()));
        let (event, packet_table) = api::network::event_table(
            &self.lua,
            event_name,
            packet,
            self.permissions.contains(Permission::NetworkModify),
            self.permissions.contains(Permission::NetworkCancel),
            control.clone(),
        )?;
        self.begin_call();
        let started = Instant::now();
        function.call::<()>(event)?;
        let elapsed = started.elapsed();
        let fields = api::network::fields_from_table(&self.lua, &packet_table)?;
        if fields != packet.fields {
            if !self.permissions.contains(Permission::NetworkModify) {
                return Err(mlua::Error::RuntimeError(
                    "permission 'network.modify' is required".into(),
                ));
            }
            packet.fields = fields;
        }
        let final_control = control.borrow().clone();
        Ok((elapsed, final_control))
    }

    pub fn translator_descriptors(&self) -> Vec<LuaTranslatorDescriptor> {
        if !self.enabled {
            return Vec::new();
        }
        self.translators.borrow().descriptors()
    }

    pub fn dispatch_protocol_translator(
        &self,
        id: &str,
        inbound: bool,
        packet: &DynamicPacket,
        source: crate::net::dynamic_packet::ProtocolVersion,
        target: crate::net::dynamic_packet::ProtocolVersion,
    ) -> mlua::Result<Vec<DynamicPacket>> {
        let function = self.translators.borrow().function(&self.lua, id, inbound)?;
        let Some(function) = function else {
            return Err(mlua::Error::RuntimeError(format!(
                "protocol translator '{id}' is no longer registered"
            )));
        };
        let packet_table = api::network::packet_table(&self.lua, packet)?;
        let context = api::protocol::context_table(&self.lua, source, target)?;
        self.begin_call();
        let value: Value = function.call((packet_table, context))?;
        api::protocol::packets_from_value(
            &self.lua,
            value,
            packet,
            if inbound { target } else { source },
        )
    }

    fn event_table(
        &self,
        event: &ScriptEvent,
        outcome: Rc<RefCell<EventOutcome>>,
    ) -> mlua::Result<Table> {
        let table = match self.lua.to_value(&event.payload)? {
            Value::Table(table) => table,
            value => {
                let table = self.lua.create_table()?;
                table.set("data", value)?;
                table
            }
        };
        table.set("name", event.name.as_str())?;

        let cancel_state = outcome.clone();
        let cancel_permission = self.permissions.clone();
        let event_name = event.name.clone();
        table.set(
            "cancel",
            self.lua.create_function(move |_, _: Table| {
                if event_name.starts_with("network.")
                    && !cancel_permission.contains(Permission::NetworkCancel)
                {
                    return Err(mlua::Error::RuntimeError(
                        "permission 'network.cancel' is required".into(),
                    ));
                }
                cancel_state.borrow_mut().cancelled = true;
                Ok(())
            })?,
        )?;

        let consume_state = outcome.clone();
        let consume_permission = self.permissions.clone();
        let event_name = event.name.clone();
        table.set(
            "consume",
            self.lua.create_function(move |_, _: Table| {
                if event_name.starts_with("input.")
                    && !consume_permission.contains(Permission::InputConsume)
                {
                    return Err(mlua::Error::RuntimeError(
                        "permission 'input.consume' is required".into(),
                    ));
                }
                consume_state.borrow_mut().consumed = true;
                Ok(())
            })?,
        )?;

        let result_state = outcome;
        table.set(
            "set_result",
            self.lua
                .create_function(move |lua, (_self, value): (Table, Value)| {
                    result_state.borrow_mut().result = Some(lua.from_value(value)?);
                    Ok(())
                })?,
        )?;
        Ok(table)
    }

    pub fn record_result(&mut self, failed: bool) {
        if failed {
            self.consecutive_errors = self.consecutive_errors.saturating_add(1);
            if self.consecutive_errors >= self.limits.consecutive_error_limit {
                self.enabled = false;
                log::error!(
                    target: "rustcraft::lua",
                    "mod '{}' disabled after {} consecutive callback errors",
                    self.id, self.consecutive_errors
                );
            }
        } else {
            self.consecutive_errors = 0;
        }
        self.callbacks.borrow_mut().prune_removed(&self.lua);
    }

    pub fn unload(&mut self) {
        if self.enabled {
            if let Err(error) = self.call_lifecycle("on_unload") {
                log::error!(
                    target: "rustcraft::lua",
                    "mod '{}' on_unload failed: {error}",
                    self.id
                );
            }
        }
        self.enabled = false;
        self.callbacks.borrow_mut().clear(&self.lua);
        self.outbound_packets.borrow_mut().clear();
        self.translators.borrow_mut().clear(&self.lua);
        self.ui_state.clear_commands();
    }

    pub fn take_outbound_packets(&self) -> Vec<DynamicPacket> {
        std::mem::take(&mut *self.outbound_packets.borrow_mut())
    }

    pub fn config_entries(&self) -> Vec<ConfigEntrySnapshot> {
        self.config.borrow().snapshots()
    }

    pub fn config_entry_count(&self) -> usize {
        self.config.borrow().entry_count()
    }

    pub fn set_config_value(&self, key: &str, value: ConfigValue) -> ScriptResult<()> {
        self.config.borrow_mut().set_value(key, value)
    }

    pub fn update_ui_snapshot(&self, snapshot: api::ui::UiSnapshot) {
        self.ui_state.set_snapshot(snapshot);
    }

    pub fn take_ui_commands(&self) -> Vec<api::ui::UiCommand> {
        self.ui_state.drain_commands()
    }
}

impl Drop for LuaModRuntime {
    fn drop(&mut self) {
        self.callbacks.borrow_mut().clear(&self.lua);
        self.translators.borrow_mut().clear(&self.lua);
    }
}
