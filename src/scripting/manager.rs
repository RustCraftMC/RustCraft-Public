use serde_json::Value;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use super::api::RuntimeApiState;
use super::config::{ConfigEntrySnapshot, ConfigValue};
use super::errors::{ScriptError, ScriptResult};
use super::event_bus::{EventOutcome, PlannedCallback, ScriptEvent, ScriptEventBus};
use super::loader::{LoadedMod, ModLoader};
use super::manifest::ModId;
use super::permissions::{Permission, PermissionPolicy, PermissionSet};
use super::profiler::ScriptProfiler;
use super::runtime::{LuaModRuntime, RuntimeLimits};
use super::scheduler::ScriptScheduler;
use crate::net::dynamic_packet::DynamicPacket;
use crate::net::dynamic_packet::ProtocolVersion;
use crate::render::first_person::{
    AnimationOverrides, FirstPersonAnimationContext, FirstPersonTransforms,
};
use crate::render::hooks::{ScriptDrawCommand, ScriptFrameContext};
use crate::scripting::api::animation::ScriptTransform;
use crate::scripting::api::context::{
    ClientCommand, ClientSnapshot, QueuedClientCommand, SharedApiContext,
};
use crate::scripting::api::input::{
    InputApiState, InputConsumeRequest, InputSnapshot, SharedInputState,
};
use crate::scripting::api::resources::{
    ResourceApiState, ResourceProviderSnapshot, ResourceRegistration, SharedResourceRegistry,
};
use crate::scripting::api::ui::{UiApiState, UiCommand, UiSnapshot};
use crate::scripting::api::world::{SharedWorldState, WorldApiState, WorldSnapshot};
use nalgebra::Matrix4;

#[derive(Clone, Debug)]
pub enum ScriptCommand {
    Log { mod_id: ModId, message: String },
}

#[derive(Default)]
pub struct ScriptCommandQueue {
    commands: VecDeque<ScriptCommand>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedUiCommand {
    pub mod_id: String,
    pub command: UiCommand,
}

impl ScriptCommandQueue {
    pub fn push(&mut self, command: ScriptCommand) {
        self.commands.push_back(command);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = ScriptCommand> + '_ {
        self.commands.drain(..)
    }
}

#[derive(Clone, Debug, Default)]
pub struct LoadReport {
    pub loaded: Vec<ModId>,
    pub errors: Vec<String>,
    pub denied_permissions: Vec<(ModId, Permission)>,
}

impl LoadReport {
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
}

pub struct ScriptManager {
    mods: HashMap<ModId, LuaModRuntime>,
    load_order: Vec<ModId>,
    event_bus: ScriptEventBus,
    scheduler: ScriptScheduler,
    profiler: ScriptProfiler,
    command_queue: ScriptCommandQueue,
    api_context: SharedApiContext,
    world_state: SharedWorldState,
    input_state: SharedInputState,
    ui_snapshot: RefCell<UiSnapshot>,
    resource_provider: ResourceProviderSnapshot,
    resource_registry: SharedResourceRegistry,
    client_override_owners: HashSet<ModId>,
    loader: ModLoader,
    permission_policy: PermissionPolicy,
    limits: RuntimeLimits,
    connection_active: bool,
    network_audit: VecDeque<NetworkAuditEntry>,
    animation_overrides: HashMap<ModId, AnimationOverrides>,
}

#[derive(Clone, Debug)]
pub struct NetworkAuditEntry {
    pub mod_id: ModId,
    pub direction: crate::net::dynamic_packet::PacketDirection,
    pub version: crate::net::dynamic_packet::ProtocolVersion,
    pub packet_name: Option<String>,
    pub modified: bool,
    pub cancelled: bool,
    pub replaced: bool,
    pub active_send: bool,
}

#[derive(Clone, Debug)]
pub struct PacketHookResult {
    pub packet: Option<DynamicPacket>,
    pub modified: bool,
}

/// Read-only data for the client mod-management screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedModInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub protocol_translator: bool,
    pub config_entries: usize,
    pub granted_permissions: Vec<String>,
    pub denied_permissions: Vec<String>,
}

#[derive(Clone, Debug)]
struct PlannedLuaTranslator {
    mod_id: ModId,
    id: String,
    source: ProtocolVersion,
    target: ProtocolVersion,
}

const MAX_PROTOCOL_PIPELINE_PACKETS: usize = 64;

fn record_result_and_cleanup_resources(
    registry: &SharedResourceRegistry,
    mod_id: &ModId,
    runtime: &mut LuaModRuntime,
    failed: bool,
) {
    runtime.record_result(failed);
    if !runtime.enabled {
        registry.clear_owner(mod_id.as_str());
    }
}

impl ScriptManager {
    pub fn take_frame_profile(&mut self) -> (u64, u32, u32) {
        self.profiler.take_frame()
    }

    pub fn has_callbacks(&self, event_name: &str) -> bool {
        self.load_order.iter().any(|mod_id| {
            self.mods
                .get(mod_id)
                .is_some_and(|runtime| runtime.has_callbacks(event_name))
        })
    }

    pub fn new(mods_dir: impl Into<PathBuf>) -> Self {
        let mods_dir = mods_dir.into();
        let permission_policy = PermissionPolicy::load(&mods_dir.join("permissions.json"));
        Self {
            mods: HashMap::new(),
            load_order: Vec::new(),
            event_bus: ScriptEventBus,
            scheduler: ScriptScheduler::default(),
            profiler: ScriptProfiler::new(),
            command_queue: ScriptCommandQueue::default(),
            api_context: SharedApiContext::default(),
            world_state: Rc::new(RefCell::new(WorldApiState::new())),
            input_state: Rc::new(RefCell::new(InputApiState::new())),
            ui_snapshot: RefCell::new(UiSnapshot::default()),
            resource_provider: ResourceProviderSnapshot::default(),
            resource_registry: SharedResourceRegistry::default(),
            client_override_owners: HashSet::new(),
            loader: ModLoader::new(mods_dir),
            permission_policy,
            limits: RuntimeLimits::default(),
            connection_active: false,
            network_audit: VecDeque::new(),
            animation_overrides: HashMap::new(),
        }
    }

    pub fn with_limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn permission_policy_mut(&mut self) -> &mut PermissionPolicy {
        &mut self.permission_policy
    }

    /// Replaces the owned state read by `game.client` and `game.player`.
    /// Existing Lua closures observe the new value on their next call.
    pub fn update_client_snapshot(&self, snapshot: ClientSnapshot) {
        self.api_context.update_snapshot(snapshot);
    }

    pub fn client_snapshot(&self) -> ClientSnapshot {
        self.api_context.snapshot()
    }

    /// Drains validated client-only commands, preserving their originating mod.
    pub fn drain_client_commands(&mut self) -> Vec<QueuedClientCommand> {
        let mut drained = Vec::new();
        for queued in self.api_context.drain_client_commands() {
            if matches!(queued.command, ClientCommand::ClearVisualOverrides) {
                self.client_override_owners
                    .retain(|owner| owner.as_str() != queued.mod_id.as_str());
                drained.push(queued);
            } else if self.is_enabled(&queued.mod_id) {
                if let Some(mod_id) = self
                    .mods
                    .keys()
                    .find(|id| id.as_str() == queued.mod_id.as_str())
                    .cloned()
                {
                    self.client_override_owners.insert(mod_id);
                }
                drained.push(queued);
            }
        }

        let disabled_owners = self
            .client_override_owners
            .iter()
            .filter(|owner| !self.mods.get(*owner).is_some_and(|runtime| runtime.enabled))
            .cloned()
            .collect::<Vec<_>>();
        for owner in disabled_owners {
            self.client_override_owners.remove(&owner);
            drained.push(QueuedClientCommand {
                mod_id: owner.to_string(),
                command: ClientCommand::ClearVisualOverrides,
            });
        }
        drained
    }

    pub fn update_world_snapshot(&self, snapshot: WorldSnapshot) {
        self.world_state.borrow_mut().set_snapshot(snapshot);
    }

    pub fn update_world_snapshot_reusing_blocks(&self, snapshot: WorldSnapshot) -> bool {
        self.world_state
            .borrow_mut()
            .set_snapshot_reusing_blocks(snapshot)
    }

    pub fn update_world_snapshot_patching_blocks(&self, snapshot: WorldSnapshot) -> bool {
        self.world_state
            .borrow_mut()
            .set_snapshot_patching_blocks(snapshot)
    }

