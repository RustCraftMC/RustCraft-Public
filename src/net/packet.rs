//! Minecraft 1.8.9 packet definitions and parsing (protocol v47).
//!
//! Clientbound packets: server → client
//! Serverbound packets: client → server

use super::slot::Slot;

pub use super::player_list::{PlayerListAction, PlayerListEntry};

mod clientbound;
mod serverbound;
pub use serverbound::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolState {
    Handshake,
    Status,
    Login,
    Play,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitySpawnKind {
    Player,
    Object,
    Mob,
}

// --- Parsed packets ---

#[derive(Debug)]
pub enum ClientboundPacket {
    // Login state
    EncryptionRequest {
        server_id: String,
        public_key: Vec<u8>,
        verify_token: Vec<u8>,
    },
    LoginSuccess {
        uuid: String,
        username: String,
    },
    SetCompression {
        threshold: i32,
    },
    Disconnect {
        reason: String,
    },
    // Play state
    KeepAlive {
        id: i32,
    },
    JoinGame {
        entity_id: i32,
        gamemode: u8,
        dimension: i8,
        difficulty: u8,
        max_players: u8,
        level_type: String,
        reduced_debug: bool,
    },
    Respawn {
        dimension: i32,
        difficulty: u8,
        gamemode: u8,
        level_type: String,
    },
    ChangeGameMode {
        gamemode: u8,
    },
    ChatMessage {
        json: String,
        position: i8,
    },
    TimeUpdate {
        world_time: i64,
        day_time: i64,
    },
    SpawnPosition {
        x: i32,
        y: i32,
        z: i32,
    },
    UpdateHealth {
        health: f32,
        food: i32,
        saturation: f32,
    },
    SetExperience {
        bar: f32,
        level: i32,
        total: i32,
    },
    PlayerPositionAndLook {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        flags: i8,
    },
    HeldItemChange {
        slot: i8,
    },
    UseBed {
        entity_id: i32,
        x: i32,
        y: i32,
        z: i32,
    },
    Animation {
        entity_id: i32,
        animation: u8,
    },
    Entity {
        entity_id: i32,
    },
    SpawnGlobalEntity {
        entity_id: i32,
        entity_type: i8,
        x: f32,
        y: f32,
        z: f32,
    },
    OpenWindow {
        window_id: u8,
        window_type: String,
        title_json: String,
        slot_count: u8,
        entity_id: Option<i32>,
    },
    CloseWindow {
        window_id: u8,
    },
    SetSlot {
        window_id: i8,
        slot: i16,
        item: Slot,
    },
    WindowItems {
        window_id: u8,
        slots: Vec<Slot>,
    },
    ConfirmTransaction {
        window_id: u8,
        action_number: i16,
        accepted: bool,
    },
    WindowProperty {
        window_id: u8,
        property: i16,
        value: i16,
    },
    UpdateSign {
        x: i32,
        y: i32,
        z: i32,
        lines: [String; 4],
    },
    MapData {
        item_damage: i32,
        scale: i8,
        icons: Vec<MapIcon>,
        columns: u8,
        rows: u8,
        x: u8,
        z: u8,
        data: Vec<u8>,
    },
    UpdateBlockEntity {
        x: i32,
        y: i32,
        z: i32,
        action: u8,
        nbt: Vec<u8>,
    },
    SignEditorOpen {
        x: i32,
        y: i32,
        z: i32,
    },
    EntityEquipment {
        entity_id: i32,
        slot: i16,
        item: Slot,
    },
    PlayerAbilities {
        flags: u8,
        flying_speed: f32,
        walking_speed: f32,
    },
    PlayerListItem {
        action: PlayerListAction,
        players: Vec<PlayerListEntry>,
    },
    EntitySpawn {
        entity_id: i32,
        spawn_kind: EntitySpawnKind,
        entity_type: i32,
        uuid: Option<String>,
        current_item: i16,
        object_data: i32,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        pitch: f32,
        head_yaw: f32,
        velocity: [f32; 3],
        metadata: Vec<super::metadata::EntityMetadata>,
    },
    ExperienceOrbSpawn {
        entity_id: i32,
        x: f32,
        y: f32,
        z: f32,
        count: i16,
    },
    EntityMove {
        entity_id: i32,
        dx: f32,
        dy: f32,
        dz: f32,
        on_ground: bool,
    },
    EntityLook {
        entity_id: i32,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    EntityMoveLook {
        entity_id: i32,
        dx: f32,
        dy: f32,
        dz: f32,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    EntityTeleport {
        entity_id: i32,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    EntityHeadLook {
        entity_id: i32,
        head_yaw: f32,
    },
    EntityVelocity {
        entity_id: i32,
        vx: f64,
        vy: f64,
        vz: f64,
    },
    CollectItem {
        collected_entity_id: i32,
        collector_entity_id: i32,
    },
    EntityStatus {
        entity_id: i32,
        status: i8,
    },
    AttachEntity {
        entity_id: i32,
        vehicle_id: i32,
        leash: bool,
    },
    EntityMetadata {
        entity_id: i32,
        metadata: Vec<super::metadata::EntityMetadata>,
    },
    EntityEffect {
        entity_id: i32,
        effect_id: i8,
        amplifier: i8,
        duration: i32,
        hide_particles: bool,
    },
    RemoveEntityEffect {
        entity_id: i32,
        effect_id: i8,
    },
    DestroyEntities {
        ids: Vec<i32>,
    },
    Particle {
        particle_id: i32,
        long_distance: bool,
        x: f32,
        y: f32,
        z: f32,
        offset_x: f32,
        offset_y: f32,
        offset_z: f32,
        speed: f32,
        count: i32,
        data: Vec<i32>,
    },
    ChunkData {
        chunk_x: i32,
        chunk_z: i32,
        full_chunk: bool,
        primary_bit_mask: u16,
        data: Vec<u8>,
    },
    MapChunkBulk {
        sky_light: bool,
        chunks: Vec<ChunkBulkData>,
    },
    BlockChange {
        x: i32,
        y: i32,
        z: i32,
        block_state: u16,
    },
    MultiBlockChange {
        chunk_x: i32,
        chunk_z: i32,
        records: Vec<(u16, u16)>,
    },
    BlockAction {
        x: i32,
        y: i32,
        z: i32,
        byte1: u8,
        byte2: u8,
        block_type: i32,
    },
    BlockBreakAnimation {
        entity_id: i32,
        x: i32,
        y: i32,
        z: i32,
        destroy_stage: i8,
    },
    Effect {
        effect_id: i32,
        x: i32,
        y: i32,
        z: i32,
        data: i32,
        disable_relative_volume: bool,
    },
    Explosion {
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
        records: Vec<[i8; 3]>,
        player_motion: [f32; 3],
    },
    NamedSoundEffect {
        name: String,
        x: f32,
        y: f32,
        z: f32,
        volume: f32,
        pitch: f32,
    },
    ChangeGameState {
        reason: u8,
        value: f32,
    },
    TabComplete {
        matches: Vec<String>,
    },
    Statistics {
        entries: Vec<(String, i32)>,
    },
    PluginMessage {
        channel: String,
        data: Vec<u8>,
    },
    ServerDifficulty {
        difficulty: u8,
    },
    CombatEvent {
        event: CombatEvent,
    },
    Camera {
        camera_id: i32,
    },
    PlayerListHeaderFooter {
        header_json: String,
        footer_json: String,
    },
    ResourcePackSend {
        url: String,
        hash: String,
    },
    EntityProperties {
        entity_id: i32,
        properties: Vec<EntityProperty>,
    },
    DisplayScoreboard {
        position: i8,
        score_name: String,
    },
    Teams {
        name: String,
        mode: i8,
        display_name: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
        friendly_flags: Option<i8>,
        name_tag_visibility: Option<String>,
        color: Option<i8>,
        players: Vec<String>,
    },
    ScoreboardObjective {
        name: String,
        mode: i8,
        value: Option<String>,
        render_type: Option<String>,
    },
    UpdateScore {
        item_name: String,
        action: i8,
        score_name: String,
        value: Option<i32>,
    },
    Title {
        action: i32,
        text_json: Option<String>,
        fade_in: Option<i32>,
        stay: Option<i32>,
        fade_out: Option<i32>,
    },
    WorldBorder {
        action: i32,
        update: WorldBorderUpdate,
    },
    Unknown {
        id: i32,
    },
}

#[derive(Debug, Clone)]
pub struct MapIcon {
    pub direction_and_type: i8,
    pub x: i8,
    pub z: i8,
}

#[derive(Debug, Clone)]
pub enum CombatEvent {
    EnterCombat,
    EndCombat {
        duration: i32,
        entity_id: i32,
    },
    EntityDead {
        player_id: i32,
        entity_id: i32,
        message_json: String,
    },
    Unknown {
        event: i32,
    },
}

#[derive(Debug, Clone)]
pub struct EntityProperty {
    pub key: String,
    pub value: f64,
    pub modifiers: Vec<EntityPropertyModifier>,
}

#[derive(Debug, Clone)]
pub struct EntityPropertyModifier {
    pub uuid: String,
    pub amount: f64,
    pub operation: i8,
}

#[derive(Debug, Clone)]
pub enum WorldBorderUpdate {
    SetSize {
        diameter: f64,
    },
    LerpSize {
        old_diameter: f64,
        new_diameter: f64,
        speed_ms: i64,
    },
    SetCenter {
        x: f64,
        z: f64,
    },
    Initialize {
        x: f64,
        z: f64,
        old_diameter: f64,
        new_diameter: f64,
        speed_ms: i64,
        portal_teleport_boundary: i32,
        warning_time: i32,
        warning_blocks: i32,
    },
    SetWarningTime {
        seconds: i32,
    },
    SetWarningBlocks {
        blocks: i32,
    },
    Unknown,
}

#[derive(Debug)]
pub struct ChunkMeta {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub primary_bit_mask: u16,
}

#[derive(Debug)]
pub struct ChunkBulkData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub primary_bit_mask: u16,
    pub data: Vec<u8>,
}
