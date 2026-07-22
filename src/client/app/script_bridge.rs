//! Main-thread bridge between the game client and sandboxed Lua states.
//!
//! The scripting runtime only sees owned snapshots. Lua writes are represented
//! by typed command queues which are drained here, on the winit/game thread.

use std::collections::{BTreeMap, HashSet};

use winit::window::Fullscreen;

use super::App;
use crate::client::keybind::Action;
use crate::client::state::GameState;
use crate::entity::EntityData;
use crate::scripting::api::context::{
    CameraMode, ClientCommand, ClientSettingsSnapshot, ClientSnapshot, ConnectionSnapshot,
    PlayerActionSnapshot, PlayerCapabilitiesSnapshot, PlayerExperienceSnapshot,
    PlayerMovementSnapshot, PlayerRotationSnapshot, PlayerSnapshot, PlayerVitalsSnapshot,
    Vec3Snapshot, WindowSnapshot,
};
use crate::scripting::api::input::{canonical_action_name, InputEdge, InputSnapshot};
use crate::scripting::api::ui::{
    UiChatSnapshot, UiCommand, UiGuiSnapshot, UiInventorySnapshot, UiItemSnapshot,
    UiScreenSnapshot, UiSnapshot, UiWindowSnapshot,
};
use crate::scripting::api::world::{
    BlockSnapshot, EntitySnapshot, WeatherSnapshot, WorldSnapshot, MAX_BLOCK_QUERY_RADIUS,
};

#[derive(Clone, Debug, Default)]
pub(super) struct ScriptVisualOverrides {
    fov: Option<f32>,
    view_bobbing: Option<bool>,
    hud_visible: Option<bool>,
    camera_mode: Option<CameraMode>,
    window_title: Option<String>,
    crosshair_visible: Option<bool>,
    hurt_cam: Option<bool>,
    fov_change: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ScriptWorldBlockUpdate {
    Replace,
    Reuse,
    Partial {
        previous_center: Option<(i32, i32, i32)>,
        changed_blocks: HashSet<(i32, i32, i32)>,
    },
}

impl App {
    /// Publish immutable API snapshots. World data is comparatively expensive,
    /// so callers request it only for logical ticks; disconnect always clears it.
    pub(super) fn publish_script_snapshots(
        &mut self,
        frame_delta_seconds: f32,
        include_world: bool,
    ) {
        let client = self.script_client_snapshot(frame_delta_seconds);
        let input = InputSnapshot::from_input_state(
            &self.input_ctrl.input,
            (self.input_ctrl.mouse_dx, self.input_ctrl.mouse_dy),
            self.input_ctrl.mouse_captured,
        );
        let ui = self.script_ui_snapshot();
        let world_update = if include_world {
            self.script_world_snapshot_update()
        } else {
            None
        };

        self.scripts.update_client_snapshot(client);
        self.scripts.update_input_snapshot(input);
        self.scripts.update_ui_snapshot(ui);
        if include_world {
            match world_update {
                Some((snapshot, key, ScriptWorldBlockUpdate::Replace)) => {
                    self.scripts.update_world_snapshot(snapshot);
                    self.last_script_world_block_key = Some(key);
                }
                Some((snapshot, key, ScriptWorldBlockUpdate::Reuse)) => {
                    if self.scripts.update_world_snapshot_reusing_blocks(snapshot) {
                        self.last_script_world_block_key = Some(key);
                    } else {
                        self.last_script_world_block_key = None;
                    }
                }
                Some((snapshot, key, ScriptWorldBlockUpdate::Partial { .. })) => {
                    if self.scripts.update_world_snapshot_merging_blocks(snapshot) {
                        self.last_script_world_block_key = Some(key);
                    } else {
                        self.last_script_world_block_key = None;
                    }
                }
                None => {
                    self.scripts.clear_world_snapshot();
                    self.last_script_world_block_key = None;
                }
            }
        } else if !self.has_script_player() {
            self.scripts.clear_world_snapshot();
            self.last_script_world_block_key = None;
        }
    }