    pub fn update_world_snapshot_shifting_blocks(&self, snapshot: WorldSnapshot) -> bool {
        self.world_state
            .borrow_mut()
            .set_snapshot_shifting_blocks(snapshot)
    }

    pub fn has_world_snapshot(&self) -> bool {
        self.world_state.borrow().has_snapshot()
    }

    pub fn clear_world_snapshot(&self) {
        self.world_state.borrow_mut().clear();
    }

    pub fn update_input_snapshot(&self, snapshot: InputSnapshot) {
        self.input_state.borrow_mut().set_snapshot(snapshot);
    }

    pub fn drain_input_consume_requests(&self) -> Vec<InputConsumeRequest> {
        let requests = self.input_state.borrow_mut().drain_consume_requests();
        requests
            .into_iter()
            .filter(|request| self.is_enabled(&request.requester))
            .collect()
    }

    pub fn update_ui_snapshot(&self, snapshot: UiSnapshot) {
        *self.ui_snapshot.borrow_mut() = snapshot.clone();
        for runtime in self.mods.values() {
            runtime.update_ui_snapshot(snapshot.clone());
        }
    }

    pub fn drain_ui_commands(&mut self) -> Vec<QueuedUiCommand> {
        let mut commands = Vec::new();
        for mod_id in &self.load_order {
            let Some(runtime) = self.mods.get(mod_id) else {
                continue;
            };
            if !runtime.enabled {
                let _ = runtime.take_ui_commands();
                continue;
            }
            commands.extend(runtime.take_ui_commands().into_iter().map(|command| {
                QueuedUiCommand {
                    mod_id: mod_id.to_string(),
                    command,
                }
            }));
        }
        commands
    }

    pub fn resource_registrations(&self) -> Vec<ResourceRegistration> {
        self.resource_registry.declarations()
    }

    pub fn resolve_resource(&self, id: &str) -> ScriptResult<String> {
        self.resource_registry
            .resolve(id)
            .map_err(ScriptError::Configuration)
    }

    pub fn set_connection_active(&mut self, active: bool) {
        if self.connection_active == active {
            return;
        }
        let previous = self.connection_active;
        self.connection_active = active;
        if previous && !active {
            self.dispatch_json("network.disconnect", serde_json::json!({"reason": null}));
        }
        self.dispatch_json(
            "network.state_changed",
            serde_json::json!({
                "previous": if previous { "play" } else { "disconnected" },
                "current": if active { "play" } else { "disconnected" }
            }),
        );
    }

    pub fn notify_disconnect(&mut self, reason: &str) {
        if !self.connection_active {
            return;
        }
        self.dispatch_json("network.disconnect", serde_json::json!({"reason": reason}));
        self.connection_active = false;
        self.dispatch_json(
            "network.state_changed",
            serde_json::json!({"previous": "play", "current": "disconnected"}),
        );
    }

    pub fn load_all(&mut self) -> LoadReport {
        let mut report = LoadReport::default();
        let loaded = match self.loader.discover() {
            Ok(loaded) => loaded,
            Err(error) => {
                report.errors.push(error.to_string());
                return report;
            }
        };
        for loaded_mod in loaded {
            match self.install(loaded_mod, false) {
                Ok((id, denied)) => {
                    report.denied_permissions.extend(
                        denied
                            .into_iter()
                            .map(|permission| (id.clone(), permission)),
                    );
                    report.loaded.push(id);
                }
                Err(error) => report.errors.push(error.to_string()),
            }
        }
        report
    }

    fn install(
        &mut self,
        loaded: LoadedMod,
        replacing: bool,
    ) -> ScriptResult<(ModId, Vec<Permission>)> {
        let id = loaded.manifest.id.clone();
        if self.mods.contains_key(&id) && !replacing {
            return Err(ScriptError::DuplicateMod(id.to_string()));
        }
        let target_is_protocol_translator = loaded
            .manifest
            .permissions
            .contains(&Permission::ProtocolTranslate);
        let current_is_protocol_translator = self.mods.get(&id).is_some_and(|runtime| {
            runtime
                .manifest
                .permissions
                .contains(&Permission::ProtocolTranslate)
        });
        if self.connection_active
            && (target_is_protocol_translator || current_is_protocol_translator)
        {
            return Err(ScriptError::ReloadDenied(format!(
                "protocol translator mod '{id}' cannot change state during a connection"
            )));
        }
        let permissions = PermissionSet::resolve(
            id.as_str(),
            &loaded.manifest.permissions,
            &self.permission_policy,
        );
        let denied = permissions.denied().collect::<Vec<_>>();
        let resource_state = ResourceApiState::new(
            id.as_str(),
            &loaded.root,
            self.resource_provider.clone(),
            self.resource_registry.clone(),
        )
        .map_err(ScriptError::Configuration)?;
        let resource_checkpoint = self.resource_registry.declarations_for_owner(id.as_str());
        self.resource_registry.clear_owner(id.as_str());
        let command_checkpoint = self.api_context.client_command_checkpoint();
        self.api_context.clear_client_commands_for(id.as_str());
        let api_state = RuntimeApiState {
            client: self.api_context.clone(),
            world: self.world_state.clone(),
            input: self.input_state.clone(),
            ui: UiApiState::new(self.ui_snapshot.borrow().clone()),
            resources: resource_state,
        };
        let runtime = match LuaModRuntime::load(loaded, permissions, self.limits, api_state) {
            Ok(runtime) => runtime,
            Err(error) => {
                self.api_context
                    .restore_client_command_checkpoint(command_checkpoint);
                if let Err(restore_error) = self
                    .resource_registry
                    .restore_owner(id.as_str(), &resource_checkpoint)
                {
                    return Err(ScriptError::Configuration(format!(
                        "failed to restore resource registrations for '{id}': {restore_error}"
                    )));
                }
                return Err(error);
            }
        };
        if replacing {
            let new_commands = self.api_context.take_client_commands_for(id.as_str());
            let _ = self
                .api_context
                .enqueue_client_command(id.as_str(), ClientCommand::ClearVisualOverrides);
            for queued in new_commands {
                let _ = self
                    .api_context
                    .enqueue_client_command(id.as_str(), queued.command);
            }
        }
        let replacement_resources =
            replacing.then(|| self.resource_registry.declarations_for_owner(id.as_str()));
        if let Some(mut old) = self.mods.insert(id.clone(), runtime) {
            old.unload();
            if let Some(declarations) = replacement_resources.as_deref() {
                self.resource_registry
                    .restore_owner(id.as_str(), declarations)
                    .expect("registrations captured from this registry must restore atomically");
            }
            self.scheduler.cancel_owner(&id);
            self.profiler.remove(&id);
        } else {
            self.load_order.push(id.clone());
        }
        Ok((id, denied))
    }

    fn loaded_mod_id(&self, id: &str) -> ScriptResult<ModId> {
        self.mods
            .keys()
            .find(|mod_id| mod_id.as_str() == id)
            .cloned()
            .ok_or_else(|| ScriptError::ModNotFound(id.to_owned()))
    }

    pub fn disable(&mut self, id: &str) -> ScriptResult<()> {
        let mod_id = self.loaded_mod_id(id)?;
        let runtime = self
            .mods
            .get(&mod_id)
            .expect("loaded mod id must have a runtime");
        if runtime.enabled
            && self.connection_active
            && runtime
                .manifest
                .permissions
                .contains(&Permission::ProtocolTranslate)
        {
            return Err(ScriptError::ReloadDenied(format!(
                "protocol translator mod '{id}' cannot change state during a connection"
            )));
        }

        self.mods
            .get_mut(&mod_id)
            .expect("loaded mod id must have a runtime")
            .unload();
        self.resource_registry.clear_owner(id);
        self.api_context.clear_client_commands_for(id);
        let _ = self
            .api_context
            .enqueue_client_command(id, ClientCommand::ClearVisualOverrides);
        self.scheduler.cancel_owner(&mod_id);
        self.profiler.remove(&mod_id);
        Ok(())
    }

    pub fn enable(&mut self, id: &str) -> ScriptResult<()> {
        let mod_id = self.loaded_mod_id(id)?;
        if self
            .mods
            .get(&mod_id)
            .is_some_and(|runtime| runtime.enabled)
        {
            return Ok(());
        }

        let loaded = self.loader.find_by_id(id)?;
        self.install(loaded, true).map(|_| ())
    }

