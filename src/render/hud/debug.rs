use crate::render::gui::widgets::MenuMetrics;
use crate::render::gui::GuiVertexBuilder;
use crate::render::Renderer;

impl Renderer {
    /// Vanilla-style F3 overlay (GuiOverlayDebug layout): left + right columns,
    /// per-line dark bars, normal HUD font size. Branding says RustCraft instead
    /// of Minecraft/Java.
    pub(super) fn draw_debug_panel(
        &mut self,
        metrics: &MenuMetrics,
        font_gui: &mut GuiVertexBuilder,
    ) {
        let gs = metrics.gs;
        let font_sz = metrics.font_sz;
        let line_h = font_sz + 1.0 * gs;
        let left = self.debug_left_lines();
        let right = self.debug_right_lines(metrics);
        self.draw_debug_column(font_gui, &left, 2.0 * gs, line_h, font_sz, false, metrics.sw);
        self.draw_debug_column(font_gui, &right, 2.0 * gs, line_h, font_sz, true, metrics.sw);
    }

    fn draw_debug_column(
        &mut self,
        font_gui: &mut GuiVertexBuilder,
        lines: &[String],
        pad: f32,
        line_h: f32,
        font_sz: f32,
        align_right: bool,
        screen_w: f32,
    ) {
        let bar = [0.0, 0.0, 0.0, 144.0 / 255.0];
        let text_color = [147.0 / 255.0, 147.0 / 255.0, 147.0 / 255.0, 1.0];
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let text_w = self.font.text_width(line, font_sz);
            let y = pad + line_h * i as f32;
            let x = if align_right {
                screen_w - pad - text_w
            } else {
                pad
            };
            font_gui.fill_rect(x - 1.0, y - 1.0, text_w + 2.0, line_h, bar);
            font_gui.draw_text(&mut self.font, x, y, line, font_sz, text_color);
        }
    }

    fn debug_left_lines(&self) -> Vec<String> {
        let version = env!("CARGO_PKG_VERSION");
        let fps = self.state.hud.fps_count();
        let profile = self.state.frame_profile.completed_frame_profile();
        let chunks = profile.chunk_count_loaded;
        let entities = self.state.hud.debug_entity_count();
        let pos = self.state.hud.debug_pos();
        let bx = pos[0].floor() as i32;
        let by = pos[1].floor() as i32;
        let bz = pos[2].floor() as i32;
        let cx = bx >> 4;
        let cy = by >> 4;
        let cz = bz >> 4;
        let lx = bx & 15;
        let ly = by & 15;
        let lz = bz & 15;
        let (facing, towards) = facing_label(self.state.hud.debug_yaw());
        let yaw = wrap_degrees(self.state.hud.debug_yaw());
        let pitch = self.state.hud.debug_pitch().clamp(-90.0, 90.0);
        let dim = match self.state.hud.dimension() {
            -1 => "Nether",
            1 => "The End",
            _ => "Overworld",
        };
        let mut lines = vec![
            format!("RustCraft {version} (Rust)"),
            format!("{fps} fps"),
            format!("C: {chunks} chunks loaded"),
            format!("E: {entities} entities"),
            format!("P: {} particles", self.state.frame_profile.completed_frame_profile().particle_count),
            dim.to_string(),
            String::new(),
            format!("XYZ: {:.3} / {:.5} / {:.3}", pos[0], pos[1], pos[2]),
            format!("Block: {bx} {by} {bz}"),
            format!("Chunk: {lx} {ly} {lz} in {cx} {cy} {cz}"),
            format!("Facing: {facing} ({towards}) ({yaw:.1} / {pitch:.1})"),
            format!("Biome: {}", biome_name(self.state.hud.debug_biome_id())),
            format!(
                "Light: {} ({} sky, {} block)",
                self.state.hud.debug_light_combined(),
                self.state.hud.debug_light_sky(),
                self.state.hud.debug_light_block()
            ),
            format!(
                "Local Difficulty: {} (Day {})",
                self.state.settings.difficulty(),
                self.state.hud.world_time() / 24000
            ),
            format!(
                "Time: {} day {}",
                self.state.hud.day_time(),
                self.state.hud.world_time() / 24000
            ),
            format!(
                "Gamemode: {}",
                gamemode_name(self.state.hud.gamemode()),
            ),
        ];
        if let Some(look) = self.state.hud.debug_looking_at() {
            lines.push(format!(
                "Looking at: {} {} {}",
                look[0], look[1], look[2]
            ));
        }
        if let Some(name) = self.state.hud.debug_looking_block() {
            lines.push(name.clone());
        }
        if let Some(spawn) = self.state.hud.spawn_position() {
            lines.push(format!(
                "Spawn: {}, {}, {}",
                spawn[0], spawn[1], spawn[2]
            ));
        }
        lines
    }

    fn debug_right_lines(&self, metrics: &MenuMetrics) -> Vec<String> {
        let profile = self.state.frame_profile.completed_frame_profile();
        let mem = process_memory_mb();
        let mut lines = vec![
            format!("Rust {}", rustc_version_string()),
            format!(
                "Mem: {:.0}MB used  frame:{:.1}ms",
                mem,
                profile.interval_us as f32 / 1000.0
            ),
            format!(
                "1%: {:.0}  0.1%: {:.0}  p99: {:.1}ms",
                profile.one_percent_low_fps,
                profile.zero_point_one_percent_low_fps,
                profile.p99_interval_us as f32 / 1000.0
            ),
            String::new(),
            format!(
                "Display: {}x{}",
                metrics.sw as u32, metrics.sh as u32
            ),
            "Vulkan".to_string(),
            String::new(),
            format!(
                "Render: {}us  Mesh: {}us  Upload: {}KiB",
                profile.render_us,
                profile.mesh_us,
                profile.chunk_upload_bytes / 1024
            ),
            format!(
                "Entity cache: {}/{}  culled: {}",
                profile.entity_cache_hits,
                profile.entity_cache_misses,
                profile.entity_culled_count
            ),
        ];
        if let Some(brand) = self.state.server_list.server_brand().clone() {
            lines.push(String::new());
            lines.push(format!("Server: {brand}"));
        }
        if let Some(pack) = self.state.server_list.resource_pack_status().clone() {
            lines.push(format!("Pack: {pack}"));
        }
        lines
    }
}