    /// Drain all effects before touching App/window/renderer state. This avoids
    /// retaining a mutable ScriptManager borrow while commands are applied.
    pub(super) fn apply_script_commands(&mut self) {
        let client_commands = self.scripts.drain_client_commands();
        let ui_commands = self.scripts.drain_ui_commands();

        for queued in client_commands {
            match queued.command {
                ClientCommand::ClearVisualOverrides => {
                    self.script_visual_overrides.remove(&queued.mod_id);
                }
                ClientCommand::SetFovOverride(value) => {
                    self.script_overrides_mut(&queued.mod_id).fov = value;
                }
                ClientCommand::SetViewBobbingOverride(value) => {
                    self.script_overrides_mut(&queued.mod_id).view_bobbing = value;
                }
                ClientCommand::SetHudVisibilityOverride(value) => {
                    self.script_overrides_mut(&queued.mod_id).hud_visible = value;
                }
                ClientCommand::SetCameraModeOverride(value) => {
                    self.script_overrides_mut(&queued.mod_id).camera_mode = value;
                }
                ClientCommand::SetWindowTitle(value) => {
                    self.script_overrides_mut(&queued.mod_id).window_title = value;
                }
                ClientCommand::SetHurtcamOverride(value) => {
                    self.script_overrides_mut(&queued.mod_id).hurt_cam = value;
                }
                ClientCommand::SetFovchangeOverride(value) => {
                    self.script_overrides_mut(&queued.mod_id).fov_change = value;
                }
                ClientCommand::SetFullscreen(fullscreen) => {
                    if let Some(window) = &self.window {
                        window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
                    }
                }
            }
        }

        for queued in ui_commands {
            match queued.command {
                UiCommand::ShowSystemMessage { text } => self.session.push_system_line(text),
                UiCommand::OpenChat { initial_text } => self.open_chat(&initial_text),
                UiCommand::CloseChat => self.close_chat(true),
                UiCommand::SetHudVisible { visible } => {
                    self.script_overrides_mut(&queued.mod_id).hud_visible = Some(visible);
                }
                UiCommand::SetCrosshairVisible { visible } => {
                    self.script_overrides_mut(&queued.mod_id).crosshair_visible = Some(visible);
                }
            }
        }

        // Runtime circuit-breakers can disable a mod without an explicit
        // manager disable call. Pruning here makes those effects disappear on
        // the same frame.
        let enabled: HashSet<_> = self
            .scripts
            .loaded_mods()
            .into_iter()
            .filter(|info| info.enabled)
            .map(|info| info.id)
            .collect();
        self.script_visual_overrides
            .retain(|mod_id, _| enabled.contains(mod_id));

        self.apply_script_visual_overrides();
    }

    pub(super) fn apply_script_visual_overrides(&mut self) {
        self.player.camera.fov = self.script_effective_fov();
        self.player.camera.view_bobbing = self.script_effective_view_bobbing();

        let camera_override = self.script_effective_camera_mode();
        match (camera_override, self.script_base_camera_mode) {
            (Some(mode), None) => {
                self.script_base_camera_mode = Some(self.player.camera_mode);
                self.player.camera_mode = camera_mode_number(mode);
            }
            (Some(mode), Some(_)) => self.player.camera_mode = camera_mode_number(mode),
            (None, Some(base)) => {
                self.player.camera_mode = base;
                self.script_base_camera_mode = None;
            }
            (None, None) => {}
        }

        let hud_visible = !self.hud_hidden && self.script_effective_hud_visible();
        let crosshair_visible = !self.hud_hidden && self.script_effective_crosshair_visible();
        if let Some(renderer) = &mut self.renderer {
            renderer.state.settings.set_hud_visible(hud_visible);
            renderer.state.settings.set_crosshair_visible(crosshair_visible);
        }

        // Window title is fixed globally to match app branding/version.
    }

    /// Dispatch a real input edge after InputState has observed it. Both the
    /// event outcome and `game.input.consume` requests can suppress the edge.
    /// Pause remains observable but intentionally cannot be suppressed.
    pub(super) fn dispatch_script_input_edge(
        &mut self,
        action: Action,
        edge: InputEdge,
        repeat: bool,
    ) -> bool {
        self.scripts
            .update_input_snapshot(InputSnapshot::from_input_state(
                &self.input_ctrl.input,
                (self.input_ctrl.mouse_dx, self.input_ctrl.mouse_dy),
                self.input_ctrl.mouse_captured,
            ));
        let outcome = self.scripts.dispatch_json(
            "input.action",
            serde_json::json!({
                "action": canonical_action_name(action),
                "edge": edge.as_str(),
                "held": self.input_ctrl.input.is_held(action),
                "repeat": repeat,
            }),
        );
        let requested = self
            .scripts
            .drain_input_consume_requests()
            .into_iter()
            .any(|request| request.action == action && request.edge == edge);
        self.apply_script_commands();
        action != Action::Pause && (outcome.consumed || requested)
    }