    pub fn reload(&mut self, id: &str) -> ScriptResult<()> {
        let mod_id = self.loaded_mod_id(id)?;
        if !self
            .mods
            .get(&mod_id)
            .is_some_and(|runtime| runtime.enabled)
        {
            return Err(ScriptError::ReloadDenied(format!(
                "disabled mod '{id}' cannot reload; enable it first"
            )));
        }
        let loaded = self.loader.find_by_id(id)?;
        self.install(loaded, true).map(|_| ())
    }

    pub fn reload_all(&mut self) -> LoadReport {
        let ids = self.load_order.clone();
        let mut report = LoadReport::default();
        for id in ids {
            if !self.mods.get(&id).is_some_and(|runtime| runtime.enabled) {
                continue;
            }
            match self.reload(id.as_str()) {
                Ok(()) => report.loaded.push(id),
                Err(error) => report.errors.push(error.to_string()),
            }
        }
        let known = self.load_order.clone();
        let discovered = match self.loader.discover() {
            Ok(discovered) => discovered,
            Err(error) => {
                report.errors.push(error.to_string());
                return report;
            }
        };
        for loaded in discovered {
            if !known.contains(&loaded.manifest.id) {
                match self.install(loaded, false) {
                    Ok((id, denied)) => {
                        report.denied_permissions.extend(
                            denied
                                .into_iter()
                                .map(|permission| (id.clone(), permission)),
                        );
                        report.loaded.push(id);
                    }
                    Err(error) => report.errors.push(error.to_string()),
                }
            }
        }
        report
    }

    pub fn dispatch(&mut self, event: ScriptEvent) -> EventOutcome {
        let planned = self.plan_callbacks(&event.name);

        let outcome = Rc::new(RefCell::new(EventOutcome::default()));
        for callback in planned {
            let Some(runtime) = self.mods.get_mut(&callback.mod_id) else {
                continue;
            };
            let started = Instant::now();
            let result = runtime.dispatch_callback(callback.callback_id, &event, outcome.clone());
            let elapsed = started.elapsed();
            let failed = result.is_err();
            if let Err(error) = result {
                log::error!(
                    target: "rustcraft::lua",
                    "mod '{}' callback for '{}' failed: {error}",
                    callback.mod_id, event.name
                );
            }
            record_result_and_cleanup_resources(
                &self.resource_registry,
                &callback.mod_id,
                runtime,
                failed,
            );
            self.profiler
                .record_callback(&callback.mod_id, elapsed, failed);
            if outcome.borrow().consumed {
                break;
            }
        }
        let final_outcome = outcome.borrow().clone();
        final_outcome
    }

    fn plan_callbacks(&self, event_name: &str) -> Vec<PlannedCallback> {
        let mut planned = Vec::new();
        for (load_order, mod_id) in self.load_order.iter().enumerate() {
            let Some(runtime) = self.mods.get(mod_id) else {
                continue;
            };
            planned.extend(
                runtime
                    .descriptors(event_name)
                    .into_iter()
                    .map(|descriptor| PlannedCallback {
                        mod_id: mod_id.clone(),
                        callback_id: descriptor.id,
                        priority: descriptor.priority,
                        load_order,
                    }),
            );
        }
        self.event_bus.order(&mut planned);
        planned
    }

    fn dispatch_animation_stage(
        &mut self,
        event_name: &str,
        context: &FirstPersonAnimationContext,
    ) -> nalgebra::Matrix4<f32> {
        self.dispatch_animation_stage_with_entry(event_name, context, Matrix4::identity())
    }

    fn dispatch_animation_stage_with_entry(
        &mut self,
        event_name: &str,
        context: &FirstPersonAnimationContext,
        stage_entry: nalgebra::Matrix4<f32>,
    ) -> nalgebra::Matrix4<f32> {
        let planned = self.plan_callbacks(event_name);
        let transform = ScriptTransform::new(stage_entry);
        for callback in planned {
            let Some(runtime) = self.mods.get_mut(&callback.mod_id) else {
                continue;
            };
            let started = Instant::now();
            let result = runtime.dispatch_animation_callback(
                callback.callback_id,
                event_name,
                context,
                transform.clone(),
            );
            let elapsed = started.elapsed();
            let failed = result.is_err();
            if let Err(error) = result {
                log::error!(
                    target: "rustcraft::lua",
                    "mod '{}' callback for '{}' failed: {error}",
                    callback.mod_id, event_name
                );
            }
            record_result_and_cleanup_resources(
                &self.resource_registry,
                &callback.mod_id,
                runtime,
                failed,
            );
            self.profiler
                .record_callback(&callback.mod_id, elapsed, failed);
        }
        let result = transform.matrix();
        if !result.iter().all(|c| c.is_finite()) {
            log::warn!(
                target: "rustcraft::lua",
                "transform matrix for '{}' contains non-finite values; discarding",
                event_name
            );
            return stage_entry;
        }
        result
    }

    pub fn dispatch_first_person(
        &mut self,
        context: &FirstPersonAnimationContext,
    ) -> FirstPersonTransforms {
        // Run calculate stage: each mod applies its AnimationOverrides
        let merged_overrides = {
            let planned = self.plan_callbacks("animation.first_person.calculate");
            let shared_overrides = Rc::new(RefCell::new(AnimationOverrides::default()));
            for callback in &planned {
                let Some(runtime) = self.mods.get(&callback.mod_id) else {
                    continue;
                };
                let transform = ScriptTransform::new(Matrix4::identity());
                let started = Instant::now();
                let result = runtime.dispatch_animation_calculate(
                    callback.callback_id,
                    context,
                    transform.clone(),
                    shared_overrides.clone(),
                );
                let elapsed = started.elapsed();
                let failed = result.is_err();
                if let Err(error) = result {
                    log::error!(
                        target: "rustcraft::lua",
                        "mod '{}' callback for 'animation.first_person.calculate' failed: {error}",
                        callback.mod_id
                    );
                }
                if let Some(runtime) = self.mods.get_mut(&callback.mod_id) {
                    record_result_and_cleanup_resources(
                        &self.resource_registry,
                        &callback.mod_id,
                        runtime,
                        failed,
                    );
                }
                self.profiler
                    .record_callback(&callback.mod_id, elapsed, failed);
                // Store per-mod overrides for later use
                self.animation_overrides
                    .insert(callback.mod_id.clone(), shared_overrides.borrow().clone());
            }
            Rc::try_unwrap(shared_overrides)
                .map(RefCell::into_inner)
                .unwrap_or_else(|rc| rc.borrow().clone())
        };

        // Build final context with merged overrides
        let mut final_context = context.clone();
        self.merge_animation_overrides(&mut final_context, &merged_overrides);

        let shared =
            self.dispatch_animation_stage("animation.first_person.transform", &final_context);
        let arm =
            self.dispatch_animation_stage("animation.first_person.arm_transform", &final_context);
        let item =
            self.dispatch_animation_stage("animation.first_person.item_transform", &final_context);
        let transforms = FirstPersonTransforms {
            shared,
            arm,
            item,
            vanilla_flags: merged_overrides.vanilla.clone(),
        };

        self.dispatch_json(
            "animation.first_person.complete",
            serde_json::json!({
                "hand": context.hand.as_str(),
                "item_id": context.item_id,
                "state": {
                    "equip_progress": final_context.equip_progress,
                    "swing_progress": final_context.swing_progress,
                    "use_progress": final_context.use_progress,
                    "blocking": final_context.blocking,
                    "using_item": final_context.using_item,
                }
            }),
        );
        transforms
    }

    fn merge_animation_overrides(
        &self,
        context: &mut FirstPersonAnimationContext,
        overrides: &AnimationOverrides,
    ) {
        if let Some(swing) = overrides.swing_progress {
            context.previous_swing_progress = context.swing_progress;
            context.swing_progress = swing;
        }
        if let Some(equip) = overrides.equip_progress {
            context.previous_equip_progress = context.equip_progress;
            context.equip_progress = equip;
        }
        if let Some(use_p) = overrides.use_progress {
            context.use_progress = use_p;
        }
        if let Some(swinging) = overrides.swinging {
            context.swinging = swinging;
        }
        if let Some(duration) = overrides.swing_duration_ticks {
            context.swing_duration_ticks = duration;
        }
        if let Some(blocking) = overrides.blocking {
            context.blocking = blocking;
        }
        if let Some(using_item) = overrides.using_item {
            context.using_item = using_item;
        }
    }

