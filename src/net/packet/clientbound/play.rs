use super::{
    ChunkBulkData, ChunkMeta, ClientboundPacket, CombatEvent, EntityProperty,
    EntityPropertyModifier, MapIcon, WorldBorderUpdate,
};
use crate::net::{metadata, player_list, protocol::PacketBuffer, slot};
use crate::world::chunk::{CHUNK_SIZE, NIBBLE_SECTION_BYTES, SECTION_VOLUME};
use std::io;

fn fixed_point_i32(v: i32) -> f32 {
    v as f32 / 32.0
}

fn rel_move(v: i8) -> f32 {
    v as f32 / 32.0
}

fn velocity(v: i16) -> f32 {
    v as f32 / 8000.0
}

fn player_velocity(v: i16) -> f64 {
    v as f64 / 8000.0
}

fn angle(v: u8) -> f32 {
    v as f32 * 360.0 / 256.0
}

fn fixed_sound_coord(v: i32) -> f32 {
    v as f32 / 8.0
}

fn unpack_position_x(raw: i64) -> i32 {
    (raw >> 38) as i32
}

fn unpack_position_y(raw: i64) -> i32 {
    ((raw >> 26) & 0xFFF) as i32
}

fn unpack_position_z(raw: i64) -> i32 {
    (raw << 38 >> 38) as i32
}

pub fn chunk_data_len(primary_bit_mask: u16, sky_light: bool, full_chunk: bool) -> usize {
    let section_count = primary_bit_mask.count_ones() as usize;
    let block_data = section_count * 2 * SECTION_VOLUME;
    let block_light = section_count * NIBBLE_SECTION_BYTES;
    let sky_light = if sky_light {
        section_count * NIBBLE_SECTION_BYTES
    } else {
        0
    };
    let biome = if full_chunk {
        CHUNK_SIZE * CHUNK_SIZE
    } else {
        0
    };
    block_data + block_light + sky_light + biome
}