    pub(super) fn script_effective_fov(&self) -> f32 {
        self.fold_script_overrides(self.config.fov, |overrides| overrides.fov)
    }

    pub(super) fn script_effective_view_bobbing(&self) -> bool {
        self.fold_script_overrides(self.config.view_bobbing, |overrides| overrides.view_bobbing)
    }

    pub(super) fn script_window_title_override(&self) -> Option<String> {
        self.fold_script_overrides(None, |overrides| overrides.window_title.clone().map(Some))
    }

    fn script_effective_hud_visible(&self) -> bool {
        self.fold_script_overrides(true, |overrides| overrides.hud_visible)
    }

    fn script_effective_crosshair_visible(&self) -> bool {
        self.fold_script_overrides(true, |overrides| overrides.crosshair_visible)
    }

    fn script_effective_camera_mode(&self) -> Option<CameraMode> {
        self.fold_script_overrides(None, |overrides| overrides.camera_mode.map(Some))
    }

    pub(super) fn script_effective_hurt_cam(&self) -> bool {
        self.fold_script_overrides(true, |overrides| overrides.hurt_cam)
    }

    pub(super) fn script_effective_fov_change(&self) -> bool {
        self.fold_script_overrides(true, |overrides| overrides.fov_change)
    }

    fn fold_script_overrides<T: Clone>(
        &self,
        mut value: T,
        select: impl Fn(&ScriptVisualOverrides) -> Option<T>,
    ) -> T {
        for info in self.scripts.loaded_mods() {
            if !info.enabled {
                continue;
            }
            if let Some(overrides) = self.script_visual_overrides.get(&info.id) {
                if let Some(next) = select(overrides) {
                    value = next;
                }
            }
        }
        value
    }

    fn script_overrides_mut(&mut self, mod_id: &str) -> &mut ScriptVisualOverrides {
        self.script_visual_overrides
            .entry(mod_id.to_owned())
            .or_default()
    }

    fn has_script_player(&self) -> bool {
        self.net_ctrl.connection.is_some() && self.session.entity_id.is_some()
    }

    fn script_client_snapshot(&self, frame_delta_seconds: f32) -> ClientSnapshot {
        let window = self.script_window_snapshot();
        ClientSnapshot {
            tick: self.tick_timer.tick_count(),
            frame_delta_seconds: finite_delta(frame_delta_seconds),
            fps: self.ui.fps as f32,
            active_screen: self.script_screen_id().to_owned(),
            paused: self.state.has_world_background() && !matches!(self.state, GameState::Playing),
            window: window.clone(),
            settings: ClientSettingsSnapshot {
                fov_degrees: self.script_effective_fov(),
                gui_scale: self.config.gui_scale as f32,
                view_bobbing: self.script_effective_view_bobbing(),
                hud_visible: !self.hud_hidden && self.script_effective_hud_visible(),
                camera_mode: camera_mode(self.player.camera_mode),
            },
            connection: self.script_connection_snapshot(),
            player: self.script_player_snapshot(),
        }
    }

    fn script_connection_snapshot(&self) -> ConnectionSnapshot {
        let state = if self.net_ctrl.connect_task.is_some() {
            "connecting"
        } else if let Some(connection) = &self.net_ctrl.connection {
            match connection.state {
                crate::net::packet::ProtocolState::Play => "play",
                crate::net::packet::ProtocolState::Login
                | crate::net::packet::ProtocolState::Handshake => "login",
                crate::net::packet::ProtocolState::Status => "connecting",
            }
        } else {
            "disconnected"
        };
        let connected = state != "disconnected";
        ConnectionSnapshot {
            state: state.to_owned(),
            server_address: connected.then(|| self.server_address.clone()),
            protocol_version: connected.then_some(crate::net::protocol::PROTOCOL_VERSION),
            protocol_name: connected.then(|| "Minecraft 1.8.9".to_owned()),
            latency_ms: None,
            encrypted: None,
            server_brand: self.session.server_brand.clone(),
        }
    }