    pub fn active_animation_overrides(&self) -> HashMap<ModId, AnimationOverrides> {
        self.animation_overrides
            .iter()
            .filter(|(id, _)| self.mods.get(*id).is_some_and(|r| r.enabled))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn clear_animation_overrides(&mut self, mod_id: &ModId) {
        self.animation_overrides.remove(mod_id);
    }

    pub fn dispatch_render(
        &mut self,
        event_name: &str,
        frame: ScriptFrameContext,
    ) -> Vec<ScriptDrawCommand> {
        let planned = self.plan_callbacks(event_name);
        let commands = Rc::new(RefCell::new(Vec::new()));
        for callback in planned {
            let Some(runtime) = self.mods.get_mut(&callback.mod_id) else {
                continue;
            };
            let started = Instant::now();
            let result = runtime.dispatch_render_callback(
                callback.callback_id,
                event_name,
                frame,
                commands.clone(),
            );
            let elapsed = started.elapsed();
            let failed = result.is_err();
            if let Err(error) = result {
                log::error!(
                    target: "rustcraft::lua",
                    "mod '{}' callback for '{}' failed: {error}",
                    callback.mod_id, event_name
                );
            }
            record_result_and_cleanup_resources(
                &self.resource_registry,
                &callback.mod_id,
                runtime,
                failed,
            );
            self.profiler
                .record_callback(&callback.mod_id, elapsed, failed);
        }
        Rc::try_unwrap(commands)
            .map(RefCell::into_inner)
            .unwrap_or_else(|commands| commands.borrow().clone())
    }

    pub fn process_packet(
        &mut self,
        event_name: &str,
        mut packet: DynamicPacket,
    ) -> PacketHookResult {
        if !self.has_callbacks(event_name) {
            return PacketHookResult {
                packet: Some(packet),
                modified: false,
            };
        }
        let original = packet.clone();
        let planned = self.plan_callbacks(event_name);
        for callback in planned {
            let Some(runtime) = self.mods.get_mut(&callback.mod_id) else {
                continue;
            };
            let before_callback = packet.clone();
            let started = Instant::now();
            let result =
                runtime.dispatch_packet_callback(callback.callback_id, event_name, &mut packet);
            let elapsed = started.elapsed();
            let failed = result.is_err();
            match result {
                Ok((_, control)) => {
                    let modified_fields = packet != before_callback;
                    let replaced = control.replacement.is_some();
                    if let Some(replacement) = control.replacement {
                        packet = replacement;
                    }
                    if modified_fields || replaced || control.cancelled {
                        self.network_audit.push_back(NetworkAuditEntry {
                            mod_id: callback.mod_id.clone(),
                            direction: packet.direction,
                            version: packet.version,
                            packet_name: packet.packet_name.clone(),
                            modified: modified_fields,
                            cancelled: control.cancelled,
                            replaced,
                            active_send: false,
                        });
                        while self.network_audit.len() > 1024 {
                            self.network_audit.pop_front();
                        }
                    }
                    record_result_and_cleanup_resources(
                        &self.resource_registry,
                        &callback.mod_id,
                        runtime,
                        false,
                    );
                    self.profiler
                        .record_callback(&callback.mod_id, elapsed, false);
                    if control.cancelled {
                        return PacketHookResult {
                            packet: None,
                            modified: true,
                        };
                    }
                }
                Err(error) => {
                    packet = before_callback;
                    log::error!(
                        target: "rustcraft::lua",
                        "mod '{}' callback for '{}' failed: {error}",
                        callback.mod_id, event_name
                    );
                    record_result_and_cleanup_resources(
                        &self.resource_registry,
                        &callback.mod_id,
                        runtime,
                        true,
                    );
                    self.profiler
                        .record_callback(&callback.mod_id, elapsed, true);
                }
            }
        }
        PacketHookResult {
            modified: packet != original,
            packet: Some(packet),
        }
    }

    pub fn translate_lua_inbound(
        &mut self,
        source: ProtocolVersion,
        target: ProtocolVersion,
        packet: DynamicPacket,
    ) -> ScriptResult<Vec<DynamicPacket>> {
        let plan = self.lua_translation_plan(source, target)?;
        self.run_lua_translation(plan.iter(), true, vec![packet])
    }

    pub fn translate_lua_outbound(
        &mut self,
        source: ProtocolVersion,
        target: ProtocolVersion,
        packet: DynamicPacket,
    ) -> ScriptResult<Vec<DynamicPacket>> {
        let plan = self.lua_translation_plan(source, target)?;
        self.run_lua_translation(plan.iter().rev(), false, vec![packet])
    }

    fn lua_translation_plan(
        &self,
        source: ProtocolVersion,
        target: ProtocolVersion,
    ) -> ScriptResult<Vec<PlannedLuaTranslator>> {
        if source == target {
            return Ok(Vec::new());
        }
        let mut available = Vec::new();
        for mod_id in &self.load_order {
            let Some(runtime) = self.mods.get(mod_id) else {
                continue;
            };
            available.extend(
                runtime
                    .translator_descriptors()
                    .into_iter()
                    .map(|descriptor| PlannedLuaTranslator {
                        mod_id: mod_id.clone(),
                        id: descriptor.id,
                        source: descriptor.source,
                        target: descriptor.target,
                    }),
            );
        }

        let mut queue = VecDeque::from([source]);
        let mut previous = HashMap::<ProtocolVersion, (ProtocolVersion, usize)>::new();
        while let Some(version) = queue.pop_front() {
            for (index, translator) in available.iter().enumerate() {
                if translator.source != version
                    || translator.target == source
                    || previous.contains_key(&translator.target)
                {
                    continue;
                }
                previous.insert(translator.target, (version, index));
                if translator.target == target {
                    queue.clear();
                    break;
                }
                queue.push_back(translator.target);
            }
        }
        if !previous.contains_key(&target) {
            return Err(ScriptError::ProtocolTranslation(format!(
                "no Lua translator path from {} to {}",
                source.0, target.0
            )));
        }
        let mut reversed = Vec::new();
        let mut cursor = target;
        while cursor != source {
            let (parent, index) = previous[&cursor];
            reversed.push(available[index].clone());
            cursor = parent;
        }
        reversed.reverse();
        Ok(reversed)
    }

    fn run_lua_translation<'a>(
        &mut self,
        plan: impl Iterator<Item = &'a PlannedLuaTranslator>,
        inbound: bool,
        mut packets: Vec<DynamicPacket>,
    ) -> ScriptResult<Vec<DynamicPacket>> {
        for translator in plan {
            let Some(runtime) = self.mods.get_mut(&translator.mod_id) else {
                return Err(ScriptError::ProtocolTranslation(format!(
                    "translator owner '{}' is not loaded",
                    translator.mod_id
                )));
            };
            let started = Instant::now();
            let mut translated = Vec::new();
            for packet in packets {
                match runtime.dispatch_protocol_translator(
                    &translator.id,
                    inbound,
                    &packet,
                    translator.source,
                    translator.target,
                ) {
                    Ok(output) => translated.extend(output),
                    Err(error) => {
                        record_result_and_cleanup_resources(
                            &self.resource_registry,
                            &translator.mod_id,
                            runtime,
                            true,
                        );
                        self.profiler
                            .record_callback(&translator.mod_id, started.elapsed(), true);
                        return Err(ScriptError::Lua(error));
                    }
                }
                if translated.len() > MAX_PROTOCOL_PIPELINE_PACKETS {
                    record_result_and_cleanup_resources(
                        &self.resource_registry,
                        &translator.mod_id,
                        runtime,
                        true,
                    );
                    self.profiler
                        .record_callback(&translator.mod_id, started.elapsed(), true);
                    return Err(ScriptError::ProtocolTranslation(format!(
                        "pipeline produced more than {MAX_PROTOCOL_PIPELINE_PACKETS} packets"
                    )));
                }
            }
            packets = translated;
            record_result_and_cleanup_resources(
                &self.resource_registry,
                &translator.mod_id,
                runtime,
                false,
            );
            self.profiler
                .record_callback(&translator.mod_id, started.elapsed(), false);
        }
        Ok(packets)
    }

    pub fn network_audit(&self) -> impl Iterator<Item = &NetworkAuditEntry> {
        self.network_audit.iter()
    }

    pub fn take_active_packets(&mut self) -> Vec<(ModId, DynamicPacket)> {
        let mut packets = Vec::new();
        for mod_id in &self.load_order {
            let Some(runtime) = self.mods.get(mod_id) else {
                continue;
            };
            for packet in runtime.take_outbound_packets() {
                self.network_audit.push_back(NetworkAuditEntry {
                    mod_id: mod_id.clone(),
                    direction: packet.direction,
                    version: packet.version,
                    packet_name: packet.packet_name.clone(),
                    modified: false,
                    cancelled: false,
                    replaced: false,
                    active_send: true,
                });
                packets.push((mod_id.clone(), packet));
            }
        }
        while self.network_audit.len() > 1024 {
            self.network_audit.pop_front();
        }
        packets
    }