impl ClientboundPacket {
    pub fn parse_play(id: i32, data: &[u8]) -> io::Result<Self> {
        let mut buf = PacketBuffer::new(data.to_vec());
        match id {
            0x00 => Ok(ClientboundPacket::KeepAlive {
                id: buf.read_varint()?,
            }),
            0x01 => Ok(ClientboundPacket::JoinGame {
                entity_id: buf.read_int()?,
                gamemode: buf.read_unsigned_byte()?,
                dimension: buf.read_byte()?,
                difficulty: buf.read_unsigned_byte()?,
                max_players: buf.read_unsigned_byte()?,
                level_type: buf.read_string()?,
                reduced_debug: buf.read_bool()?,
            }),
            0x02 => Ok(ClientboundPacket::ChatMessage {
                json: buf.read_string()?,
                position: buf.read_byte()?,
            }),
            0x03 => Ok(ClientboundPacket::TimeUpdate {
                world_time: buf.read_long()?,
                day_time: buf.read_long()?,
            }),
            0x05 => {
                let raw = buf.read_long()?;
                let x = (raw >> 38) as i32;
                let y = ((raw >> 26) & 0xFFF) as i32;
                let z = (raw << 38 >> 38) as i32;
                Ok(ClientboundPacket::SpawnPosition { x, y, z })
            }
            0x06 => Ok(ClientboundPacket::UpdateHealth {
                health: buf.read_float()?,
                food: buf.read_varint()?,
                saturation: buf.read_float()?,
            }),
            0x07 => Ok(ClientboundPacket::Respawn {
                dimension: buf.read_int()?,
                difficulty: buf.read_unsigned_byte()?,
                gamemode: buf.read_unsigned_byte()?,
                level_type: buf.read_string()?,
            }),
            0x08 => Ok(ClientboundPacket::PlayerPositionAndLook {
                x: buf.read_double()?,
                y: buf.read_double()?,
                z: buf.read_double()?,
                yaw: buf.read_float()?,
                pitch: buf.read_float()?,
                flags: buf.read_byte()?,
            }),
            0x09 => Ok(ClientboundPacket::HeldItemChange {
                slot: buf.read_byte()?,
            }),
            0x0A => {
                let entity_id = buf.read_varint()?;
                let raw = buf.read_long()?;
                Ok(ClientboundPacket::UseBed {
                    entity_id,
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                })
            }
            0x0B => Ok(ClientboundPacket::Animation {
                entity_id: buf.read_varint()?,
                animation: buf.read_unsigned_byte()?,
            }),
            0x04 => Ok(ClientboundPacket::EntityEquipment {
                entity_id: buf.read_varint()?,
                slot: buf.read_short()?,
                item: slot::read_slot(&mut buf)?,
            }),
            0x10 => Ok(ClientboundPacket::SpawnGlobalEntity {
                entity_id: buf.read_varint()?,
                entity_type: buf.read_byte()?,
                x: fixed_point_i32(buf.read_int()?),
                y: fixed_point_i32(buf.read_int()?),
                z: fixed_point_i32(buf.read_int()?),
            }),
            0x11 => Ok(ClientboundPacket::ExperienceOrbSpawn {
                entity_id: buf.read_varint()?,
                x: fixed_point_i32(buf.read_int()?),
                y: fixed_point_i32(buf.read_int()?),
                z: fixed_point_i32(buf.read_int()?),
                count: buf.read_short()?,
            }),
            0x0C => {
                let entity_id = buf.read_varint()?;
                let uuid = buf.read_uuid()?;
                let x = fixed_point_i32(buf.read_int()?);
                let y = fixed_point_i32(buf.read_int()?);
                let z = fixed_point_i32(buf.read_int()?);
                let yaw = angle(buf.read_unsigned_byte()?);
                let pitch = angle(buf.read_unsigned_byte()?);
                let current_item = buf.read_short()?;
                let metadata = metadata::read_entity_metadata(&mut buf)?;
                Ok(ClientboundPacket::EntitySpawn {
                    entity_id,
                    spawn_kind: crate::net::packet::EntitySpawnKind::Player,
                    entity_type: -1,
                    uuid: Some(uuid),
                    current_item,
                    object_data: 0,
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    head_yaw: yaw,
                    velocity: [0.0, 0.0, 0.0],
                    metadata,
                })
            }
            0x0E => {
                let entity_id = buf.read_varint()?;
                let entity_type = buf.read_byte()? as i32;
                let x = fixed_point_i32(buf.read_int()?);
                let y = fixed_point_i32(buf.read_int()?);
                let z = fixed_point_i32(buf.read_int()?);
                let pitch = angle(buf.read_unsigned_byte()?);
                let yaw = angle(buf.read_unsigned_byte()?);
                let object_data = buf.read_int()?;
                let mut spawn_velocity = [0.0, 0.0, 0.0];
                if buf.remaining() >= 6 {
                    spawn_velocity = [
                        velocity(buf.read_short()?),
                        velocity(buf.read_short()?),
                        velocity(buf.read_short()?),
                    ];
                }
                Ok(ClientboundPacket::EntitySpawn {
                    entity_id,
                    spawn_kind: crate::net::packet::EntitySpawnKind::Object,
                    entity_type,
                    uuid: None,
                    current_item: -1,
                    object_data,
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    head_yaw: yaw,
                    velocity: spawn_velocity,
                    metadata: Vec::new(),
                })
            }
            0x0F => {
                let entity_id = buf.read_varint()?;
                let entity_type = buf.read_unsigned_byte()? as i32;
                let x = fixed_point_i32(buf.read_int()?);
                let y = fixed_point_i32(buf.read_int()?);
                let z = fixed_point_i32(buf.read_int()?);
                let yaw = angle(buf.read_unsigned_byte()?);
                let pitch = angle(buf.read_unsigned_byte()?);
                let head_yaw = angle(buf.read_unsigned_byte()?);
                let spawn_velocity = if buf.remaining() >= 6 {
                    [
                        velocity(buf.read_short()?),
                        velocity(buf.read_short()?),
                        velocity(buf.read_short()?),
                    ]
                } else {
                    [0.0, 0.0, 0.0]
                };
                let metadata = metadata::read_entity_metadata(&mut buf)?;
                Ok(ClientboundPacket::EntitySpawn {
                    entity_id,
                    spawn_kind: crate::net::packet::EntitySpawnKind::Mob,
                    entity_type,
                    uuid: None,
                    current_item: -1,
                    object_data: 0,
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    head_yaw,
                    velocity: spawn_velocity,
                    metadata,
                })
            }
            0x13 => {
                let count = buf.read_varint()?.max(0) as usize;
                let mut ids = Vec::with_capacity(count);
                for _ in 0..count {
                    ids.push(buf.read_varint()?);
                }
                Ok(ClientboundPacket::DestroyEntities { ids })
            }
            0x0D => Ok(ClientboundPacket::CollectItem {
                collected_entity_id: buf.read_varint()?,
                collector_entity_id: buf.read_varint()?,
            }),
            0x14 => Ok(ClientboundPacket::Entity {
                entity_id: buf.read_varint()?,
            }),
            0x15 => Ok(ClientboundPacket::EntityMove {
                entity_id: buf.read_varint()?,
                dx: rel_move(buf.read_byte()?),
                dy: rel_move(buf.read_byte()?),
                dz: rel_move(buf.read_byte()?),
                on_ground: buf.read_bool()?,
            }),
            0x16 => Ok(ClientboundPacket::EntityLook {
                entity_id: buf.read_varint()?,
                yaw: angle(buf.read_unsigned_byte()?),
                pitch: angle(buf.read_unsigned_byte()?),
                on_ground: buf.read_bool()?,
            }),
            0x17 => Ok(ClientboundPacket::EntityMoveLook {
                entity_id: buf.read_varint()?,
                dx: rel_move(buf.read_byte()?),
                dy: rel_move(buf.read_byte()?),
                dz: rel_move(buf.read_byte()?),
                yaw: angle(buf.read_unsigned_byte()?),
                pitch: angle(buf.read_unsigned_byte()?),
                on_ground: buf.read_bool()?,
            }),
            0x18 => Ok(ClientboundPacket::EntityTeleport {
                entity_id: buf.read_varint()?,
                x: fixed_point_i32(buf.read_int()?),
                y: fixed_point_i32(buf.read_int()?),
                z: fixed_point_i32(buf.read_int()?),
                yaw: angle(buf.read_unsigned_byte()?),
                pitch: angle(buf.read_unsigned_byte()?),
                on_ground: buf.read_bool()?,
            }),
            0x19 => Ok(ClientboundPacket::EntityHeadLook {
                entity_id: buf.read_varint()?,
                head_yaw: angle(buf.read_unsigned_byte()?),
            }),
            0x1A => Ok(ClientboundPacket::EntityStatus {
                entity_id: buf.read_int()?,
                status: buf.read_byte()?,
            }),
            0x1B => Ok(ClientboundPacket::AttachEntity {
                entity_id: buf.read_int()?,
                vehicle_id: buf.read_int()?,
                leash: buf.read_bool()?,
            }),
            0x1C => Ok(ClientboundPacket::EntityMetadata {
                entity_id: buf.read_varint()?,
                metadata: metadata::read_entity_metadata(&mut buf)?,
            }),
            0x1D => Ok(ClientboundPacket::EntityEffect {
                entity_id: buf.read_varint()?,
                effect_id: buf.read_byte()?,
                amplifier: buf.read_byte()?,
                duration: buf.read_varint()?,
                hide_particles: buf.read_bool()?,
            }),
            0x1E => Ok(ClientboundPacket::RemoveEntityEffect {
                entity_id: buf.read_varint()?,
                effect_id: buf.read_byte()?,
            }),
            0x20 => Self::parse_entity_properties(&mut buf),
            0x12 => Ok(ClientboundPacket::EntityVelocity {
                entity_id: buf.read_varint()?,
                vx: player_velocity(buf.read_short()?),
                vy: player_velocity(buf.read_short()?),
                vz: player_velocity(buf.read_short()?),
            }),
            0x21 => {
                let chunk_x = buf.read_int()?;
                let chunk_z = buf.read_int()?;
                let full_chunk = buf.read_bool()?;
                let primary_bit_mask = buf.read_unsigned_short()?;
                let data_len = buf.read_varint()? as usize;
                let data = buf.read_bytes(data_len)?;
                Ok(ClientboundPacket::ChunkData {
                    chunk_x,
                    chunk_z,
                    full_chunk,
                    primary_bit_mask,
                    data,
                })
            }
            0x22 | 0x23 => Self::parse_play_block_updates(id, &mut buf),
            0x24 => {
                let raw = buf.read_long()?;
                Ok(ClientboundPacket::BlockAction {
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                    byte1: buf.read_unsigned_byte()?,
                    byte2: buf.read_unsigned_byte()?,
                    block_type: buf.read_varint()?,
                })
            }
            0x25 => {
                let entity_id = buf.read_varint()?;
                let raw = buf.read_long()?;
                Ok(ClientboundPacket::BlockBreakAnimation {
                    entity_id,
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                    destroy_stage: buf.read_byte()?,
                })
            }
            0x2D => {
                let window_id = buf.read_unsigned_byte()?;
                let window_type = buf.read_string()?;
                let title_json = buf.read_string()?;
                let slot_count = buf.read_unsigned_byte()?;
                let entity_id = if window_type == "EntityHorse" {
                    Some(buf.read_int()?)
                } else {
                    None
                };
                Ok(ClientboundPacket::OpenWindow {
                    window_id,
                    window_type,
                    title_json,
                    slot_count,
                    entity_id,
                })
            }
            0x2E => Ok(ClientboundPacket::CloseWindow {
                window_id: buf.read_unsigned_byte()?,
            }),
            0x2F => Ok(ClientboundPacket::SetSlot {
                window_id: buf.read_byte()?,
                slot: buf.read_short()?,
                item: slot::read_slot(&mut buf)?,
            }),
            0x30 => {
                let window_id = buf.read_unsigned_byte()?;
                let count = buf.read_short()?.max(0) as usize;
                let mut slots = Vec::with_capacity(count);
                for _ in 0..count {
                    slots.push(slot::read_slot(&mut buf)?);
                }
                Ok(ClientboundPacket::WindowItems { window_id, slots })
            }
            0x32 => Ok(ClientboundPacket::ConfirmTransaction {
                window_id: buf.read_unsigned_byte()?,
                action_number: buf.read_short()?,
                accepted: buf.read_bool()?,
            }),
            0x31 => Ok(ClientboundPacket::WindowProperty {
                window_id: buf.read_unsigned_byte()?,
                property: buf.read_short()?,
                value: buf.read_short()?,
            }),
            0x33 => {
                let raw = buf.read_long()?;
                let lines = [
                    buf.read_string()?,
                    buf.read_string()?,
                    buf.read_string()?,
                    buf.read_string()?,
                ];
                Ok(ClientboundPacket::UpdateSign {
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                    lines,
                })
            }
            0x34 => {
                let item_damage = buf.read_varint()?;
                let scale = buf.read_byte()?;
                let icon_count = buf.read_varint()?.max(0) as usize;
                let mut icons = Vec::with_capacity(icon_count);
                for _ in 0..icon_count {
                    icons.push(MapIcon {
                        direction_and_type: buf.read_byte()?,
                        x: buf.read_byte()?,
                        z: buf.read_byte()?,
                    });
                }
                let columns = buf.read_unsigned_byte()?;
                let mut rows = 0;
                let mut x = 0;
                let mut z = 0;
                let mut map_data = Vec::new();
                if columns > 0 {
                    rows = buf.read_unsigned_byte()?;
                    x = buf.read_unsigned_byte()?;
                    z = buf.read_unsigned_byte()?;
                    let len = buf.read_varint()?.max(0) as usize;
                    map_data = buf.read_bytes(len)?;
                }
                Ok(ClientboundPacket::MapData {
                    item_damage,
                    scale,
                    icons,
                    columns,
                    rows,
                    x,
                    z,
                    data: map_data,
                })
            }
            0x35 => {
                let raw = buf.read_long()?;
                let action = buf.read_unsigned_byte()?;
                let nbt = buf.read_bytes(buf.remaining())?;
                Ok(ClientboundPacket::UpdateBlockEntity {
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                    action,
                    nbt,
                })
            }
            0x36 => {
                let raw = buf.read_long()?;
                Ok(ClientboundPacket::SignEditorOpen {
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                })
            }
            0x3A => Ok(ClientboundPacket::TabComplete {
                matches: {
                    let count = buf.read_varint()?.max(0) as usize;
                    let mut matches = Vec::with_capacity(count);
                    for _ in 0..count {
                        matches.push(buf.read_string()?);
                    }
                    matches
                },
            }),
            0x39 => Ok(ClientboundPacket::PlayerAbilities {
                flags: buf.read_unsigned_byte()?,
                flying_speed: buf.read_float()?,
                walking_speed: buf.read_float()?,
            }),
            0x38 => {
                let (action, players) = player_list::read_player_list_item(&mut buf)?;
                Ok(ClientboundPacket::PlayerListItem { action, players })
            }
            0x27 => {
                let x = buf.read_float()?;
                let y = buf.read_float()?;
                let z = buf.read_float()?;
                let radius = buf.read_float()?;
                let count = buf.read_int()?.max(0) as usize;
                let mut records = Vec::with_capacity(count);
                for _ in 0..count {
                    records.push([buf.read_byte()?, buf.read_byte()?, buf.read_byte()?]);
                }
                Ok(ClientboundPacket::Explosion {
                    x,
                    y,
                    z,
                    radius,
                    records,
                    player_motion: [buf.read_float()?, buf.read_float()?, buf.read_float()?],
                })
            }
            0x28 => {
                let effect_id = buf.read_int()?;
                let raw = buf.read_long()?;
                Ok(ClientboundPacket::Effect {
                    effect_id,
                    x: unpack_position_x(raw),
                    y: unpack_position_y(raw),
                    z: unpack_position_z(raw),
                    data: buf.read_int()?,
                    disable_relative_volume: buf.read_bool()?,
                })
            }
            0x29 => Ok(ClientboundPacket::NamedSoundEffect {
                name: buf.read_string()?,
                x: fixed_sound_coord(buf.read_int()?),
                y: fixed_sound_coord(buf.read_int()?),
                z: fixed_sound_coord(buf.read_int()?),
                volume: buf.read_float()?,
                pitch: buf.read_unsigned_byte()? as f32 / 63.0,
            }),
            0x2A => {
                let particle_id = buf.read_int()?;
                let long_distance = buf.read_bool()?;
                let x = buf.read_float()?;
                let y = buf.read_float()?;
                let z = buf.read_float()?;
                let offset_x = buf.read_float()?;
                let offset_y = buf.read_float()?;
                let offset_z = buf.read_float()?;
                let speed = buf.read_float()?;
                let count = buf.read_int()?;
                let data_len = match particle_id {
                    36 => 2,
                    37 | 38 => 1,
                    _ => 0,
                };
                let mut data = Vec::with_capacity(data_len);
                for _ in 0..data_len {
                    data.push(buf.read_varint()?);
                }
                Ok(ClientboundPacket::Particle {
                    particle_id,
                    long_distance,
                    x,
                    y,
                    z,
                    offset_x,
                    offset_y,
                    offset_z,
                    speed,
                    count,
                    data,
                })
            }
            0x37 => {
                let count = buf.read_varint()?.max(0) as usize;
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    entries.push((buf.read_string()?, buf.read_varint()?));
                }
                Ok(ClientboundPacket::Statistics { entries })
            }
            0x3F => Ok(ClientboundPacket::PluginMessage {
                channel: buf.read_string()?,
                data: buf.read_bytes(buf.remaining())?,
            }),
            0x40 => Ok(ClientboundPacket::Disconnect {
                reason: buf.read_string()?,
            }),
            0x41 => Ok(ClientboundPacket::ServerDifficulty {
                difficulty: buf.read_unsigned_byte()?,
            }),
            0x42 => Self::parse_combat_event(&mut buf),
            0x43 => Ok(ClientboundPacket::Camera {
                camera_id: buf.read_varint()?,
            }),
            0x26 => {
                let sky_light = buf.read_bool()?;
                let column_count = buf.read_varint()? as usize;
                let mut meta = Vec::with_capacity(column_count);
                for _ in 0..column_count {
                    meta.push(ChunkMeta {
                        chunk_x: buf.read_int()?,
                        chunk_z: buf.read_int()?,
                        primary_bit_mask: buf.read_unsigned_short()?,
                    });
                }

                let mut chunks = Vec::with_capacity(column_count);
                for chunk in meta {
                    let data_len = chunk_data_len(chunk.primary_bit_mask, sky_light, true);
                    chunks.push(ChunkBulkData {
                        chunk_x: chunk.chunk_x,
                        chunk_z: chunk.chunk_z,
                        primary_bit_mask: chunk.primary_bit_mask,
                        data: buf.read_bytes(data_len)?,
                    });
                }
                Ok(ClientboundPacket::MapChunkBulk { sky_light, chunks })
            }
            0x1F => Ok(ClientboundPacket::SetExperience {
                bar: buf.read_float()?,
                level: buf.read_varint()?,
                total: buf.read_varint()?,
            }),
            0x3B => Ok(ClientboundPacket::ScoreboardObjective {
                name: buf.read_string()?,
                mode: buf.read_byte()?,
                value: if buf.remaining() > 0 {
                    Some(buf.read_string()?)
                } else {
                    None
                },
                render_type: if buf.remaining() > 0 {
                    Some(buf.read_string()?)
                } else {
                    None
                },
            }),
            0x3C => Ok(ClientboundPacket::UpdateScore {
                item_name: buf.read_string()?,
                action: buf.read_byte()?,
                score_name: buf.read_string()?,
                value: if buf.remaining() > 0 {
                    Some(buf.read_varint()?)
                } else {
                    None
                },
            }),
            0x3D => Ok(ClientboundPacket::DisplayScoreboard {
                position: buf.read_byte()?,
                score_name: buf.read_string()?,
            }),
            0x3E => Self::parse_teams(&mut buf),
            0x2B => Ok(ClientboundPacket::ChangeGameState {
                reason: buf.read_unsigned_byte()?,
                value: buf.read_float()?,
            }),
            0x45 => Self::parse_title(&mut buf),
            0x44 => Self::parse_world_border(&mut buf),
            0x47 => Ok(ClientboundPacket::PlayerListHeaderFooter {
                header_json: buf.read_string()?,
                footer_json: buf.read_string()?,
            }),
            0x48 => Ok(ClientboundPacket::ResourcePackSend {
                url: buf.read_string()?,
                hash: buf.read_string()?,
            }),
            0x46 => Ok(ClientboundPacket::SetCompression {
                threshold: buf.read_varint()?,
            }),
            _ => Ok(ClientboundPacket::Unknown { id }),
        }
    }
    fn parse_teams(buf: &mut PacketBuffer) -> io::Result<Self> {
        let name = buf.read_string()?;
        let mode = buf.read_byte()?;
        let mut display_name = None;
        let mut prefix = None;
        let mut suffix = None;
        let mut friendly_flags = None;
        let mut name_tag_visibility = None;
        let mut color = None;
        let mut players = Vec::new();

        if matches!(mode, 0 | 2) {
            display_name = Some(buf.read_string()?);
            prefix = Some(buf.read_string()?);
            suffix = Some(buf.read_string()?);
            friendly_flags = Some(buf.read_byte()?);
            name_tag_visibility = Some(buf.read_string()?);
            color = Some(buf.read_byte()?);
        }
        if matches!(mode, 0 | 3 | 4) {
            let count = buf.read_varint()?.max(0) as usize;
            players.reserve(count);
            for _ in 0..count {
                players.push(buf.read_string()?);
            }
        }

        Ok(ClientboundPacket::Teams {
            name,
            mode,
            display_name,
            prefix,
            suffix,
            friendly_flags,
            name_tag_visibility,
            color,
            players,
        })
    }

    fn parse_title(buf: &mut PacketBuffer) -> io::Result<Self> {
        let action = buf.read_varint()?;
        match action {
            0 | 1 => Ok(ClientboundPacket::Title {
                action,
                text_json: Some(buf.read_string()?),
                fade_in: None,
                stay: None,
                fade_out: None,
            }),
            2 => Ok(ClientboundPacket::Title {
                action,
                text_json: None,
                fade_in: Some(buf.read_int()?),
                stay: Some(buf.read_int()?),
                fade_out: Some(buf.read_int()?),
            }),
            3 | 4 => Ok(ClientboundPacket::Title {
                action,
                text_json: None,
                fade_in: None,
                stay: None,
                fade_out: None,
            }),
            _ => Ok(ClientboundPacket::Title {
                action,
                text_json: None,
                fade_in: None,
                stay: None,
                fade_out: None,
            }),
        }
    }

    fn parse_world_border(buf: &mut PacketBuffer) -> io::Result<Self> {
        let action = buf.read_varint()?;
        let update = match action {
            0 => WorldBorderUpdate::SetSize {
                diameter: buf.read_double()?,
            },
            1 => WorldBorderUpdate::LerpSize {
                old_diameter: buf.read_double()?,
                new_diameter: buf.read_double()?,
                speed_ms: buf.read_varlong()?,
            },
            2 => WorldBorderUpdate::SetCenter {
                x: buf.read_double()?,
                z: buf.read_double()?,
            },
            3 => WorldBorderUpdate::Initialize {
                x: buf.read_double()?,
                z: buf.read_double()?,
                old_diameter: buf.read_double()?,
                new_diameter: buf.read_double()?,
                speed_ms: buf.read_varlong()?,
                portal_teleport_boundary: buf.read_varint()?,
                warning_time: buf.read_varint()?,
                warning_blocks: buf.read_varint()?,
            },
            4 => WorldBorderUpdate::SetWarningTime {
                seconds: buf.read_varint()?,
            },
            5 => WorldBorderUpdate::SetWarningBlocks {
                blocks: buf.read_varint()?,
            },
            _ => WorldBorderUpdate::Unknown,
        };
        Ok(ClientboundPacket::WorldBorder { action, update })
    }

    fn parse_entity_properties(buf: &mut PacketBuffer) -> io::Result<Self> {
        let entity_id = buf.read_varint()?;
        let count = buf.read_int()?.max(0) as usize;
        let mut properties = Vec::with_capacity(count);
        for _ in 0..count {
            let key = buf.read_string()?;
            let value = buf.read_double()?;
            let modifier_count = buf.read_varint()?.max(0) as usize;
            let mut modifiers = Vec::with_capacity(modifier_count);
            for _ in 0..modifier_count {
                modifiers.push(EntityPropertyModifier {
                    uuid: buf.read_uuid()?,
                    amount: buf.read_double()?,
                    operation: buf.read_byte()?,
                });
            }
            properties.push(EntityProperty {
                key,
                value,
                modifiers,
            });
        }
        Ok(ClientboundPacket::EntityProperties {
            entity_id,
            properties,
        })
    }

    fn parse_combat_event(buf: &mut PacketBuffer) -> io::Result<Self> {
        let event = buf.read_varint()?;
        let event = match event {
            0 => CombatEvent::EnterCombat,
            1 => CombatEvent::EndCombat {
                duration: buf.read_varint()?,
                entity_id: buf.read_int()?,
            },
            2 => CombatEvent::EntityDead {
                player_id: buf.read_varint()?,
                entity_id: buf.read_int()?,
                message_json: buf.read_string()?,
            },
            event => CombatEvent::Unknown { event },
        };
        Ok(ClientboundPacket::CombatEvent { event })
    }

    fn parse_play_block_updates(id: i32, buf: &mut PacketBuffer) -> io::Result<Self> {
        match id {
            0x23 => {
                let raw = buf.read_long()?;
                let x = (raw >> 38) as i32;
                let y = ((raw >> 26) & 0xFFF) as i32;
                let z = (raw << 38 >> 38) as i32;
                let block_state = buf.read_varint()? as u16;
                Ok(ClientboundPacket::BlockChange {
                    x,
                    y,
                    z,
                    block_state,
                })
            }
            0x22 => {
                let chunk_x = buf.read_int()?;
                let chunk_z = buf.read_int()?;
                let record_count = buf.read_varint()? as usize;
                let mut records = Vec::with_capacity(record_count);
                for _ in 0..record_count {
                    let raw = buf.read_unsigned_short()?;
                    let block_state = buf.read_varint()? as u16;
                    records.push((raw, block_state));
                }
                Ok(ClientboundPacket::MultiBlockChange {
                    chunk_x,
                    chunk_z,
                    records,
                })
            }
            _ => Ok(ClientboundPacket::Unknown { id }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{player_velocity, ClientboundPacket, PacketBuffer};

    #[test]
    fn local_player_velocity_uses_double_packet_scaling() {
        let value = player_velocity(12_345);
        assert_eq!(value, 12_345_f64 / 8000.0);
        assert_ne!(value, (12_345_f32 / 8000.0) as f64);
    }

    #[test]
    fn horse_window_consumes_its_trailing_entity_id() {
        let mut payload = PacketBuffer::new(Vec::new());
        payload.write_unsigned_byte(7);
        payload.write_string("EntityHorse");
        payload.write_string(r#"{"text":"Horse"}"#);
        payload.write_unsigned_byte(17);
        payload.write_int(1234);

        let packet = ClientboundPacket::parse_play(0x2d, &payload.into_inner()).unwrap();
        assert!(matches!(
            packet,
            ClientboundPacket::OpenWindow {
                window_id: 7,
                slot_count: 17,
                entity_id: Some(1234),
                ..
            }
        ));
    }
}