    fn script_player_snapshot(&self) -> Option<PlayerSnapshot> {
        if !self.has_script_player() {
            return None;
        }
        let selected = self.inventory.selected_item();
        let using_item = self.item_use_active;
        let use_action = using_item.then(|| item_use_action(selected.item_id).to_owned());
        let blocking = using_item && super::block_interaction::is_sword(selected.item_id);
        let (swing_progress, swinging) = self.renderer.as_ref().map_or((0.0, false), |renderer| {
            let progress = renderer.state.hud.hand_swing_progress();
            (
                progress,
                renderer.state.hud.hand_swing_timer() > 0.0 || progress > 0.0,
            )
        });
        Some(PlayerSnapshot {
            entity_id: self.session.entity_id,
            name: Some(self.username.clone()),
            gamemode: self.session.gamemode,
            dimension: self.session.dimension,
            position: vec3_point(self.player.position),
            previous_position: vec3_point(self.player.prev_position),
            velocity: vec3_vector(self.player.velocity),
            rotation: PlayerRotationSnapshot {
                yaw: self.player.camera.mc_yaw_degrees(),
                pitch: self.player.camera.mc_pitch_degrees(),
                body_yaw: self.player.body_yaw,
                head_yaw: self.player.camera.mc_yaw_degrees(),
            },
            movement: PlayerMovementSnapshot {
                on_ground: self.player.on_ground,
                collided_horizontally: self.player.collided_horizontally,
                sneaking: self.player.sneaking,
                sprinting: self.player.sprinting,
                jumping: self.player.movement_jump,
                in_water: self.player.in_water,
                in_lava: self.player.in_lava(),
                fall_distance: self.player.fall_distance,
                input_strafe: self.player.move_strafe,
                input_forward: self.player.move_forward,
            },
            action: PlayerActionSnapshot {
                using_item,
                use_action,
                use_ticks: (self.item_use_timer.max(0.0) * 20.0).floor() as u32,
                blocking,
                swinging,
                swing_progress,
            },
            capabilities: PlayerCapabilitiesSnapshot {
                invulnerable: self.session.ability_flags & 0x01 != 0,
                creative_mode: self.session.ability_flags & 0x08 != 0,
                allow_flying: self.player.allow_flying,
                flying: self.player.flying,
                walk_speed: self.session.walking_speed,
                fly_speed: self.session.flying_speed,
            },
            vitals: PlayerVitalsSnapshot {
                health: self.session.health,
                max_health: None,
                absorption: None,
                food: self.session.food,
                saturation: self.session.saturation,
                oxygen: self.player.oxygen,
            },
            experience: PlayerExperienceSnapshot {
                level: self.session.experience_level,
                progress: self.session.experience_bar,
                total: self.session.experience_total,
            },
            selected_hotbar_slot: self.inventory.selected.min(8) as u8,
        })
    }

    fn script_world_snapshot_update(
        &mut self,
    ) -> Option<(
        WorldSnapshot,
        ((i32, i32, i32), crate::world::SnapshotRegionRevision),
        ScriptWorldBlockUpdate,
    )> {
        if !self.has_script_player() {
            return None;
        }
        let px = self.player.position.x.floor() as i32;
        let py = self.player.position.y.floor() as i32;
        let pz = self.player.position.z.floor() as i32;
        let center = (px, py, pz);
        let key = (
            center,
            self.world
                .snapshot_revision_around(center, MAX_BLOCK_QUERY_RADIUS),
        );
        let changes = self.world.take_snapshot_changes();
        let had_tracked_changes =
            changes.all || !changes.chunks.is_empty() || !changes.blocks.is_empty();
        let bounds = script_world_block_bounds(center);
        let changed_blocks = changes
            .blocks
            .into_iter()
            .filter(|&position| script_world_bounds_contain(bounds, position))
            .collect::<HashSet<_>>();
        let local_chunk_dirty = changes
            .chunks
            .iter()
            .copied()
            .any(|chunk| script_world_bounds_intersect_chunk(bounds, chunk));
        let block_update = match self.last_script_world_block_key.as_ref() {
            None => ScriptWorldBlockUpdate::Replace,
            Some(_) if !self.scripts.has_world_snapshot() => ScriptWorldBlockUpdate::Replace,
            Some(_) if changes.all || local_chunk_dirty => ScriptWorldBlockUpdate::Replace,
            Some((previous_center, _)) if *previous_center != center => {
                ScriptWorldBlockUpdate::Partial {
                    previous_center: Some(*previous_center),
                    changed_blocks,
                }
            }
            Some(_) if !changed_blocks.is_empty() => ScriptWorldBlockUpdate::Partial {
                previous_center: None,
                changed_blocks,
            },
            Some((_, previous_revision)) if previous_revision == &key.1 || had_tracked_changes => {
                ScriptWorldBlockUpdate::Reuse
            }
            Some(_) => ScriptWorldBlockUpdate::Replace,
        };
        Some((
            self.script_world_snapshot(center, &block_update),
            key,
            block_update,
        ))
    }