    pub fn dispatch_json(&mut self, name: &str, payload: Value) -> EventOutcome {
        self.dispatch(ScriptEvent::new(name, payload))
    }

    pub fn mod_count(&self) -> usize {
        self.mods.len()
    }

    pub fn loaded_mods(&self) -> Vec<LoadedModInfo> {
        self.load_order
            .iter()
            .filter_map(|id| self.mods.get(id))
            .map(|runtime| {
                let mut granted_permissions = runtime
                    .permissions
                    .granted()
                    .map(|permission| permission.as_str().to_string())
                    .collect::<Vec<_>>();
                let mut denied_permissions = runtime
                    .permissions
                    .denied()
                    .map(|permission| permission.as_str().to_string())
                    .collect::<Vec<_>>();
                granted_permissions.sort();
                denied_permissions.sort();
                LoadedModInfo {
                    id: runtime.id.to_string(),
                    name: runtime.manifest.name.clone(),
                    version: runtime.manifest.version.clone(),
                    enabled: runtime.enabled,
                    protocol_translator: runtime
                        .manifest
                        .permissions
                        .contains(&Permission::ProtocolTranslate),
                    config_entries: runtime.config_entry_count(),
                    granted_permissions,
                    denied_permissions,
                }
            })
            .collect()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.mods
            .iter()
            .find(|(mod_id, _)| mod_id.as_str() == id)
            .map(|(_, runtime)| runtime.enabled)
            .unwrap_or(false)
    }

    pub fn config_entries(&self, id: &str) -> ScriptResult<Vec<ConfigEntrySnapshot>> {
        let mod_id = self.loaded_mod_id(id)?;
        Ok(self
            .mods
            .get(&mod_id)
            .expect("loaded mod id must have a runtime")
            .config_entries())
    }

    pub fn set_config_value(&self, id: &str, key: &str, value: ConfigValue) -> ScriptResult<()> {
        let mod_id = self.loaded_mod_id(id)?;
        let runtime = self
            .mods
            .get(&mod_id)
            .expect("loaded mod id must have a runtime");
        if self.connection_active
            && runtime
                .manifest
                .permissions
                .contains(&Permission::ProtocolTranslate)
        {
            return Err(ScriptError::ReloadDenied(format!(
                "protocol translator mod '{id}' cannot change configuration during a connection"
            )));
        }
        runtime.set_config_value(key, value)
    }

    pub fn shutdown(&mut self) {
        for runtime in self.mods.values_mut() {
            runtime.unload();
        }
        self.resource_registry.clear();
        for owner in std::mem::take(&mut self.client_override_owners) {
            self.api_context.clear_client_commands_for(owner.as_str());
            let _ = self
                .api_context
                .enqueue_client_command(owner.as_str(), ClientCommand::ClearVisualOverrides);
        }
        self.mods.clear();
        self.load_order.clear();
    }
}

