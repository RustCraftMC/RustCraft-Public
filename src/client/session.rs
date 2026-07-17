use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct SessionText {
    pub inventory_rejected: String,
    pub resource_pack_offered: String,
    pub opened_window: String,
}

impl Default for SessionText {
    fn default() -> Self {
        Self {
            inventory_rejected: "Inventory action rejected; waiting for server resync".to_string(),
            resource_pack_offered: "Server offered a resource pack".to_string(),
            opened_window: "Opened window %1$s (%2$s slots)".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionState {
    pub entity_id: Option<i32>,
    pub gamemode: u8,
    pub dimension: i8,
    pub difficulty: u8,
    pub max_players: u8,
    pub level_type: String,
    pub reduced_debug: bool,
    pub spawn_position: Option<[i32; 3]>,
    pub world_time: i64,
    pub day_time: i64,
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
    pub experience_bar: f32,
    pub experience_level: i32,
    pub experience_total: i32,
    pub chat_lines: Vec<String>,
    /// Original JSON components, retained so translated messages follow a locale change.
    pub chat_json: Vec<Option<String>>,
    /// UUIDs inferred from normal player chat messages; system messages have no sender.
    pub chat_senders: Vec<Option<String>>,
    pub last_disconnect_reason: Option<String>,
    pub ability_flags: u8,
    pub flying_speed: f32,
    pub walking_speed: f32,
    pub locale: String,
    pub view_distance: u8,
    pub skin_parts: u8,
    pub player_list: HashMap<String, PlayerListState>,
    pub player_list_generation: u64,
    pub player_profile_generation: u64,
    pub player_skin_generation: u64,
    pub title: TitleState,
    pub action_bar: Option<String>,
    pub action_bar_timer: i32,
    pub tab_header: Option<String>,
    pub tab_footer: Option<String>,
    pub scoreboard: ScoreboardState,
    pub scoreboard_generation: u64,
    pub world_border: WorldBorderState,
    pub statistics: HashMap<String, i32>,
    pub camera_entity_id: Option<i32>,
    pub last_bed_use: Option<(i32, [i32; 3])>,
    pub game_state: GameStateFlags,
    pub server_brand: Option<String>,
    pub resource_pack: Option<ResourcePackOffer>,
    pub tab_complete_matches: Vec<String>,
    pub sign_editor: Option<SignEditorState>,
    /// Stored sign text data, keyed by block position (x, y, z).
    pub sign_data: HashMap<(i32, i32, i32), [String; 4]>,
    pub text: SessionText,
    pub gamemode_dirty: bool,
    /// Set to true when the server sends the first PlayerPositionAndLook packet.
    /// Used to know when to transition from LoadingWorld to Playing.
    pub received_initial_position: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerListState {
    pub uuid: String,
    pub name: String,
    pub display_name: Option<String>,
    pub gamemode: i32,
    pub ping: i32,
    pub skin_property: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TitleState {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub fade_in: i32,
    pub stay: i32,
    pub fade_out: i32,
    pub age: i32,
    pub visible: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ScoreboardState {
    pub objectives: HashMap<String, ScoreboardObjectiveState>,
    pub display_slots: HashMap<i8, String>,
    pub scores: HashMap<(String, String), i32>,
    pub teams: HashMap<String, TeamState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreboardObjectiveState {
    pub name: String,
    pub display_name: String,
    pub render_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamState {
    pub name: String,
    pub display_name: String,
    pub prefix: String,
    pub suffix: String,
    pub friendly_flags: i8,
    pub name_tag_visibility: String,
    pub color: i8,
    pub players: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct WorldBorderState {
    pub center_x: f64,
    pub center_z: f64,
    pub old_diameter: f64,
    pub diameter: f64,
    pub lerp_speed_ms: i64,
    pub portal_teleport_boundary: i32,
    pub warning_time: i32,
    pub warning_blocks: i32,
}

#[derive(Clone, Debug)]
pub struct SidebarLine {
    pub name: String,
    pub display: String,
    pub score: i32,
}

#[derive(Clone, Debug)]
pub struct GameStateFlags {
    pub invalid_bed: bool,
    pub raining: bool,
    pub rain_level: f32,
    pub thunder_level: f32,
    pub demo_message: Option<f32>,
    pub credits: bool,
}

#[derive(Clone, Debug)]
pub struct ResourcePackOffer {
    pub url: String,
    pub hash: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct SignEditorState {
    pub pos: (i32, i32, i32),
    pub lines: [String; 4],
    pub active_line: usize,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            entity_id: None,
            gamemode: 0,
            dimension: 0,
            difficulty: 0,
            max_players: 0,
            level_type: "default".to_string(),
            reduced_debug: false,
            spawn_position: None,
            world_time: 0,
            day_time: 0,
            health: 20.0,
            food: 20,
            saturation: 5.0,
            experience_bar: 0.0,
            experience_level: 0,
            experience_total: 0,
            chat_lines: Vec::new(),
            chat_json: Vec::new(),
            chat_senders: Vec::new(),
            last_disconnect_reason: None,
            ability_flags: 0,
            flying_speed: 0.05,
            walking_speed: 0.1,
            locale: "zh_CN".to_string(),
            view_distance: 8,
            skin_parts: 0x7f,
            player_list: HashMap::new(),
            player_list_generation: 0,
            player_profile_generation: 0,
            player_skin_generation: 0,
            title: TitleState::new(),
            action_bar: None,
            action_bar_timer: 0,
            tab_header: None,
            tab_footer: None,
            scoreboard: ScoreboardState::default(),
            scoreboard_generation: 0,
            world_border: WorldBorderState::new(),
            statistics: HashMap::new(),
            camera_entity_id: None,
            last_bed_use: None,
            game_state: GameStateFlags::new(),
            server_brand: None,
            resource_pack: None,
            tab_complete_matches: Vec::new(),
            sign_editor: None,
            sign_data: HashMap::new(),
            text: SessionText::default(),
            gamemode_dirty: false,
            received_initial_position: false,
        }
    }

    pub fn push_chat_json(&mut self, json: &str, position: i8) {
        let text = extract_chat_text(json).unwrap_or_else(|| json.to_string());
        let log_text = crate::logging::event_text(&text);
        match position {
            0 => log::info!(target: "rustcraft::chat", "incoming chat: {log_text}"),
            1 => log::info!(target: "rustcraft::chat", "incoming system chat: {log_text}"),
            2 => {
                self.action_bar = Some(text);
                self.action_bar_timer = 60;
                return;
            }
            other => log::debug!(
                target: "rustcraft::chat",
                "incoming chat (position={other}): {log_text}"
            ),
        }
        self.chat_senders.push(self.chat_sender_uuid(&text));
        self.chat_lines.push(text);
        self.chat_json.push(Some(json.to_string()));
        if self.chat_lines.len() > 100 {
            let drain = self.chat_lines.len() - 100;
            self.chat_lines.drain(0..drain);
            self.chat_senders.drain(0..drain);
            self.chat_json.drain(0..drain);
        }
    }

    pub fn push_system_line(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::debug!(
            target: "rustcraft::chat",
            "client system message: {}",
            crate::logging::event_text(&text)
        );
        self.chat_lines.push(text);
        self.chat_senders.push(None);
        self.chat_json.push(None);
        if self.chat_lines.len() > 100 {
            let drain = self.chat_lines.len() - 100;
            self.chat_lines.drain(0..drain);
            self.chat_senders.drain(0..drain);
            self.chat_json.drain(0..drain);
        }
    }

    fn chat_sender_uuid(&self, text: &str) -> Option<String> {
        let text = strip_format_codes(text);
        self.player_list.iter().find_map(|(uuid, player)| {
            let display = self.player_display_name(player);
            let display = strip_format_codes(&display);
            let matches_sender = [display.as_str(), player.name.as_str()]
                .into_iter()
                .any(|name| {
                    text.strip_prefix(name)
                        .and_then(|rest| rest.chars().next())
                        .is_some_and(|separator| {
                            matches!(separator, ':' | '\u{ff1a}' | '>' | '\u{bb}')
                        })
                });
            matches_sender.then(|| uuid.clone())
        })
    }

    pub fn player_display_name(&self, player: &PlayerListState) -> String {
        player
            .display_name
            .clone()
            .unwrap_or_else(|| self.decorate_player_name(&player.name))
    }

    pub fn decorate_player_name(&self, name: &str) -> String {
        self.scoreboard
            .teams
            .values()
            .find(|team| team.players.contains(name))
            .map(|team| format!("{}{}{}", team.prefix, name, team.suffix))
            .unwrap_or_else(|| name.to_string())
    }

    pub fn apply_player_list_item(
        &mut self,
        action: crate::net::packet::PlayerListAction,
        players: Vec<crate::net::packet::PlayerListEntry>,
    ) {
        use crate::net::packet::PlayerListAction;
        let mut changed = false;
        let mut profile_changed = false;
        let mut skin_changed = false;
        for player in players {
            match action {
                PlayerListAction::AddPlayer => {
                    let skin_property = player
                        .properties
                        .iter()
                        .find(|property| property.name == "textures")
                        .map(|property| property.value.clone());
                    let display_name = player
                        .display_name_json
                        .as_deref()
                        .and_then(extract_chat_text);
                    let state = PlayerListState {
                        uuid: player.uuid.clone(),
                        name: player.name.unwrap_or_else(|| "Player".to_string()),
                        display_name,
                        gamemode: player.gamemode.unwrap_or(0),
                        ping: player.ping.unwrap_or(0),
                        skin_property,
                    };
                    profile_changed |= self.player_list.get(&player.uuid).map_or(true, |current| {
                        current.uuid != state.uuid
                            || current.name != state.name
                            || current.display_name != state.display_name
                            || current.skin_property != state.skin_property
                    });
                    skin_changed |= self.player_list.get(&player.uuid).map_or(true, |current| {
                        current.uuid != state.uuid
                            || current.name != state.name
                            || current.skin_property != state.skin_property
                    });
                    changed |= self.player_list.get(&player.uuid) != Some(&state);
                    self.player_list.insert(player.uuid, state);
                }
                PlayerListAction::UpdateGamemode => {
                    if let Some(state) = self.player_list.get_mut(&player.uuid) {
                        if let Some(gamemode) = player.gamemode {
                            if state.gamemode != gamemode {
                                state.gamemode = gamemode;
                                changed = true;
                            }
                        }
                    }
                }
                PlayerListAction::UpdateLatency => {
                    if let Some(state) = self.player_list.get_mut(&player.uuid) {
                        if let Some(ping) = player.ping {
                            if state.ping != ping {
                                state.ping = ping;
                                changed = true;
                            }
                        }
                    }
                }
                PlayerListAction::UpdateDisplayName => {
                    if let Some(state) = self.player_list.get_mut(&player.uuid) {
                        let display_name = player
                            .display_name_json
                            .as_deref()
                            .and_then(extract_chat_text);
                        if state.display_name != display_name {
                            state.display_name = display_name;
                            changed = true;
                            profile_changed = true;
                        }
                    }
                }
                PlayerListAction::RemovePlayer => {
                    let removed = self.player_list.remove(&player.uuid).is_some();
                    changed |= removed;
                    profile_changed |= removed;
                }
            }
        }
        if changed {
            self.player_list_generation = self.player_list_generation.wrapping_add(1);
        }
        if profile_changed {
            self.player_profile_generation = self.player_profile_generation.wrapping_add(1);
        }
        if skin_changed {
            self.player_skin_generation = self.player_skin_generation.wrapping_add(1);
        }
    }

    pub fn tick_title(&mut self, ticks: i32) {
        self.title.tick(ticks);
    }

    pub fn tick_action_bar(&mut self, ticks: i32) {
        self.action_bar_timer = (self.action_bar_timer - ticks).max(0);
        if self.action_bar_timer == 0 {
            self.action_bar = None;
        }
    }

    pub fn apply_title(
        &mut self,
        action: i32,
        text_json: Option<String>,
        fade_in: Option<i32>,
        stay: Option<i32>,
        fade_out: Option<i32>,
    ) {
        match action {
            0 => {
                self.title.title = text_json
                    .as_deref()
                    .and_then(plain_text)
                    .or_else(|| text_json.clone());
                if let Some(title) = self.title.title.as_deref() {
                    log::info!(
                        target: "rustcraft::chat",
                        "incoming title: {}",
                        crate::logging::event_text(title)
                    );
                }
                self.title.restart();
            }
            1 => {
                self.title.subtitle = text_json
                    .as_deref()
                    .and_then(plain_text)
                    .or_else(|| text_json.clone());
                if let Some(subtitle) = self.title.subtitle.as_deref() {
                    log::info!(
                        target: "rustcraft::chat",
                        "incoming subtitle: {}",
                        crate::logging::event_text(subtitle)
                    );
                }
                self.title.restart();
            }
            2 => {
                if let Some(fade_in) = fade_in {
                    self.title.fade_in = fade_in.max(0);
                }
                if let Some(stay) = stay {
                    self.title.stay = stay.max(0);
                }
                if let Some(fade_out) = fade_out {
                    self.title.fade_out = fade_out.max(0);
                }
            }
            3 => self.title.visible = false,
            4 => self.title = TitleState::new(),
            _ => {}
        }
    }

    pub fn set_tab_header_footer(&mut self, header_json: String, footer_json: String) {
        self.tab_header = plain_text(&header_json)
            .filter(|text| !text.is_empty())
            .or_else(|| (!header_json.is_empty()).then_some(header_json));
        self.tab_footer = plain_text(&footer_json)
            .filter(|text| !text.is_empty())
            .or_else(|| (!footer_json.is_empty()).then_some(footer_json));
    }

    pub fn apply_scoreboard_objective(
        &mut self,
        name: String,
        mode: i8,
        value: Option<String>,
        render_type: Option<String>,
    ) {
        let mut changed = false;
        match mode {
            0 | 2 => {
                let display_name = value
                    .as_deref()
                    .and_then(plain_text)
                    .filter(|text| !text.is_empty())
                    .or(value)
                    .unwrap_or_else(|| name.clone());
                let objective = ScoreboardObjectiveState {
                    name: name.clone(),
                    display_name,
                    render_type: render_type.unwrap_or_else(|| "integer".to_string()),
                };
                changed |= self.scoreboard.objectives.get(&name) != Some(&objective);
                self.scoreboard.objectives.insert(name, objective);
            }
            1 => {
                changed |= self.scoreboard.objectives.remove(&name).is_some();
                let display_slot_count = self.scoreboard.display_slots.len();
                self.scoreboard
                    .display_slots
                    .retain(|_, slot_name| slot_name != &name);
                changed |= self.scoreboard.display_slots.len() != display_slot_count;
                let score_count = self.scoreboard.scores.len();
                self.scoreboard
                    .scores
                    .retain(|(_, objective), _| objective != &name);
                changed |= self.scoreboard.scores.len() != score_count;
            }
            _ => {}
        }
        if changed {
            self.scoreboard_generation = self.scoreboard_generation.wrapping_add(1);
        }
    }

    pub fn apply_display_scoreboard(&mut self, position: i8, score_name: String) {
        let changed = if score_name.is_empty() {
            self.scoreboard.display_slots.remove(&position).is_some()
        } else {
            match self.scoreboard.display_slots.get(&position) {
                Some(current) if current == &score_name => false,
                _ => {
                    self.scoreboard.display_slots.insert(position, score_name);
                    true
                }
            }
        };
        if changed {
            self.scoreboard_generation = self.scoreboard_generation.wrapping_add(1);
        }
    }

    pub fn apply_update_score(
        &mut self,
        item_name: String,
        action: i8,
        score_name: String,
        value: Option<i32>,
    ) {
        let changed = if action == 1 {
            // S3C action REMOVE with an empty objective removes the player's
            // score from every objective in vanilla Scoreboard.
            if score_name.is_empty() {
                let score_count = self.scoreboard.scores.len();
                self.scoreboard
                    .scores
                    .retain(|(name, _), _| name != &item_name);
                self.scoreboard.scores.len() != score_count
            } else {
                self.scoreboard
                    .scores
                    .remove(&(item_name, score_name))
                    .is_some()
            }
        } else if let Some(value) = value {
            let key = (item_name, score_name);
            match self.scoreboard.scores.get(&key) {
                Some(current) if *current == value => false,
                _ => {
                    self.scoreboard.scores.insert(key, value);
                    true
                }
            }
        } else {
            false
        };
        if changed {
            self.scoreboard_generation = self.scoreboard_generation.wrapping_add(1);
        }
    }

    pub fn apply_team(
        &mut self,
        name: String,
        mode: i8,
        display_name: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
        friendly_flags: Option<i8>,
        name_tag_visibility: Option<String>,
        color: Option<i8>,
        players: Vec<String>,
    ) {
        let mut changed = false;
        match mode {
            0 => {
                let team = TeamState {
                    name: name.clone(),
                    display_name: display_name.unwrap_or_default(),
                    prefix: prefix.unwrap_or_default(),
                    suffix: suffix.unwrap_or_default(),
                    friendly_flags: friendly_flags.unwrap_or_default(),
                    name_tag_visibility: name_tag_visibility.unwrap_or_default(),
                    color: color.unwrap_or(-1),
                    players: players.into_iter().collect(),
                };
                changed |= self.scoreboard.teams.get(&name) != Some(&team);
                self.scoreboard.teams.insert(name, team);
            }
            1 => {
                changed |= self.scoreboard.teams.remove(&name).is_some();
            }
            2 => {
                if let Some(team) = self.scoreboard.teams.get_mut(&name) {
                    if let Some(display_name) = display_name {
                        if team.display_name != display_name {
                            team.display_name = display_name;
                            changed = true;
                        }
                    }
                    if let Some(prefix) = prefix {
                        if team.prefix != prefix {
                            team.prefix = prefix;
                            changed = true;
                        }
                    }
                    if let Some(suffix) = suffix {
                        if team.suffix != suffix {
                            team.suffix = suffix;
                            changed = true;
                        }
                    }
                    if let Some(friendly_flags) = friendly_flags {
                        if team.friendly_flags != friendly_flags {
                            team.friendly_flags = friendly_flags;
                            changed = true;
                        }
                    }
                    if let Some(name_tag_visibility) = name_tag_visibility {
                        if team.name_tag_visibility != name_tag_visibility {
                            team.name_tag_visibility = name_tag_visibility;
                            changed = true;
                        }
                    }
                    if let Some(color) = color {
                        if team.color != color {
                            team.color = color;
                            changed = true;
                        }
                    }
                }
            }
            3 => {
                if let Some(team) = self.scoreboard.teams.get_mut(&name) {
                    let player_count = team.players.len();
                    team.players.extend(players);
                    changed |= team.players.len() != player_count;
                } else {
                    let mut team = TeamState::empty(name.clone());
                    team.players.extend(players);
                    self.scoreboard.teams.insert(name, team);
                    changed = true;
                }
            }
            4 => {
                if let Some(team) = self.scoreboard.teams.get_mut(&name) {
                    for player in players {
                        changed |= team.players.remove(&player);
                    }
                }
            }
            _ => {}
        }
        if changed {
            self.scoreboard_generation = self.scoreboard_generation.wrapping_add(1);
            // Team membership and prefix/suffix affect the tab list and player nametags.
            self.player_list_generation = self.player_list_generation.wrapping_add(1);
            self.player_profile_generation = self.player_profile_generation.wrapping_add(1);
        }
    }

    pub fn apply_world_border(&mut self, update: crate::net::packet::WorldBorderUpdate) {
        use crate::net::packet::WorldBorderUpdate;
        match update {
            WorldBorderUpdate::SetSize { diameter } => {
                self.world_border.old_diameter = self.world_border.diameter;
                self.world_border.diameter = diameter;
                self.world_border.lerp_speed_ms = 0;
            }
            WorldBorderUpdate::LerpSize {
                old_diameter,
                new_diameter,
                speed_ms,
            } => {
                self.world_border.old_diameter = old_diameter;
                self.world_border.diameter = new_diameter;
                self.world_border.lerp_speed_ms = speed_ms;
            }
            WorldBorderUpdate::SetCenter { x, z } => {
                self.world_border.center_x = x;
                self.world_border.center_z = z;
            }
            WorldBorderUpdate::Initialize {
                x,
                z,
                old_diameter,
                new_diameter,
                speed_ms,
                portal_teleport_boundary,
                warning_time,
                warning_blocks,
            } => {
                self.world_border.center_x = x;
                self.world_border.center_z = z;
                self.world_border.old_diameter = old_diameter;
                self.world_border.diameter = new_diameter;
                self.world_border.lerp_speed_ms = speed_ms;
                self.world_border.portal_teleport_boundary = portal_teleport_boundary;
                self.world_border.warning_time = warning_time;
                self.world_border.warning_blocks = warning_blocks;
            }
            WorldBorderUpdate::SetWarningTime { seconds } => {
                self.world_border.warning_time = seconds;
            }
            WorldBorderUpdate::SetWarningBlocks { blocks } => {
                self.world_border.warning_blocks = blocks;
            }
            WorldBorderUpdate::Unknown => {}
        }
    }

    pub fn apply_statistics(&mut self, entries: Vec<(String, i32)>) {
        for (name, value) in entries {
            self.statistics.insert(name, value);
        }
    }

    pub fn set_gamemode(&mut self, gamemode: u8) {
        if self.gamemode != gamemode {
            log::info!(
                target: "rustcraft::gameplay",
                "gamemode changed: previous={}, current={gamemode}",
                self.gamemode
            );
            self.gamemode = gamemode;
            self.gamemode_dirty = true;
        }
    }

    pub fn apply_game_state(&mut self, reason: u8, value: f32) {
        log::debug!(
            target: "rustcraft::gameplay",
            "game state change: reason={reason}, value={value}"
        );
        match reason {
            0 => self.game_state.invalid_bed = true,
            1 => self.game_state.raining = false,
            2 => self.game_state.raining = true,
            3 => self.set_gamemode(value as u8),
            4 => self.game_state.credits = true,
            7 => self.game_state.rain_level = value,
            8 => self.game_state.thunder_level = value,
            101..=103 => self.game_state.demo_message = Some(value),
            _ => {}
        }
    }

    pub fn apply_plugin_message(&mut self, channel: String, data: Vec<u8>) {
        if channel == "MC|Brand" || channel == "minecraft:brand" {
            let mut buf = crate::net::protocol::PacketBuffer::new(data);
            if let Ok(brand) = buf.read_string() {
                self.server_brand = Some(brand);
            }
        }
    }

    pub fn apply_combat_event(&mut self, event: crate::net::packet::CombatEvent) {
        if let crate::net::packet::CombatEvent::EntityDead { message_json, .. } = event {
            self.push_chat_json(&message_json, 0);
        }
    }

    pub fn apply_tab_complete(&mut self, matches: Vec<String>) {
        self.tab_complete_matches = matches;
    }

    pub fn open_sign_editor(&mut self, x: i32, y: i32, z: i32) {
        self.sign_editor = Some(SignEditorState {
            pos: (x, y, z),
            lines: Default::default(),
            active_line: 0,
        });
    }

    pub fn sidebar_lines(&self) -> (Option<String>, Vec<SidebarLine>) {
        let Some(objective_name) = self.scoreboard.display_slots.get(&1) else {
            return (None, Vec::new());
        };
        let title = self
            .scoreboard
            .objectives
            .get(objective_name)
            .map(|objective| objective.display_name.clone())
            .unwrap_or_else(|| objective_name.clone());
        let mut lines: Vec<_> = self
            .scoreboard
            .scores
            .iter()
            .filter(|((_, objective), _)| objective == objective_name)
            .map(|((item, _), score)| SidebarLine {
                name: item.clone(),
                display: self.decorated_score_name(item),
                score: *score,
            })
            .collect();
        // Vanilla's Score comparator orders scores ascending and uses the
        // player name in reverse case-insensitive order for ties. The HUD
        // draws this list from top to bottom, so keep the equivalent
        // descending order here.
        lines.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| b.name.to_lowercase().cmp(&a.name.to_lowercase()))
        });
        lines.truncate(15);
        (Some(title), lines)
    }

    fn decorated_score_name(&self, name: &str) -> String {
        self.scoreboard
            .teams
            .values()
            .find(|team| team.players.contains(name))
            .map(|team| format!("{}{}{}", team.prefix, name, team.suffix))
            .unwrap_or_else(|| name.to_string())
    }
}

impl TitleState {
    pub fn new() -> Self {
        Self {
            title: None,
            subtitle: None,
            fade_in: 10,
            stay: 70,
            fade_out: 20,
            age: 0,
            visible: false,
        }
    }

    pub fn restart(&mut self) {
        self.age = 0;
        self.visible = true;
    }

    pub fn tick(&mut self, ticks: i32) {
        if !self.visible {
            return;
        }
        self.age = self.age.saturating_add(ticks.max(0));
        if self.age > self.fade_in + self.stay + self.fade_out {
            self.visible = false;
        }
    }

    pub fn alpha(&self) -> f32 {
        if !self.visible {
            return 0.0;
        }
        let age = self.age.max(0);
        if self.fade_in > 0 && age < self.fade_in {
            return age as f32 / self.fade_in as f32;
        }
        let fade_out_start = self.fade_in + self.stay;
        if self.fade_out > 0 && age > fade_out_start {
            return 1.0 - ((age - fade_out_start) as f32 / self.fade_out as f32).clamp(0.0, 1.0);
        }
        1.0
    }
}

impl TeamState {
    fn empty(name: String) -> Self {
        Self {
            name,
            display_name: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            friendly_flags: 0,
            name_tag_visibility: String::new(),
            color: -1,
            players: HashSet::new(),
        }
    }
}

impl WorldBorderState {
    pub fn new() -> Self {
        Self {
            center_x: 0.0,
            center_z: 0.0,
            old_diameter: 60_000_000.0,
            diameter: 60_000_000.0,
            lerp_speed_ms: 0,
            portal_teleport_boundary: 29_999_984,
            warning_time: 15,
            warning_blocks: 5,
        }
    }
}

impl GameStateFlags {
    fn new() -> Self {
        Self {
            invalid_bed: false,
            raining: false,
            rain_level: 0.0,
            thunder_level: 0.0,
            demo_message: None,
            credits: false,
        }
    }
}

fn strip_format_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{00a7}' {
            chars.next();
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn plain_text(json: &str) -> Option<String> {
    extract_chat_text(json)
}

pub fn localized_chat_text(json: &str, i18n: &crate::ui::i18n::I18n) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut out = String::new();
    collect_localized_text(&value, &mut out, &mut ChatFormat::default(), i18n);
    (!out.is_empty()).then_some(out)
}

fn extract_chat_text(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut out = String::new();
    collect_text(&value, &mut out, &mut ChatFormat::default());
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[derive(Clone, Default)]
struct ChatFormat {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
}

impl ChatFormat {
    fn emit_format_codes(&self, prev: &Self, out: &mut String) {
        if self.color != prev.color {
            if let Some(ref c) = self.color {
                if let Some(code) = mc_color_code(c) {
                    out.push('\u{00a7}');
                    out.push(code);
                }
            } else {
                out.push('\u{00a7}');
                out.push('r');
            }
        }
        if self.bold && !prev.bold {
            out.push('\u{00a7}');
            out.push('l');
        }
        if self.italic && !prev.italic {
            out.push('\u{00a7}');
            out.push('o');
        }
        if self.underlined && !prev.underlined {
            out.push('\u{00a7}');
            out.push('n');
        }
        if self.strikethrough && !prev.strikethrough {
            out.push('\u{00a7}');
            out.push('m');
        }
        if self.obfuscated && !prev.obfuscated {
            out.push('\u{00a7}');
            out.push('k');
        }
    }

    fn from_json(obj: &serde_json::Map<String, serde_json::Value>, parent: &Self) -> Self {
        Self {
            color: obj
                .get("color")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| parent.color.clone()),
            bold: obj
                .get("bold")
                .and_then(|v| v.as_bool())
                .unwrap_or(parent.bold),
            italic: obj
                .get("italic")
                .and_then(|v| v.as_bool())
                .unwrap_or(parent.italic),
            underlined: obj
                .get("underlined")
                .and_then(|v| v.as_bool())
                .unwrap_or(parent.underlined),
            strikethrough: obj
                .get("strikethrough")
                .and_then(|v| v.as_bool())
                .unwrap_or(parent.strikethrough),
            obfuscated: obj
                .get("obfuscated")
                .and_then(|v| v.as_bool())
                .unwrap_or(parent.obfuscated),
        }
    }
}

fn mc_color_code(name: &str) -> Option<char> {
    match name.to_lowercase().as_str() {
        "black" => Some('0'),
        "dark_blue" => Some('1'),
        "dark_green" => Some('2'),
        "dark_aqua" => Some('3'),
        "dark_red" => Some('4'),
        "dark_purple" => Some('5'),
        "gold" => Some('6'),
        "gray" => Some('7'),
        "dark_gray" => Some('8'),
        "blue" => Some('9'),
        "green" => Some('a'),
        "aqua" => Some('b'),
        "red" => Some('c'),
        "light_purple" => Some('d'),
        "yellow" => Some('e'),
        "white" => Some('f'),
        _ => None,
    }
}

fn collect_text(value: &serde_json::Value, out: &mut String, fmt: &mut ChatFormat) {
    match value {
        serde_json::Value::String(s) => out.push_str(s),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_text(item, out, fmt);
            }
        }
        serde_json::Value::Object(map) => {
            let new_fmt = ChatFormat::from_json(map, fmt);
            new_fmt.emit_format_codes(fmt, out);
            let saved = fmt.clone();
            *fmt = new_fmt;

            if let Some(translate) = map.get("translate").and_then(|v| v.as_str()) {
                out.push_str(translate);
                if let Some(with) = map.get("with").and_then(|v| v.as_array()) {
                    out.push(' ');
                    for (idx, item) in with.iter().enumerate() {
                        if idx > 0 {
                            out.push(' ');
                        }
                        collect_text(item, out, fmt);
                    }
                }
            }
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
            if let Some(extra) = map.get("extra") {
                collect_text(extra, out, fmt);
            }

            *fmt = saved;
        }
        _ => {}
    }
}

fn collect_localized_text(
    value: &serde_json::Value,
    out: &mut String,
    fmt: &mut ChatFormat,
    i18n: &crate::ui::i18n::I18n,
) {
    match value {
        serde_json::Value::String(text) => out.push_str(text),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_localized_text(item, out, fmt, i18n);
            }
        }
        serde_json::Value::Object(map) => {
            let new_fmt = ChatFormat::from_json(map, fmt);
            new_fmt.emit_format_codes(fmt, out);
            let saved = fmt.clone();
            *fmt = new_fmt;
            if let Some(key) = map.get("translate").and_then(|value| value.as_str()) {
                let args = map
                    .get("with")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .map(|item| {
                                let mut arg = String::new();
                                collect_localized_text(item, &mut arg, fmt, i18n);
                                arg
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
                out.push_str(&i18n.tf(key, &refs));
            }
            if let Some(text) = map.get("text").and_then(|value| value.as_str()) {
                out.push_str(text);
            }
            if let Some(extra) = map.get("extra") {
                collect_localized_text(extra, out, fmt, i18n);
            }
            *fmt = saved;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::SessionState;
    use crate::net::packet::{PlayerListAction, PlayerListEntry};

    fn player_entry(uuid: &str, gamemode: i32, ping: i32) -> PlayerListEntry {
        PlayerListEntry {
            uuid: uuid.to_string(),
            name: Some("Player".to_string()),
            properties: Vec::new(),
            gamemode: Some(gamemode),
            ping: Some(ping),
            display_name_json: None,
        }
    }

    #[test]
    fn player_list_generation_only_advances_for_actual_changes() {
        let mut session = SessionState::new();
        let entry = player_entry("player-uuid", 0, 50);

        session.apply_player_list_item(PlayerListAction::AddPlayer, vec![entry.clone()]);
        assert_eq!(session.player_list_generation, 1);
        assert_eq!(session.player_profile_generation, 1);
        assert_eq!(session.player_skin_generation, 1);

        session.apply_player_list_item(PlayerListAction::AddPlayer, vec![entry.clone()]);
        session.apply_player_list_item(
            PlayerListAction::UpdateLatency,
            vec![player_entry("player-uuid", 0, 50)],
        );
        session.apply_player_list_item(
            PlayerListAction::RemovePlayer,
            vec![player_entry("missing-uuid", 0, 0)],
        );
        assert_eq!(session.player_list_generation, 1);
        assert_eq!(session.player_profile_generation, 1);
        assert_eq!(session.player_skin_generation, 1);

        session.apply_player_list_item(
            PlayerListAction::UpdateLatency,
            vec![player_entry("player-uuid", 0, 75)],
        );
        assert_eq!(session.player_list_generation, 2);
        assert_eq!(session.player_profile_generation, 1);
        assert_eq!(session.player_skin_generation, 1);

        let mut display_entry = player_entry("player-uuid", 0, 75);
        display_entry.display_name_json = Some(r#"{"text":"Shown"}"#.to_string());
        session.apply_player_list_item(PlayerListAction::UpdateDisplayName, vec![display_entry]);
        assert_eq!(session.player_list_generation, 3);
        assert_eq!(session.player_profile_generation, 2);
        assert_eq!(session.player_skin_generation, 1);

        session.apply_player_list_item(PlayerListAction::RemovePlayer, vec![entry]);
        assert_eq!(session.player_list_generation, 4);
        assert_eq!(session.player_profile_generation, 3);
        assert_eq!(session.player_skin_generation, 1);
    }

    #[test]
    fn scoreboard_generation_tracks_each_kind_of_change() {
        let mut session = SessionState::new();

        session.apply_scoreboard_objective(
            "sidebar".to_string(),
            0,
            Some("Sidebar".to_string()),
            Some("integer".to_string()),
        );
        assert_eq!(session.scoreboard_generation, 1);
        session.apply_scoreboard_objective(
            "sidebar".to_string(),
            0,
            Some("Sidebar".to_string()),
            Some("integer".to_string()),
        );
        assert_eq!(session.scoreboard_generation, 1);

        session.apply_display_scoreboard(1, "sidebar".to_string());
        assert_eq!(session.scoreboard_generation, 2);
        session.apply_display_scoreboard(1, "sidebar".to_string());
        assert_eq!(session.scoreboard_generation, 2);

        session.apply_update_score("Player".to_string(), 0, "sidebar".to_string(), Some(10));
        assert_eq!(session.scoreboard_generation, 3);
        session.apply_update_score("Player".to_string(), 0, "sidebar".to_string(), Some(10));
        assert_eq!(session.scoreboard_generation, 3);

        session.apply_team(
            "red".to_string(),
            0,
            Some("Red".to_string()),
            Some("[R] ".to_string()),
            None,
            None,
            None,
            None,
            vec!["Player".to_string()],
        );
        assert_eq!(session.scoreboard_generation, 4);
        session.apply_team(
            "red".to_string(),
            3,
            None,
            None,
            None,
            None,
            None,
            None,
            vec!["Player".to_string()],
        );
        assert_eq!(session.scoreboard_generation, 4);
        session.apply_team(
            "red".to_string(),
            2,
            Some("Red Team".to_string()),
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
        );
        assert_eq!(session.scoreboard_generation, 5);

        session.apply_scoreboard_objective("sidebar".to_string(), 1, None, None);
        assert_eq!(session.scoreboard_generation, 6);
        assert!(session.scoreboard.objectives.is_empty());
        assert!(session.scoreboard.display_slots.is_empty());
        assert!(session.scoreboard.scores.is_empty());
    }

    #[test]
    fn empty_objective_score_removal_clears_all_player_scores() {
        let mut session = SessionState::new();
        session.apply_update_score("player".to_string(), 0, "sidebar".to_string(), Some(5));
        session.apply_update_score("player".to_string(), 0, "list".to_string(), Some(3));
        session.apply_update_score("other".to_string(), 0, "sidebar".to_string(), Some(7));

        session.apply_update_score("player".to_string(), 1, String::new(), None);

        assert!(!session
            .scoreboard
            .scores
            .keys()
            .any(|(name, _)| name == "player"));
        assert_eq!(session.scoreboard.scores.len(), 1);
    }
}