    fn script_world_snapshot(
        &self,
        center: (i32, i32, i32),
        block_update: &ScriptWorldBlockUpdate,
    ) -> WorldSnapshot {
        let blocks = self.script_world_blocks(center, block_update);

        let entities = self
            .entities
            .iter()
            .into_iter()
            .map(|(_id, entity)| {
                let (name, health, max_health) = match &entity.data {
                    EntityData::Player { name, .. } => (Some(name.clone()), None, None),
                    EntityData::Mob { health, max_health }
                    | EntityData::Living {
                        health, max_health, ..
                    } => (None, Some(*health), Some(*max_health)),
                    _ => (None, None, None),
                };
                EntitySnapshot {
                    id: entity.entity_id,
                    kind: entity_api_name(entity.entity_type),
                    name,
                    position: [
                        entity.position.x as f64,
                        entity.position.y as f64,
                        entity.position.z as f64,
                    ],
                    velocity: [
                        entity.velocity.x as f64,
                        entity.velocity.y as f64,
                        entity.velocity.z as f64,
                    ],
                    yaw: entity.yaw,
                    pitch: entity.pitch,
                    on_ground: entity.on_ground,
                    health,
                    max_health,
                }
            })
            .collect();

        WorldSnapshot {
            dimension_id: self.session.dimension as i32,
            dimension_name: dimension_name(self.session.dimension).to_owned(),
            game_time: self.session.world_time,
            day_time: self.session.day_time,
            weather: WeatherSnapshot {
                raining: self.session.game_state.raining,
                thundering: self.session.game_state.thunder_level > 0.0,
                rain_strength: self.session.game_state.rain_level,
                thunder_strength: self.session.game_state.thunder_level,
            },
            loaded_chunks: self.world.chunks.len(),
            player_position: [
                self.player.position.x,
                self.player.position.y,
                self.player.position.z,
            ],
            blocks,
            entities,
        }
    }

    fn script_world_blocks(
        &self,
        center: (i32, i32, i32),
        block_update: &ScriptWorldBlockUpdate,
    ) -> BTreeMap<(i32, i32, i32), BlockSnapshot> {
        if matches!(block_update, ScriptWorldBlockUpdate::Reuse) {
            return BTreeMap::new();
        }

        if let ScriptWorldBlockUpdate::Partial {
            previous_center: None,
            changed_blocks,
        } = block_update
        {
            let mut blocks = BTreeMap::new();
            for &(x, y, z) in changed_blocks {
                if let Some(block) = self.script_block_snapshot_at(x, y, z) {
                    blocks.insert((x, y, z), block);
                }
            }
            return blocks;
        }

        let ((min_x, min_y, min_z), (max_x, max_y, max_z)) = script_world_block_bounds(center);
        let previous_bounds = match block_update {
            ScriptWorldBlockUpdate::Partial {
                previous_center: Some(previous_center),
                ..
            } => Some(script_world_block_bounds(*previous_center)),
            ScriptWorldBlockUpdate::Replace
            | ScriptWorldBlockUpdate::Reuse
            | ScriptWorldBlockUpdate::Partial {
                previous_center: None,
                ..
            } => None,
        };
        let changed_blocks = match block_update {
            ScriptWorldBlockUpdate::Partial { changed_blocks, .. } => Some(changed_blocks),
            _ => None,
        };
        let radius = MAX_BLOCK_QUERY_RADIUS;
        let mut blocks = BTreeMap::new();

        for x in min_x..=max_x {
            let cx = x.div_euclid(16);
            let lx = x.rem_euclid(16) as usize;
            for z in min_z..=max_z {
                let cz = z.div_euclid(16);
                let lz = z.rem_euclid(16) as usize;
                let Some(chunk) = self.world.chunks.get(&(cx, cz)) else {
                    continue;
                };
                let network_light_valid = chunk.has_valid_network_light();
                let light_chunk = self.world.light.chunk(cx, cz);
                let biome = chunk.biome(lx, lz);
                for y in min_y..=max_y {
                    if previous_bounds.is_some_and(|bounds| {
                        script_world_bounds_contain(bounds, (x, y, z))
                            && !changed_blocks.is_some_and(|changed| changed.contains(&(x, y, z)))
                    }) {
                        continue;
                    }
                    let light = script_light_at(
                        chunk,
                        network_light_valid,
                        light_chunk,
                        lx,
                        y as usize,
                        lz,
                    );
                    blocks.insert(
                        (x, y, z),
                        BlockSnapshot {
                            state: chunk.state(lx, y as usize, lz),
                            sky_light: light.sky,
                            block_light: light.block,
                            biome,
                        },
                    );
                }
            }
        }
        debug_assert!(blocks.len() <= ((radius * 2 + 1).pow(3) as usize));
        blocks
    }