impl Drop for ScriptManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

    struct TempMods(PathBuf);

    impl TempMods {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("rustcraft-lua-tests-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempMods {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_mod(root: &Path, id: &str, permissions: &[&str], source: &str) {
        let mod_root = root.join(id);
        fs::create_dir_all(mod_root.join("scripts")).unwrap();
        let permissions = permissions
            .iter()
            .map(|permission| format!(r#""{permission}""#))
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            mod_root.join("manifest.json"),
            format!(
                r#"{{"id":"{id}","name":"{id}","version":"1.0.0","api_version":1,"entrypoints":{{"client":"scripts/client.lua"}},"permissions":[{permissions}]}}"#
            ),
        )
        .unwrap();
        fs::write(mod_root.join("scripts/client.lua"), source).unwrap();
    }

    fn write_mod_asset(root: &Path, id: &str, relative: &str, bytes: &[u8]) {
        let path = root.join(id).join("assets").join(id).join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn loads_mods_and_isolates_globals() {
        let temp = TempMods::new();
        write_mod(&temp.0, "one", &[], "shared_name = 'one'");
        write_mod(
            &temp.0,
            "two",
            &[],
            "assert(shared_name == nil); shared_name = 'two'",
        );
        let mut manager = ScriptManager::new(&temp.0);
        let report = manager.load_all();
        assert!(report.is_clean(), "{:?}", report.errors);
        assert_eq!(manager.mod_count(), 2);
    }

    #[test]
    fn client_and_player_apis_are_permission_trimmed_and_read_live_state() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "no_client_access",
            &[],
            "assert(game.client == nil); assert(game.player == nil)",
        );
        write_mod(
            &temp.0,
            "client_reader",
            &["client.read"],
            r#"assert(game.client.snapshot ~= nil)
               assert(game.client.set_fov_override == nil)
               assert(game.player.snapshot ~= nil)
               game.events.on("client.tick", function(event)
                   local client = game.client.snapshot()
                   local player = game.player.snapshot()
                   event:set_result({ tick = client.tick, health = player.vitals.health })
               end)"#,
        );
        write_mod(
            &temp.0,
            "client_visual",
            &["client.modify"],
            r#"assert(game.client.snapshot == nil)
               assert(game.player == nil)
               game.client.set_fov_override(88)"#,
        );

        let mut manager = ScriptManager::new(&temp.0);
        let report = manager.load_all();
        assert!(report.is_clean(), "{:?}", report.errors);
        manager.update_client_snapshot(ClientSnapshot {
            tick: 73,
            player: Some(crate::scripting::api::context::PlayerSnapshot {
                vitals: crate::scripting::api::context::PlayerVitalsSnapshot {
                    health: 6.5,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        });

        let outcome = manager.dispatch_json("client.tick", serde_json::json!({}));
        assert_eq!(
            outcome.result,
            Some(serde_json::json!({"tick": 73, "health": 6.5}))
        );
        assert_eq!(
            manager.drain_client_commands(),
            vec![QueuedClientCommand {
                mod_id: "client_visual".into(),
                command: ClientCommand::SetFovOverride(Some(88.0)),
            }]
        );
    }

    #[test]
    fn fresh_and_reloaded_runtimes_start_with_the_latest_ui_snapshot() {
        let temp = TempMods::new();
        let source = r#"
            game.events.on("ui.probe", function(event)
                event:set_result(game.ui.screen().id)
            end)
        "#;
        write_mod(&temp.0, "ui_reader", &["ui.read"], source);

        let mut manager = ScriptManager::new(&temp.0);
        manager.update_ui_snapshot(UiSnapshot {
            screen: crate::scripting::api::ui::UiScreenSnapshot {
                id: "main_menu".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(manager.load_all().is_clean());
        assert_eq!(
            manager
                .dispatch_json("ui.probe", serde_json::json!({}))
                .result,
            Some(Value::String("main_menu".into()))
        );

        manager.update_ui_snapshot(UiSnapshot {
            screen: crate::scripting::api::ui::UiScreenSnapshot {
                id: "pause".into(),
                in_game: true,
                paused: true,
                ..Default::default()
            },
            ..Default::default()
        });
        write_mod(&temp.0, "ui_reader", &["ui.read"], source);
        manager.reload("ui_reader").unwrap();
        assert_eq!(
            manager
                .dispatch_json("ui.probe", serde_json::json!({}))
                .result,
            Some(Value::String("pause".into()))
        );
    }

    #[test]
    fn disabling_a_mod_clears_its_visual_overrides() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "visual_owner",
            &["client.modify"],
            "game.client.set_fov_override(82)",
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        assert_eq!(
            manager.drain_client_commands(),
            vec![QueuedClientCommand {
                mod_id: "visual_owner".into(),
                command: ClientCommand::SetFovOverride(Some(82.0)),
            }]
        );

        manager.disable("visual_owner").unwrap();
        assert_eq!(
            manager.drain_client_commands(),
            vec![QueuedClientCommand {
                mod_id: "visual_owner".into(),
                command: ClientCommand::ClearVisualOverrides,
            }]
        );
    }

    #[test]
    fn failed_reload_restores_the_previous_client_command_transaction() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "visual_transaction",
            &["client.modify"],
            "game.client.set_fov_override(77)",
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        fs::write(
            temp.0.join("visual_transaction").join("scripts/client.lua"),
            "game.client.set_fov_override(91); error('reload failed')",
        )
        .unwrap();
        assert!(manager.reload("visual_transaction").is_err());
        assert!(manager.is_enabled("visual_transaction"));
        assert_eq!(
            manager.drain_client_commands(),
            vec![QueuedClientCommand {
                mod_id: "visual_transaction".into(),
                command: ClientCommand::SetFovOverride(Some(77.0)),
            }]
        );
    }

    #[test]
    fn resource_registrations_are_removed_on_disable_and_shutdown() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "resource_lifecycle",
            &["resources.register"],
            r#"game.resources.register_alias(
                "resource_lifecycle:current",
                "resource_lifecycle:textures/icon.png"
            )"#,
        );
        write_mod_asset(&temp.0, "resource_lifecycle", "textures/icon.png", b"icon");
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("resource_lifecycle", Permission::ResourcesRegister);
        assert!(manager.load_all().is_clean());
        assert_eq!(manager.resource_registrations().len(), 1);

        manager.disable("resource_lifecycle").unwrap();
        assert!(manager.resource_registrations().is_empty());
        assert_eq!(
            manager
                .resolve_resource("resource_lifecycle:current")
                .unwrap(),
            "resource_lifecycle:current"
        );

        manager.enable("resource_lifecycle").unwrap();
        assert_eq!(manager.resource_registrations().len(), 1);
        manager.shutdown();
        assert!(manager.resource_registrations().is_empty());
    }

    #[test]
    fn failed_reload_restores_previous_resource_registrations() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "resource_transaction",
            &["resources.register"],
            r#"game.resources.register_alias(
                "resource_transaction:current",
                "resource_transaction:textures/old.png"
            )"#,
        );
        write_mod_asset(&temp.0, "resource_transaction", "textures/old.png", b"old");
        write_mod_asset(&temp.0, "resource_transaction", "textures/new.png", b"new");
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("resource_transaction", Permission::ResourcesRegister);
        assert!(manager.load_all().is_clean());

        write_mod(
            &temp.0,
            "resource_transaction",
            &["resources.register"],
            r#"game.resources.register_alias(
                "resource_transaction:current",
                "resource_transaction:textures/new.png"
            )
            error("reload failed")"#,
        );
        assert!(manager.reload("resource_transaction").is_err());
        assert!(manager.is_enabled("resource_transaction"));
        assert_eq!(manager.resource_registrations().len(), 1);
        assert_eq!(
            manager
                .resolve_resource("resource_transaction:current")
                .unwrap(),
            "resource_transaction:textures/old.png"
        );
    }

    #[test]
    fn successful_reload_keeps_new_resources_after_old_on_unload() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "resource_replacement",
            &["resources.register"],
            r#"game.resources.register_alias(
                "resource_replacement:current",
                "resource_replacement:textures/old.png"
            )
            function on_unload()
                game.resources.register_alias(
                    "resource_replacement:current",
                    "resource_replacement:textures/old.png"
                )
            end"#,
        );
        write_mod_asset(&temp.0, "resource_replacement", "textures/old.png", b"old");
        write_mod_asset(&temp.0, "resource_replacement", "textures/new.png", b"new");
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("resource_replacement", Permission::ResourcesRegister);
        assert!(manager.load_all().is_clean());

        write_mod(
            &temp.0,
            "resource_replacement",
            &["resources.register"],
            r#"game.resources.register_alias(
                "resource_replacement:current",
                "resource_replacement:textures/new.png"
            )"#,
        );
        manager.reload("resource_replacement").unwrap();
        assert_eq!(manager.resource_registrations().len(), 1);
        assert_eq!(
            manager
                .resolve_resource("resource_replacement:current")
                .unwrap(),
            "resource_replacement:textures/new.png"
        );
    }

    #[test]
    fn automatic_runtime_disable_removes_resource_registrations() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "resource_crash",
            &["resources.register"],
            r#"game.resources.register_alias(
                "resource_crash:current",
                "resource_crash:textures/icon.png"
            )
            game.events.on("custom", function() error("boom") end)"#,
        );
        write_mod_asset(&temp.0, "resource_crash", "textures/icon.png", b"icon");
        let mut manager = ScriptManager::new(&temp.0).with_limits(RuntimeLimits {
            consecutive_error_limit: 1,
            ..RuntimeLimits::default()
        });
        manager
            .permission_policy_mut()
            .approve_for("resource_crash", Permission::ResourcesRegister);
        assert!(manager.load_all().is_clean());
        assert_eq!(manager.resource_registrations().len(), 1);

        manager.dispatch_json("custom", serde_json::json!({}));
        assert!(!manager.is_enabled("resource_crash"));
        assert!(manager.resource_registrations().is_empty());
    }

    #[test]
    fn loaded_mod_snapshot_reports_effective_permissions() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "snapshot",
            &["network.observe", "network.send", "protocol.translate"],
            "assert(game.network ~= nil)",
        );
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("snapshot", Permission::NetworkSend);
        assert!(manager.load_all().is_clean());

        let mods = manager.loaded_mods();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].id, "snapshot");
        assert!(mods[0].enabled);
        assert!(mods[0].protocol_translator);
        assert_eq!(mods[0].config_entries, 0);
        assert_eq!(
            mods[0].granted_permissions,
            vec!["network.observe", "network.send"]
        );
        assert_eq!(mods[0].denied_permissions, vec!["protocol.translate"]);
    }

    #[test]
    fn config_snapshot_writes_validate_and_survive_reload() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "configured",
            &[],
            r#"
                settings = game.config.define({
                    { key = "enabled", type = "boolean", default = true },
                    { key = "strength", type = "number", default = 1, min = 0, max = 2, step = 0.1 },
                    { key = "style", type = "choice", default = "classic", options = { "classic", "subtle" } }
                })
            "#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        assert_eq!(manager.loaded_mods()[0].config_entries, 3);

        manager
            .set_config_value("configured", "enabled", ConfigValue::Boolean(false))
            .unwrap();
        manager
            .set_config_value("configured", "strength", ConfigValue::Number(1.5))
            .unwrap();
        manager
            .set_config_value("configured", "style", ConfigValue::Choice("subtle".into()))
            .unwrap();
        assert!(manager
            .set_config_value("configured", "strength", ConfigValue::Number(3.0))
            .is_err());
        assert!(manager
            .set_config_value("configured", "style", ConfigValue::Choice("unknown".into()))
            .is_err());

        manager.reload("configured").unwrap();
        let entries = manager.config_entries("configured").unwrap();
        assert_eq!(entries[0].value, ConfigValue::Boolean(false));
        assert_eq!(entries[1].value, ConfigValue::Number(1.5));
        assert_eq!(entries[2].value, ConfigValue::Choice("subtle".into()));
    }

    #[test]
    fn dispatches_by_priority_and_supports_cancellation() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "events",
            &[],
            r#"
                game.events.on("custom", { priority = 500, callback = function(event)
                    event:set_result("high")
                    event:cancel()
                end })
                game.events.on("custom", function(event)
                    assert(event.name == "custom")
                end)
            "#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        let outcome = manager.dispatch_json("custom", serde_json::json!({}));
        assert!(outcome.cancelled);
        assert_eq!(outcome.result, Some(Value::String("high".into())));
    }

    #[test]
    fn callback_errors_are_isolated() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "broken",
            &[],
            r#"game.events.on("custom", {priority=500, callback=function() error("boom") end})"#,
        );
        write_mod(
            &temp.0,
            "healthy",
            &[],
            r#"game.events.on("custom", function(event) event:set_result("ok") end)"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        let outcome = manager.dispatch_json("custom", serde_json::json!({}));
        assert_eq!(outcome.result, Some(Value::String("ok".into())));
    }

    #[test]
    fn instruction_budget_stops_runaway_entrypoint() {
        let temp = TempMods::new();
        write_mod(&temp.0, "looping", &[], "while true do end");
        let mut manager = ScriptManager::new(&temp.0).with_limits(RuntimeLimits {
            instructions_per_call: 10_000,
            ..RuntimeLimits::default()
        });
        let report = manager.load_all();
        assert_eq!(manager.mod_count(), 0);
        assert!(report.errors.iter().any(|error| error.contains("budget")));
    }

    #[test]
    fn memory_limit_stops_large_script_allocation() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "memory_hog",
            &[],
            "local value = string.rep('x', 4 * 1024 * 1024)",
        );
        let mut manager = ScriptManager::new(&temp.0).with_limits(RuntimeLimits {
            memory_bytes: 512 * 1024,
            ..RuntimeLimits::default()
        });
        let report = manager.load_all();
        assert_eq!(manager.mod_count(), 0);
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn reload_replaces_old_state_without_duplicate_listeners() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "reloadable",
            &[],
            r#"game.events.on("custom", function(event) event:set_result("old") end)"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        write_mod(
            &temp.0,
            "reloadable",
            &[],
            r#"game.events.on("custom", function(event) event:set_result("new") end)"#,
        );
        manager.reload("reloadable").unwrap();
        let outcome = manager.dispatch_json("custom", serde_json::json!({}));
        assert_eq!(outcome.result, Some(Value::String("new".into())));
        let mod_id = ModId::parse("reloadable").unwrap();
        assert_eq!(
            manager.profiler.profile(&mod_id).unwrap().callback_count,
            1,
            "a reload must leave exactly one live callback"
        );
    }

    #[test]
    fn disable_unloads_callbacks_and_enable_reloads_from_disk() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "toggleable",
            &[],
            r#"game.events.on("custom", function(event) event:set_result("before") end)"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        assert_eq!(
            manager
                .dispatch_json("custom", serde_json::json!({}))
                .result,
            Some(Value::String("before".into()))
        );

        manager.disable("toggleable").unwrap();
        assert!(!manager.is_enabled("toggleable"));
        assert_eq!(manager.mod_count(), 1, "disabled mods remain manageable");
        assert_eq!(manager.loaded_mods().len(), 1);
        assert!(!manager.loaded_mods()[0].enabled);
        assert_eq!(
            manager
                .dispatch_json("custom", serde_json::json!({}))
                .result,
            None,
            "disabled callbacks must not run"
        );

        write_mod(
            &temp.0,
            "toggleable",
            &[],
            r#"game.events.on("custom", function(event) event:set_result("after") end)"#,
        );
        assert!(matches!(
            manager.reload("toggleable"),
            Err(ScriptError::ReloadDenied(_))
        ));
        let report = manager.reload_all();
        assert!(report.is_clean(), "{:?}", report.errors);
        assert!(report.loaded.is_empty());
        assert!(!manager.is_enabled("toggleable"));

        manager.enable("toggleable").unwrap();
        assert!(manager.is_enabled("toggleable"));
        assert_eq!(
            manager
                .dispatch_json("custom", serde_json::json!({}))
                .result,
            Some(Value::String("after".into())),
            "enabling must build a fresh runtime from disk"
        );
    }

    #[test]
    fn protocol_translator_state_changes_are_locked_during_connections() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "translator",
            &["protocol.translate"],
            r#"game.config.define({ { key = "enabled", type = "boolean", default = true } })"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("translator", Permission::ProtocolTranslate);
        assert!(manager.load_all().is_clean());

        manager.set_connection_active(true);
        assert!(matches!(
            manager.disable("translator"),
            Err(ScriptError::ReloadDenied(_))
        ));
        assert!(matches!(
            manager.reload("translator"),
            Err(ScriptError::ReloadDenied(_))
        ));
        assert!(matches!(
            manager.set_config_value("translator", "enabled", ConfigValue::Boolean(false)),
            Err(ScriptError::ReloadDenied(_))
        ));
        assert!(manager.is_enabled("translator"));

        manager.set_connection_active(false);
        manager.disable("translator").unwrap();
        manager.set_connection_active(true);
        assert!(matches!(
            manager.enable("translator"),
            Err(ScriptError::ReloadDenied(_))
        ));
        assert!(!manager.is_enabled("translator"));

        let report = manager.reload_all();
        assert!(report.is_clean(), "{:?}", report.errors);
        assert!(report.loaded.is_empty());
        assert!(!manager.is_enabled("translator"));

        manager.set_connection_active(false);
        manager.enable("translator").unwrap();
        assert!(manager.is_enabled("translator"));
    }

    fn animation_context(
        hand: crate::render::first_person::Hand,
        blocking: bool,
        swing_progress: f32,
    ) -> FirstPersonAnimationContext {
        FirstPersonAnimationContext {
            hand,
            item_id: "minecraft:diamond_sword".into(),
            numeric_item_id: 276,
            item_type: crate::render::first_person::ItemType::Sword,
            use_action: if blocking {
                crate::render::first_person::UseAction::Block
            } else {
                crate::render::first_person::UseAction::None
            },
            equip_progress: 0.0,
            previous_equip_progress: 0.0,
            swing_progress,
            previous_swing_progress: 0.0,
            swinging: swing_progress > 0.0,
            swing_duration_ticks: 6,
            use_progress: 0.5,
            use_ticks: 0,
            remaining_use_ticks: 0,
            max_use_ticks: 0,
            attack_cooldown: 1.0,
            using_item: blocking,
            blocking,
            attack_pressed: false,
            attack_held: false,
            use_pressed: false,
            use_held: false,
            sneaking: false,
            yaw: 0.0,
            pitch: 0.0,
            partial_tick: 0.5,
            fov: 70.0,
            aspect_ratio: 16.0 / 9.0,
        }
    }

    #[test]
    fn old_animations_mod_only_changes_blocking_main_hand_vanilla_layers() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "old_animations",
            &[
                "animation.modify",
                "render.read",
                "render.custom_draw",
                "client.read",
                "input.observe",
            ],
            include_str!("../../mods/old_animations/scripts/client.lua"),
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());

        let idle = manager.dispatch_first_person(&animation_context(
            crate::render::first_person::Hand::MainHand,
            false,
            0.5,
        ));
        assert!(idle.shared.abs().max() <= 1.0);
        assert_eq!(idle.shared, nalgebra::Matrix4::identity());

        let blocking_start = manager.dispatch_first_person(&animation_context(
            crate::render::first_person::Hand::MainHand,
            true,
            0.0,
        ));
        let blocking_swing = manager.dispatch_first_person(&animation_context(
            crate::render::first_person::Hand::MainHand,
            true,
            0.5,
        ));
        assert_eq!(blocking_start.shared, nalgebra::Matrix4::identity());
        assert_eq!(blocking_start.shared, blocking_swing.shared);
        assert!(blocking_start.vanilla_flags.base);
        assert!(blocking_start.vanilla_flags.equip);
        assert!(blocking_start.vanilla_flags.use_transform);
        assert!(blocking_start.vanilla_flags.block_transform);
        // 1.7-style BlockHit keeps the vanilla swing chain for its short
        // forward push; the mod only adds a small guard-pivot recoil.
        assert!(blocking_start.vanilla_flags.swing);
        assert!(blocking_swing.vanilla_flags.swing);

        let off_hand = manager.dispatch_first_person(&animation_context(
            crate::render::first_person::Hand::OffHand,
            true,
            0.5,
        ));
        assert_eq!(off_hand.shared, nalgebra::Matrix4::identity());
        assert!(off_hand.vanilla_flags.swing);

        let commands = manager.dispatch_render(
            "render.hud.after",
            ScriptFrameContext {
                delta_time: 1.0 / 60.0,
                viewport_width: 1920,
                viewport_height: 1080,
            },
        );
        assert!(commands.is_empty());
    }

    #[test]
    fn custom_draw_permission_is_enforced_when_commands_are_created() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "no_draw",
            &["render.read"],
            r#"game.events.on("render.hud.after", function(event)
                event.draw:rect({x=0,y=0,width=10,height=10})
            end)"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        let commands = manager.dispatch_render(
            "render.hud.after",
            ScriptFrameContext {
                delta_time: 0.016,
                viewport_width: 1280,
                viewport_height: 720,
            },
        );
        assert!(commands.is_empty());
    }

    #[test]
    fn structured_packet_fields_can_be_modified_replaced_and_cancelled() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "packet_mod",
            &["network.observe", "network.modify", "network.cancel"],
            r#"game.events.on("network.packet.inbound", function(event)
                if event.packet.fields.message == "modify" then
                    event.packet.fields.message = "changed"
                elseif event.packet.fields.message == "replace" then
                    event:replace({
                        name = "clientbound_disconnect",
                        fields = { reason = "replaced" }
                    })
                elseif event.packet.fields.message == "cancel" then
                    event:cancel()
                end
            end)"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("packet_mod", Permission::NetworkModify);
        manager
            .permission_policy_mut()
            .approve_for("packet_mod", Permission::NetworkCancel);
        assert!(manager.load_all().is_clean());

        let packet = |message: &str| DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Inbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: crate::net::dynamic_packet::ProtocolVersion::V1_8_9,
            packet_id: 0x02,
            packet_name: Some("clientbound_chat_message".into()),
            fields: serde_json::json!({"message": message, "position": 0}),
            raw_payload: None,
        };

        let modified = manager.process_packet("network.packet.inbound", packet("modify"));
        assert_eq!(modified.packet.unwrap().fields["message"], "changed");

        let replaced = manager.process_packet("network.packet.inbound", packet("replace"));
        assert_eq!(
            replaced.packet.unwrap().packet_name.as_deref(),
            Some("clientbound_disconnect")
        );

        let cancelled = manager.process_packet("network.packet.inbound", packet("cancel"));
        assert!(cancelled.packet.is_none());
        assert_eq!(manager.network_audit().count(), 3);
    }

    #[test]
    fn observer_cannot_modify_packet_without_sensitive_approval() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "observer",
            &["network.observe"],
            r#"game.events.on("network.packet.inbound", function(event)
                event.packet.fields.message = "forbidden"
            end)"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        assert!(manager.load_all().is_clean());
        let original = DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Inbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: crate::net::dynamic_packet::ProtocolVersion::V1_8_9,
            packet_id: 0x02,
            packet_name: Some("clientbound_chat_message".into()),
            fields: serde_json::json!({"message": "original", "position": 0}),
            raw_payload: None,
        };
        let result = manager.process_packet("network.packet.inbound", original.clone());
        assert_eq!(result.packet, Some(original));
    }

    #[test]
    fn active_send_api_is_absent_without_approval_and_rate_limited_when_approved() {
        let denied = TempMods::new();
        write_mod(
            &denied.0,
            "denied_sender",
            &["network.send"],
            "assert(game.network == nil)",
        );
        let mut denied_manager = ScriptManager::new(&denied.0);
        let denied_report = denied_manager.load_all();
        assert!(denied_report.is_clean());
        assert_eq!(denied_manager.take_active_packets().len(), 0);

        let allowed = TempMods::new();
        write_mod(
            &allowed.0,
            "allowed_sender",
            &["network.send"],
            r#"for i = 1, 25 do
                pcall(function()
                    game.network.send({
                        name = "serverbound_custom_payload",
                        fields = { channel = "example:test", data = { i } }
                    })
                end)
            end"#,
        );
        let mut allowed_manager = ScriptManager::new(&allowed.0);
        allowed_manager
            .permission_policy_mut()
            .approve_for("allowed_sender", Permission::NetworkSend);
        assert!(allowed_manager.load_all().is_clean());
        let packets = allowed_manager.take_active_packets();
        assert_eq!(packets.len(), 20);
        assert!(packets
            .iter()
            .all(|(_, packet)| packet.encode_v47_serverbound().is_ok()));
        assert_eq!(
            allowed_manager
                .network_audit()
                .filter(|entry| entry.active_send)
                .count(),
            20
        );
    }

    #[test]
    fn packet_filter_only_invokes_callback_for_selected_names() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "filtered",
            &["network.observe", "network.cancel"],
            r#"game.network.on_packet({
                direction = "inbound",
                names = { "clientbound_chat_message" },
                callback = function(event) event:cancel() end
            })"#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("filtered", Permission::NetworkCancel);
        assert!(manager.load_all().is_clean());

        let chat = DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Inbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: crate::net::dynamic_packet::ProtocolVersion::V1_8_9,
            packet_id: 0x02,
            packet_name: Some("clientbound_chat_message".into()),
            fields: serde_json::json!({"message":"hello","position":0}),
            raw_payload: None,
        };
        assert!(manager
            .process_packet("network.packet.inbound", chat)
            .packet
            .is_none());

        let payload = DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Inbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: crate::net::dynamic_packet::ProtocolVersion::V1_8_9,
            packet_id: 0x3F,
            packet_name: Some("clientbound_custom_payload".into()),
            fields: serde_json::json!({"channel":"example:test","data":[]}),
            raw_payload: None,
        };
        assert!(manager
            .process_packet("network.packet.inbound", payload)
            .packet
            .is_some());
    }

    #[test]
    fn lua_protocol_translators_require_approval_and_support_chained_fan_out() {
        let denied = TempMods::new();
        write_mod(
            &denied.0,
            "denied_translator",
            &["protocol.translate"],
            "assert(game.protocol == nil)",
        );
        let mut denied_manager = ScriptManager::new(&denied.0);
        assert!(denied_manager.load_all().is_clean());

        let allowed = TempMods::new();
        write_mod(
            &allowed.0,
            "translator",
            &["protocol.translate"],
            r#"
            game.protocol.register_translator({
                id = "test:one_to_two", source = 1, target = 2,
                inbound = function(packet, context)
                    assert(context.source_version == 1 and context.target_version == 2)
                    return {
                        { id = 10, name = "test:middle", fields = { trace = { "1>2" } } },
                        { id = 11, name = "test:middle", fields = { trace = { "1>2" } } }
                    }
                end,
                outbound = function(packet, context)
                    table.insert(packet.fields.trace, "1<2")
                    return packet
                end
            })
            game.protocol.register_translator({
                id = "test:two_to_three", source = 2, target = 3,
                inbound = function(packet, context)
                    table.insert(packet.fields.trace, "2>3")
                    packet.id = packet.id + 100
                    packet.name = "test:canonical"
                    return packet
                end,
                outbound = function(packet, context)
                    packet.fields.trace = { "2<3" }
                    return packet
                end
            })
            "#,
        );
        let mut manager = ScriptManager::new(&allowed.0);
        manager
            .permission_policy_mut()
            .approve_for("translator", Permission::ProtocolTranslate);
        assert!(manager.load_all().is_clean());

        let inbound = DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Inbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: ProtocolVersion(1),
            packet_id: 1,
            packet_name: Some("test:source".into()),
            fields: serde_json::json!({}),
            raw_payload: None,
        };
        let translated = manager
            .translate_lua_inbound(ProtocolVersion(1), ProtocolVersion(3), inbound)
            .unwrap();
        assert_eq!(translated.len(), 2);
        assert_eq!(translated[0].version, ProtocolVersion(3));
        assert_eq!(translated[0].packet_id, 110);
        assert_eq!(translated[1].packet_id, 111);
        assert_eq!(
            translated[0].fields["trace"],
            serde_json::json!(["1>2", "2>3"])
        );

        let outbound = DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Outbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: ProtocolVersion(3),
            packet_id: 20,
            packet_name: Some("test:canonical".into()),
            fields: serde_json::json!({}),
            raw_payload: None,
        };
        let translated = manager
            .translate_lua_outbound(ProtocolVersion(1), ProtocolVersion(3), outbound)
            .unwrap();
        assert_eq!(translated.len(), 1);
        assert_eq!(translated[0].version, ProtocolVersion(1));
        assert_eq!(
            translated[0].fields["trace"],
            serde_json::json!(["2<3", "1<2"])
        );
    }

    #[test]
    fn lua_protocol_translation_pipeline_limit_is_profiled_as_error() {
        let temp = TempMods::new();
        write_mod(
            &temp.0,
            "translator",
            &["protocol.translate"],
            r#"
            game.protocol.register_translator({
                id = "test:one_to_two", source = 1, target = 2,
                inbound = function(packet, context)
                    local packets = {}
                    for i = 1, 5 do
                        packets[i] = { id = i, name = "test:middle", fields = {} }
                    end
                    return packets
                end,
                outbound = function(packet, context) return packet end
            })
            game.protocol.register_translator({
                id = "test:two_to_three", source = 2, target = 3,
                inbound = function(packet, context)
                    local packets = {}
                    for i = 1, 16 do
                        packets[i] = { id = i, name = "test:target", fields = {} }
                    end
                    return packets
                end,
                outbound = function(packet, context) return packet end
            })
            "#,
        );
        let mut manager = ScriptManager::new(&temp.0);
        manager
            .permission_policy_mut()
            .approve_for("translator", Permission::ProtocolTranslate);
        assert!(manager.load_all().is_clean());

        let inbound = DynamicPacket {
            direction: crate::net::dynamic_packet::PacketDirection::Inbound,
            state: crate::net::dynamic_packet::DynamicProtocolState::Play,
            version: ProtocolVersion(1),
            packet_id: 1,
            packet_name: Some("test:source".into()),
            fields: serde_json::json!({}),
            raw_payload: None,
        };
        let error = manager
            .translate_lua_inbound(ProtocolVersion(1), ProtocolVersion(3), inbound)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("pipeline produced more than 64 packets"));

        let (_, callback_count, _) = manager.take_frame_profile();
        assert_eq!(callback_count, 2);
        let profile = manager
            .profiler
            .profile(&ModId::parse("translator").unwrap())
            .unwrap();
        assert_eq!(profile.callback_count, 2);
        assert_eq!(profile.error_count, 1);
    }
}
