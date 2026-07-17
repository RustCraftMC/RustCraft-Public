use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

impl Renderer {
    pub(super) fn draw_debug_panel(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let base_panel_h: f32 = if self.state.player_list.is_empty() {
            240.0
        } else {
            278.0
        };
        let gs = metrics
            .gs
            .min((metrics.sw / 354.0).max(0.01))
            .min((metrics.sh / (base_panel_h + 4.0)).max(0.01));
        let font_sz = 9.0 * gs;
        let pad = 4.0 * gs;
        let panel_h = base_panel_h * gs;
        let profile = self.state.completed_frame_profile;
        let frame_cap = profile.max_framerate.max(30);
        let target_us = 1_000_000u64 / u64::from(frame_cap);
        let over_target_us = profile.interval_us.saturating_sub(target_us);
        let render_accounted_us = profile
            .fence_us
            .saturating_add(profile.acquire_us)
            .saturating_add(profile.mesh_us)
            .saturating_add(profile.gui_us)
            .saturating_add(profile.command_us)
            .saturating_add(profile.submit_us)
            .saturating_add(profile.present_us);
        let render_other_us = profile.render_us.saturating_sub(render_accounted_us);
        let worst = profile.worst;
        let (worst_cause, worst_cause_us) = [
            ("outside", worst.outside_us),
            ("tasks", worst.tasks_us),
            ("network", worst.network_us),
            ("world", worst.world_us),
            ("tick", worst.tick_us),
            ("sync", worst.sync_us),
            ("render", worst.render_us),
            ("other", worst.other_us),
        ]
        .into_iter()
        .max_by_key(|(_, duration)| *duration)
        .unwrap_or(("none", 0));
        font_gui.fill_rect(pad, pad, 350.0 * gs, panel_h, [0.0, 0.0, 0.0, 0.5]);
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 3.0 * gs,
            &format!(
                "fps:{} 1%:{:.0} 0.1%:{:.0} p99:{:.2} p99.9:{:.2} max:{:.2}ms n:{}",
                self.state.fps_count,
                profile.one_percent_low_fps,
                profile.zero_point_one_percent_low_fps,
                profile.p99_interval_us as f32 / 1000.0,
                profile.p99_9_interval_us as f32 / 1000.0,
                profile.max_interval_us as f32 / 1000.0,
                profile.interval_sample_count,
            ),
            font_sz * 0.58,
            [1.0, 1.0, 1.0, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 15.0 * gs,
            &format!(
                "app tasks:{} net:{} world:{} tick:{} sync:{} other:{}",
                profile.tasks_us,
                profile.network_us,
                profile.world_us,
                profile.tick_us,
                profile.sync_us,
                profile.other_us,
            ),
            font_sz * 0.52,
            [0.85, 0.85, 0.65, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 27.0 * gs,
            &format!(
                "render:{} fence:{} acq:{} mesh:{} gui:{} cmd:{} sub:{} pres:{} other:{}",
                profile.render_us,
                profile.fence_us,
                profile.acquire_us,
                profile.mesh_us,
                profile.gui_us,
                profile.command_us,
                profile.submit_us,
                profile.present_us,
                render_other_us,
            ),
            font_sz * 0.46,
            [0.72, 0.86, 0.95, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 39.0 * gs,
            &format!(
                "entity total:{} loop:{} skin:{} extras:{} prune:{}",
                profile.entity_us,
                profile.entity_loop_us,
                profile.entity_skin_sync_us,
                profile.entity_extras_us,
                profile.entity_prune_us,
            ),
            font_sz * 0.52,
            [0.85, 0.85, 0.85, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 51.0 * gs,
            &format!(
                "entity gen:{} up:{} hash:{} lookup:{} n:{} vis:{} cul:{} cache:{}/{} {}",
                profile.entity_generate_us,
                profile.entity_upload_us,
                profile.entity_hash_us,
                profile.entity_lookup_us,
                profile.entity_count,
                profile.entity_visible_count,
                profile.entity_culled_count,
                profile.entity_cache_hits,
                profile.entity_cache_misses,
                if profile.entity_batch_reused {
                    "reuse"
                } else {
                    "build"
                },
            ),
            font_sz * 0.46,
            [0.85, 0.85, 0.65, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 63.0 * gs,
            &format!(
                "particle:{} {} n:{} local:{} {} nametag:{}",
                profile.particle_us,
                if profile.particle_batch_reused {
                    "reuse"
                } else {
                    "build"
                },
                profile.particle_count,
                profile.local_us,
                if profile.local_batch_reused {
                    "reuse"
                } else {
                    "build"
                },
                profile.nametag_us,
            ),
            font_sz * 0.52,
            [0.72, 0.86, 0.95, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 75.0 * gs,
            &format!(
                "lua:{}us callbacks:{} slow>200us:{}",
                profile.script_us, profile.script_callbacks, profile.script_slow_callbacks,
            ),
            font_sz * 0.52,
            [0.85, 0.72, 0.95, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 87.0 * gs,
            &format!(
                "chunks:{} upload:{} {}KiB {}us",
                profile.chunk_count_loaded,
                profile.chunk_upload_count,
                profile.chunk_upload_bytes / 1024,
                profile.chunk_upload_us,
            ),
            font_sz * 0.52,
            [0.72, 0.86, 0.95, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 99.0 * gs,
            &format!(
                "worst10s:{:.2}ms age:{:.1}s cause:{}:{} active:{} out:{} lua:{}",
                worst.interval_us as f32 / 1000.0,
                profile.worst_age_us as f32 / 1_000_000.0,
                worst_cause,
                worst_cause_us,
                worst.total_us,
                worst.outside_us,
                worst.script_us,
            ),
            font_sz * 0.48,
            [1.0, 0.72, 0.58, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 111.0 * gs,
            &format!(
                "W task:{} net:{} world:{} tick:{} sync:{} render:{} other:{} | R fence:{} acq:{} mesh:{} ent:{} gui:{} cmd:{} sub:{} pres:{} up:{}",
                worst.tasks_us,
                worst.network_us,
                worst.world_us,
                worst.tick_us,
                worst.sync_us,
                worst.render_us,
                worst.other_us,
                worst.fence_us,
                worst.acquire_us,
                worst.mesh_us,
                worst.entity_us,
                worst.gui_us,
                worst.command_us,
                worst.submit_us,
                worst.present_us,
                worst.chunk_upload_us,
            ),
            font_sz * 0.38,
            [0.95, 0.72, 0.62, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 123.0 * gs,
            &format!(
                "E loop:{} hash:{} lookup:{} gen:{} up:{} skin:{} prune:{} extras:{} vis:{}/{}/{}",
                worst.entity_loop_us,
                worst.entity_hash_us,
                worst.entity_lookup_us,
                worst.entity_generate_us,
                worst.entity_upload_us,
                worst.entity_skin_sync_us,
                worst.entity_prune_us,
                worst.entity_extras_us,
                worst.entity_cache_hits,
                worst.entity_cache_misses,
                worst.entity_visible_count + worst.entity_culled_count,
            ),
            font_sz * 0.36,
            [0.95, 0.72, 0.62, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 135.0 * gs,
            &format!(
                "P {}:{}us units:{} hook:{} session:{} inv:{} entity:{} world:{} | sched:{} scan:{} handled:{} deferred:{}",
                worst.network_packet_kind,
                worst.network_packet_us,
                worst.network_packet_units,
                worst.network_hook_us,
                worst.network_session_us,
                worst.network_inventory_us,
                worst.network_entity_us,
                worst.network_world_us,
                worst.network_scheduler_us,
                worst.network_scanned_packets,
                worst.network_handled_packets,
                worst.network_deferred_packets,
            ),
            font_sz * 0.42,
            [0.95, 0.78, 0.55, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 151.0 * gs,
            &format!(
                "World: {} dim {} gm {}",
                self.state.level_type, self.state.dimension, self.state.gamemode
            ),
            font_sz * 0.68,
            [0.75, 0.75, 0.75, 1.0],
        );
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 162.0 * gs,
            &format!("Time: {} / {}", self.state.world_time, self.state.day_time),
            font_sz * 0.68,
            [0.75, 0.75, 0.75, 1.0],
        );
        if let Some(spawn) = self.state.spawn_position {
            font_gui.draw_text(
                &mut self.font,
                pad + 4.0 * gs,
                pad + 173.0 * gs,
                &format!("Spawn: {}, {}, {}", spawn[0], spawn[1], spawn[2]),
                font_sz * 0.68,
                [0.75, 0.75, 0.75, 1.0],
            );
        }
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 185.0 * gs,
            &format!("Sounds played: {}", self.state.sound_event_count),
            font_sz * 0.62,
            [0.70, 0.82, 0.95, 1.0],
        );
        if let Some(sound) = self.state.recent_sounds.first() {
            font_gui.draw_text(
                &mut self.font,
                pad + 4.0 * gs,
                pad + 183.0 * gs,
                sound,
                font_sz * 0.58,
                [0.64, 0.64, 0.64, 1.0],
            );
        }
        font_gui.draw_text(
            &mut self.font,
            pad + 4.0 * gs,
            pad + 193.0 * gs,
            &format!(
                "Border: {:.0} @ {:.0},{:.0}",
                self.state.world_border_diameter,
                self.state.world_border_center[0],
                self.state.world_border_center[1]
            ),
            font_sz * 0.58,
            [0.70, 0.70, 0.70, 1.0],
        );
        if let Some(brand) = self.state.server_brand.clone() {
            font_gui.draw_text(
                &mut self.font,
                pad + 4.0 * gs,
                pad + 203.0 * gs,
                &format!("Server: {}", brand),
                font_sz * 0.58,
                [0.70, 0.82, 0.95, 1.0],
            );
        }
        if let Some(pack) = self.state.resource_pack_status.clone() {
            font_gui.draw_text(
                &mut self.font,
                pad + 4.0 * gs,
                pad + 213.0 * gs,
                &format!("Pack: {}", pack),
                font_sz * 0.54,
                [0.65, 0.65, 0.65, 1.0],
            );
        }
        if !self.state.player_list.is_empty() {
            font_gui.draw_text(
                &mut self.font,
                pad + 4.0 * gs,
                pad + 225.0 * gs,
                &format!(
                    "Online: {}/{}",
                    self.state.player_list.len(),
                    self.state
                        .max_players
                        .max(self.state.player_list.len() as u8)
                ),
                font_sz * 0.68,
                [0.75, 0.9, 1.0, 1.0],
            );
            for (i, (name, ping, _gamemode)) in self.state.player_list.iter().take(3).enumerate() {
                font_gui.draw_text(
                    &mut self.font,
                    pad + 4.0 * gs,
                    pad + (237.0 + i as f32 * 10.0) * gs,
                    &format!("{}  {}ms", name, ping),
                    font_sz * 0.62,
                    [0.72, 0.72, 0.72, 1.0],
                );
            }
        }

        let face_px = 3.0 * gs;
        let face_x = pad + 316.0 * gs;
        let face_y = pad + 6.0 * gs;
        font_gui.fill_rect(
            face_x - 1.0 * gs,
            face_y - 1.0 * gs,
            face_px * 8.0 + 2.0 * gs,
            face_px * 8.0 + 2.0 * gs,
            [0.0, 0.0, 0.0, 0.65],
        );
        font_gui.draw_pixel_face(face_x, face_y, face_px, &self.state.local_skin_face);
    }
}