    fn script_block_snapshot_at(&self, x: i32, y: i32, z: i32) -> Option<BlockSnapshot> {
        if !(0..crate::world::chunk::CHUNK_HEIGHT as i32).contains(&y) {
            return None;
        }
        let cx = x.div_euclid(16);
        let cz = z.div_euclid(16);
        let lx = x.rem_euclid(16) as usize;
        let lz = z.rem_euclid(16) as usize;
        let chunk = self.world.chunks.get(&(cx, cz))?;
        let light = script_light_at(
            chunk,
            chunk.has_valid_network_light(),
            self.world.light.chunk(cx, cz),
            lx,
            y as usize,
            lz,
        );
        Some(BlockSnapshot {
            state: chunk.state(lx, y as usize, lz),
            sky_light: light.sky,
            block_light: light.block,
            biome: chunk.biome(lx, lz),
        })
    }

    fn script_ui_snapshot(&self) -> UiSnapshot {
        let window = self.script_window_snapshot();
        let screen_id = self.script_screen_id().to_owned();
        let in_game = self.state.has_world_background();
        let paused = in_game && !matches!(self.state, GameState::Playing);
        let inventory = self.script_inventory_snapshot();
        let debug_visible = self
            .renderer
            .as_ref()
            .is_some_and(|renderer| renderer.state.settings.debug_overlay());
        UiSnapshot {
            screen: UiScreenSnapshot {
                id: screen_id,
                title: Some(super::WINDOW_TITLE.to_string()),
                in_game,
                paused,
            },
            chat: UiChatSnapshot {
                open: self.chat_open,
                input: self.chat_input.clone(),
                visible_messages: self.session.chat_lines.len().min(if self.chat_open {
                    15
                } else {
                    6
                }) as u32,
                unread_messages: 0,
            },
            inventory,
            gui: UiGuiSnapshot {
                hud_visible: !self.hud_hidden && self.script_effective_hud_visible(),
                crosshair_visible: !self.hud_hidden && self.script_effective_crosshair_visible(),
                chat_visible: (!self.hud_hidden && self.script_effective_hud_visible())
                    || self.chat_open,
                debug_visible,
                scale: self.config.gui_scale as f32,
                focused_widget: self.script_focused_widget().map(str::to_owned),
            },
            window: UiWindowSnapshot {
                width: window.width,
                height: window.height,
                framebuffer_width: window.framebuffer_width,
                framebuffer_height: window.framebuffer_height,
                scale_factor: window.scale_factor,
                focused: window.focused,
                fullscreen: window.fullscreen,
            },
        }
    }

    fn script_inventory_snapshot(&self) -> UiInventorySnapshot {
        if !self.inventory_open {
            return UiInventorySnapshot {
                selected_hotbar_slot: self.inventory.selected.min(8) as u8,
                ..UiInventorySnapshot::default()
            };
        }

        let mut slots = Vec::new();
        if self.inventory.open_window_id == 0 {
            for (slot, stack) in self.inventory.slots.iter().enumerate() {
                if let Some(item) = ui_item_snapshot(slot as i32, stack) {
                    slots.push(item);
                }
            }
        } else {
            for (slot, stack) in self.inventory.open_window_slots.iter().enumerate() {
                if let Some(item) = ui_item_snapshot(slot as i32, stack) {
                    slots.push(item);
                }
            }
        }
        UiInventorySnapshot {
            open: true,
            window_id: Some(self.inventory.open_window_id as i32),
            kind: Some(if self.inventory.open_window_id == 0 {
                "minecraft:inventory".to_owned()
            } else {
                self.inventory.open_window_type.clone()
            }),
            title: Some(self.inventory.open_window_title.clone()),
            slot_count: if self.inventory.open_window_id == 0 {
                self.inventory.slots.len() as u32
            } else {
                self.inventory.open_window_slot_count as u32
            },
            selected_hotbar_slot: self.inventory.selected.min(8) as u8,
            cursor_item: ui_item_snapshot(-1, &self.inventory.cursor),
            slots,
        }
    }