fn facing_label(yaw_deg: f32) -> (&'static str, &'static str) {
    let y = wrap_degrees(yaw_deg);
    let idx = ((y + 180.0) / 90.0).floor() as i32 & 3;
    match idx {
        0 => ("north", "Towards negative Z"),
        1 => ("east", "Towards positive X"),
        2 => ("south", "Towards positive Z"),
        _ => ("west", "Towards negative X"),
    }
}

fn wrap_degrees(mut deg: f32) -> f32 {
    deg = deg.rem_euclid(360.0);
    if deg >= 180.0 {
        deg - 360.0
    } else {
        deg
    }
}

fn gamemode_name(gm: u8) -> &'static str {
    match gm {
        0 => "survival",
        1 => "creative",
        2 => "adventure",
        3 => "spectator",
        _ => "unknown",
    }
}

fn biome_name(id: u8) -> &'static str {
    match id {
        0 => "Ocean",
        1 => "Plains",
        2 => "Desert",
        3 => "Extreme Hills",
        4 => "Forest",
        5 => "Taiga",
        6 => "Swampland",
        7 => "River",
        8 => "Hell",
        9 => "The End",
        10 => "FrozenOcean",
        11 => "FrozenRiver",
        12 => "Ice Plains",
        13 => "Ice Mountains",
        14 => "MushroomIsland",
        15 => "MushroomIslandShore",
        16 => "Beach",
        17 => "DesertHills",
        18 => "ForestHills",
        19 => "TaigaHills",
        20 => "Extreme Hills Edge",
        21 => "Jungle",
        22 => "JungleHills",
        23 => "JungleEdge",
        24 => "Deep Ocean",
        25 => "Stone Beach",
        26 => "Cold Beach",
        27 => "Birch Forest",
        28 => "Birch Forest Hills",
        29 => "Roofed Forest",
        30 => "Cold Taiga",
        31 => "Cold Taiga Hills",
        32 => "Mega Taiga",
        33 => "Mega Taiga Hills",
        34 => "Extreme Hills+",
        35 => "Savanna",
        36 => "Savanna Plateau",
        37 => "Mesa",
        38 => "Mesa Plateau F",
        39 => "Mesa Plateau",
        129 => "Sunflower Plains",
        130 => "Desert M",
        131 => "Extreme Hills M",
        132 => "Flower Forest",
        133 => "Taiga M",
        134 => "Swampland M",
        140 => "Ice Plains Spikes",
        149 => "Jungle M",
        151 => "JungleEdge M",
        155 => "Birch Forest M",
        156 => "Birch Forest Hills M",
        157 => "Roofed Forest M",
        158 => "Cold Taiga M",
        160 => "Mega Spruce Taiga",
        161 => "Redwood Taiga Hills M",
        162 => "Extreme Hills+ M",
        163 => "Savanna M",
        164 => "Savanna Plateau M",
        165 => "Mesa (Bryce)",
        166 => "Mesa Plateau F M",
        167 => "Mesa Plateau M",
        _ => "Unknown",
    }
}

fn rustc_version_string() -> String {
    format!("{}-bit", std::mem::size_of::<usize>() * 8)
}

fn process_memory_mb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    let kb: f64 = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    return kb / 1024.0;
                }
            }
        }
    }
    0.0
}