    fn script_window_snapshot(&self) -> WindowSnapshot {
        let Some(window) = &self.window else {
            return WindowSnapshot::default();
        };
        let framebuffer = window.inner_size();
        let scale_factor = window.scale_factor();
        let logical = framebuffer.to_logical::<f64>(scale_factor);
        WindowSnapshot {
            width: logical.width.max(0.0).round() as u32,
            height: logical.height.max(0.0).round() as u32,
            framebuffer_width: framebuffer.width,
            framebuffer_height: framebuffer.height,
            scale_factor,
            focused: window.has_focus(),
            fullscreen: window.fullscreen().is_some(),
        }
    }

    fn script_screen_id(&self) -> &'static str {
        if self.session.sign_editor.is_some() {
            return "sign_editor";
        }
        if self.chat_open {
            return "chat";
        }
        if self.inventory_open {
            return "inventory";
        }
        match self.state {
            GameState::MainMenu => "main_menu",
            GameState::AltManager => "alt_manager",
            GameState::Multiplayer => "multiplayer",
            GameState::DirectConnect => "direct_connect",
            GameState::ServerEditor { .. } => "server_editor",
            GameState::Connecting => "connecting",
            GameState::LoadingWorld => "loading_world",
            GameState::Playing => "game",
            GameState::Paused => "pause",
            GameState::Disconnected { .. } => "disconnected",
            GameState::Options { .. } => "options",
            GameState::VideoSettings { .. } => "video_settings",
            GameState::Controls { .. } => "controls",
            GameState::SkinCustomization { .. } => "skin_customization",
            GameState::Language { .. } => "language",
            GameState::AudioSettings { .. } => "audio_settings",
            GameState::ChatSettings { .. } => "chat_settings",
            GameState::ResourcePacks { .. } => "resource_packs",
            GameState::ShaderPacks { .. } => "shader_packs",
            GameState::Modding { .. } => "modding",
            GameState::ModConfig { .. } => "mod_config",
        }
    }

    fn script_focused_widget(&self) -> Option<&'static str> {
        if self.chat_open {
            Some("chat_input")
        } else if self.session.sign_editor.is_some() {
            Some("sign_editor")
        } else if matches!(self.state, GameState::DirectConnect) {
            Some("server_address")
        } else if matches!(self.state, GameState::ServerEditor { .. })
            && self.server_editor_address_focused
        {
            Some("server_address")
        } else {
            None
        }
    }
}

fn ui_item_snapshot(
    slot: i32,
    stack: &crate::client::inventory::ItemStack,
) -> Option<UiItemSnapshot> {
    (!stack.is_empty()).then(|| UiItemSnapshot {
        slot,
        id: crate::render::first_person::item_resource_id(stack.item_id, stack.damage),
        count: stack.count as u32,
        damage: stack.damage as i32,
        display_name: None,
    })
}

fn vec3_point(value: nalgebra::Point3<f64>) -> Vec3Snapshot {
    Vec3Snapshot {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn vec3_vector(value: nalgebra::Vector3<f64>) -> Vec3Snapshot {
    Vec3Snapshot {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn camera_mode(value: u8) -> CameraMode {
    match value {
        1 => CameraMode::ThirdPersonBack,
        2 => CameraMode::ThirdPersonFront,
        _ => CameraMode::FirstPerson,
    }
}

fn camera_mode_number(value: CameraMode) -> u8 {
    match value {
        CameraMode::FirstPerson => 0,
        CameraMode::ThirdPersonBack => 1,
        CameraMode::ThirdPersonFront => 2,
    }
}

fn item_use_action(item_id: u16) -> &'static str {
    if super::block_interaction::is_sword(item_id) {
        "block"
    } else if super::block_interaction::is_potion(item_id) {
        "drink"
    } else if item_id == 261 {
        "bow"
    } else if super::block_interaction::is_food(item_id) {
        "eat"
    } else {
        "use"
    }
}

type ScriptWorldBlockBounds = ((i32, i32, i32), (i32, i32, i32));

fn script_world_block_bounds(center: (i32, i32, i32)) -> ScriptWorldBlockBounds {
    let radius = MAX_BLOCK_QUERY_RADIUS;
    (
        (
            center.0.saturating_sub(radius),
            center.1.saturating_sub(radius).max(0),
            center.2.saturating_sub(radius),
        ),
        (
            center.0.saturating_add(radius),
            center.1.saturating_add(radius).min(255),
            center.2.saturating_add(radius),
        ),
    )
}

fn script_world_bounds_contain(bounds: ScriptWorldBlockBounds, position: (i32, i32, i32)) -> bool {
    let (min, max) = bounds;
    position.0 >= min.0
        && position.0 <= max.0
        && position.1 >= min.1
        && position.1 <= max.1
        && position.2 >= min.2
        && position.2 <= max.2
}

fn script_world_bounds_intersect_chunk(bounds: ScriptWorldBlockBounds, chunk: (i32, i32)) -> bool {
    let (min, max) = bounds;
    let chunk_min_x = i64::from(chunk.0) * 16;
    let chunk_min_z = i64::from(chunk.1) * 16;
    let chunk_max_x = chunk_min_x + 15;
    let chunk_max_z = chunk_min_z + 15;
    chunk_max_x >= i64::from(min.0)
        && chunk_min_x <= i64::from(max.0)
        && chunk_max_z >= i64::from(min.2)
        && chunk_min_z <= i64::from(max.2)
}

fn dimension_name(dimension: i8) -> &'static str {
    match dimension {
        -1 => "the_nether",
        0 => "overworld",
        1 => "the_end",
        _ => "unknown",
    }
}

fn entity_api_name(entity_type: crate::entity::EntityType) -> String {
    let debug = format!("{entity_type:?}");
    let mut result = String::with_capacity(debug.len() + 4);
    let characters: Vec<_> = debug.chars().collect();
    for (index, &character) in characters.iter().enumerate() {
        let previous_is_lowercase = index > 0 && characters[index - 1].is_ascii_lowercase();
        let acronym_ends = index > 0
            && characters[index - 1].is_ascii_uppercase()
            && characters
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
        if character.is_ascii_uppercase() && (previous_is_lowercase || acronym_ends) {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}

fn finite_delta(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 0.5)
    } else {
        0.0
    }
}

fn script_light_at(
    chunk: &crate::world::chunk::Chunk,
    network_light_valid: bool,
    local_light: Option<&crate::world::light::ChunkLight>,
    x: usize,
    y: usize,
    z: usize,
) -> crate::world::light::LightLevel {
    if network_light_valid {
        let (sky, block) = chunk.light_at(x, y, z);
        crate::world::light::LightLevel { sky, block }
    } else {
        local_light.map_or(
            crate::world::light::LightLevel { sky: 0, block: 0 },
            |light| light.get(x, y, z),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_entity_names_are_snake_case() {
        assert_eq!(
            entity_api_name(crate::entity::EntityType::MinecartTNT),
            "minecart_tnt"
        );
        assert_eq!(entity_api_name(crate::entity::EntityType::XPOrb), "xp_orb");
        assert_eq!(
            entity_api_name(crate::entity::EntityType::CaveSpider),
            "cave_spider"
        );
    }

    #[test]
    fn camera_mode_round_trips() {
        for value in 0..=2 {
            assert_eq!(camera_mode_number(camera_mode(value)), value);
        }
    }

    #[test]
    fn script_world_bounds_clamp_to_vanilla_height() {
        assert_eq!(
            script_world_block_bounds((0, 0, 0)),
            ((-15, 0, -15), (15, 15, 15))
        );
        assert_eq!(
            script_world_block_bounds((0, 255, 0)),
            ((-15, 240, -15), (15, 255, 15))
        );
    }

    #[test]
    fn one_block_shift_only_adds_one_snapshot_face() {
        let previous = script_world_block_bounds((0, 64, 0));
        let next = script_world_block_bounds((1, 64, 0));
        let mut entering = 0;
        for x in next.0 .0..=next.1 .0 {
            for y in next.0 .1..=next.1 .1 {
                for z in next.0 .2..=next.1 .2 {
                    entering += usize::from(!script_world_bounds_contain(previous, (x, y, z)));
                }
            }
        }
        assert_eq!(entering, 31 * 31);
    }

    #[test]
    fn script_snapshot_prefers_fresh_server_light_nibbles() {
        let mut chunk = crate::world::chunk::Chunk::new(0, 0);
        chunk.set_sky_light(1, 64, 1, 7);
        chunk.set_block_light(1, 64, 1, 11);
        chunk.finish_network_light(true, true);
        let stale_local = crate::world::light::ChunkLight::new();

        let light = script_light_at(&chunk, true, Some(&stale_local), 1, 64, 1);

        assert_eq!(light.sky, 7);
        assert_eq!(light.block, 11);
    }
}
