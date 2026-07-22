use crate::client::inventory::Inventory;
use crate::client::particles::ParticleSystem;
use crate::client::player::Player;
use crate::client::session::SessionState;
use crate::entity::EntityManager;
use crate::net;
use crate::world;
use crate::{audio::AudioBackend, world::network::ChunkDataOptions};
use smallvec::SmallVec;
use std::collections::VecDeque;

/// Inline capacity for the per-frame mesh list. A typical poll produces a
/// handful of chunk meshes, so this keeps small updates on the stack.
pub type MeshList = SmallVec<[world::mesh::ChunkMesh; 2]>;
/// Inline capacity for sign-text updates collected while scanning packets.
/// Each sign packet contributes at most one entry, and most packets produce
/// none, so a single inline slot covers the common case without heap traffic.
pub type SignUpdateList = SmallVec<[(i32, i32, i32, [String; 4]); 1]>;

mod effects;
mod entity;
mod inventory;
mod outbound;
mod session;
mod world_packets;

/// Scan enough packets to keep UI/session state responsive, but retain the
/// original world-data budget that prevents a chunk burst from stalling a frame.
const INBOUND_SCAN_BUDGET: usize = 64;
/// Chunk columns are the dominant main-thread packet cost. Count every column
/// inside MapChunkBulk as work instead of treating a whole bulk as one packet.
// Columns applied per frame from the deferred world queue. Too low and the
// player steps into unloaded columns (movement hold) long before meshes appear.
const WORLD_WORK_BUDGET: usize = 12;
const MULTI_BLOCK_RECORD_BUDGET: usize = 256;
/// A plugin can request thousands of particles per packet. Bound the total
/// construction work across all particle packets handled in one render frame.
const SERVER_PARTICLE_BUDGET: usize = 512;

pub use outbound::{
    send_animation, send_block_placement_slot, send_book_update, send_chat_message,
    send_client_settings, send_close_window, send_creative_inventory_action,
    send_creative_inventory_action_slot, send_digging_cancel, send_digging_finish,
    send_digging_start, send_drop_selected_item, send_dynamic_packet, send_enchant_item,
    send_held_item_change, send_player_abilities, send_player_tick, send_release_use_item,
    send_resource_pack_status, send_respawn_request, send_tab_complete, send_update_sign,
    send_use_entity_attack, send_use_entity_interact, send_use_entity_interact_at, send_use_item,
    sync_held_item, ClientNetworkState,
};

#[derive(Clone, Copy, Default)]
pub struct NetworkDebugProfile {
    pub worst_packet_kind: &'static str,
    pub worst_packet_us: u64,
    pub worst_packet_units: u32,
    pub worst_hook_us: u64,
    pub worst_session_us: u64,
    pub worst_inventory_us: u64,
    pub worst_entity_us: u64,
    pub worst_world_us: u64,
    pub scheduler_us: u64,
    pub scanned_packets: u32,
    pub handled_packets: u32,
    pub deferred_packets: u32,
}

#[derive(Default)]
pub struct NetworkPollResult {
    pub meshes: MeshList,
    pub debug: NetworkDebugProfile,
}

pub fn poll_network(
    connection: &mut Option<net::connection::Connection>,
    scripts: &mut crate::scripting::ScriptManager,
    player: &mut Player,
    network_state: &mut ClientNetworkState,
    world: &mut world::World,
    inventory: &mut Inventory,
    session: &mut SessionState,
    entities: &mut EntityManager,
    particles: &mut ParticleSystem,
    audio: &mut impl AudioBackend,
    i18n: Option<&crate::ui::i18n::I18n>,
) -> NetworkPollResult {
    let mut meshes = MeshList::new();
    let mut debug = NetworkDebugProfile::default();
    let Some(connection) = connection else {
        return NetworkPollResult { meshes, debug };
    };

    // Scoreboard and other independent session packets may scan ahead of chunk
    // columns, but a respawn/join is an ordering barrier: it must never clear the
    // world before older deferred chunks have been applied.
    let scheduler_started = std::time::Instant::now();
    let mut packets = Vec::with_capacity(INBOUND_SCAN_BUDGET + WORLD_WORK_BUDGET);
    // Continue scanning independent packets even while chunk work is queued.
    // `stage_inbound_packets` persists any join/respawn ordering barrier across
    // frames, so only packets that are safe to overtake the backlog run early.
    let incoming = connection.poll(INBOUND_SCAN_BUDGET);
    debug.scanned_packets = incoming.len() as u32;
    stage_inbound_packets(
        &mut connection.deferred_packets,
        &mut connection.deferred_barrier_count,
        incoming,
        &mut packets,
    );
    drain_deferred_packets(
        &mut connection.deferred_packets,
        &mut connection.deferred_barrier_count,
        &mut packets,
        WORLD_WORK_BUDGET,
    );
    debug.deferred_packets = connection.deferred_packets.len() as u32;
    debug.scheduler_us = scheduler_started.elapsed().as_micros() as u64;

    let hook_inbound_packets = scripts.has_callbacks("network.packet.inbound");
    let mut remaining_server_particles = SERVER_PARTICLE_BUDGET;
    for packet in packets {
        debug.handled_packets = debug.handled_packets.saturating_add(1);
        let (_, original_packet_units) = packet_debug_info(&packet);
        let packet_started = std::time::Instant::now();
        let hook_started = std::time::Instant::now();
        let packet = if hook_inbound_packets {
            if let Some(dynamic) =
                crate::net::dynamic_packet::DynamicPacket::from_v47_clientbound(&packet)
            {
                let hooked = scripts.process_packet("network.packet.inbound", dynamic);
                match hooked.packet {
                    None => None,
                    Some(dynamic) => {
                        if hooked.modified {
                            match dynamic.into_v47_clientbound() {
                                Ok(packet) => Some(packet),
                                Err(error) => {
                                    log::warn!(
                                        target: "rustcraft::lua",
                                        "rejected invalid inbound packet modification: {error}"
                                    );
                                    Some(packet)
                                }
                            }
                        } else {
                            Some(packet)
                        }
                    }
                }
            } else {
                Some(packet)
            }
        } else {
            Some(packet)
        };
        let hook_us = hook_started.elapsed().as_micros() as u64;
        let Some(packet) = packet else {
            let packet_us = packet_started.elapsed().as_micros() as u64;
            if packet_us > debug.worst_packet_us {
                debug.worst_packet_kind = "hook_cancel";
                debug.worst_packet_us = packet_us;
                debug.worst_packet_units = original_packet_units;
                debug.worst_hook_us = hook_us;
                debug.worst_session_us = 0;
                debug.worst_inventory_us = 0;
                debug.worst_entity_us = 0;
                debug.worst_world_us = 0;
            }
            continue;
        };
        // A Lua hook may replace the protocol packet with a different kind.
        // Attribute downstream work to the packet that is actually handled.
        let (packet_kind, packet_units) = packet_debug_info(&packet);
        let mut packet = Some(packet);
        let mut sign_updates = SignUpdateList::new();

        let session_started = std::time::Instant::now();
        if let Some(unhandled) = session::handle_packet(
            connection,
            player,
            network_state,
            world,
            session,
            entities,
            packet.take(),
        ) {
            packet = Some(unhandled);
        }
        let session_us = session_started.elapsed().as_micros() as u64;
        let inventory_started = std::time::Instant::now();
        if let Some(unhandled) =
            inventory::handle_packet(connection, inventory, session, packet.take(), i18n)
        {
            packet = Some(unhandled);
        }
        let inventory_us = inventory_started.elapsed().as_micros() as u64;
        let entity_started = std::time::Instant::now();
        if let Some(unhandled) =
            entity::handle_packet(entities, session, player, audio, packet.take())
        {
            packet = Some(unhandled);
        }
        let entity_us = entity_started.elapsed().as_micros() as u64;
        let world_started = std::time::Instant::now();
        if let Some(net::packet::ClientboundPacket::Particle { count, .. }) = packet.as_mut() {
            if let Some(limited_count) =
                budget_server_particle_count(*count, &mut remaining_server_particles)
            {
                *count = limited_count;
            } else {
                packet = None;
            }
        }
        if let Some(unhandled) = world_packets::handle_packet(
            world,
            particles,
            audio,
            player,
            session.dimension,
            packet.take(),
            &mut meshes,
            &mut sign_updates,
        ) {
            packet = Some(unhandled);
        }
        let world_us = world_started.elapsed().as_micros() as u64;

        for (x, y, z, lines) in sign_updates {
            session.sign_data.insert((x, y, z), lines);
        }

        if packet.is_some() {
            // Unknown or not-yet-rendered protocol surfaces are intentionally ignored here.
        }

        let packet_us = packet_started.elapsed().as_micros() as u64;
        if packet_us > debug.worst_packet_us {
            debug.worst_packet_kind = packet_kind;
            debug.worst_packet_us = packet_us;
            debug.worst_packet_units = packet_units;
            debug.worst_hook_us = hook_us;
            debug.worst_session_us = session_us;
            debug.worst_inventory_us = inventory_us;
            debug.worst_entity_us = entity_us;
            debug.worst_world_us = world_us;
        }
    }

    NetworkPollResult { meshes, debug }
}

fn packet_debug_info(packet: &net::packet::ClientboundPacket) -> (&'static str, u32) {
    use net::packet::ClientboundPacket;
    match packet {
        ClientboundPacket::ChunkData { data, .. } => ("chunk", data.len() as u32),
        ClientboundPacket::MapChunkBulk { chunks, .. } => ("bulk", chunks.len() as u32),
        ClientboundPacket::BlockChange { .. } => ("block", 1),
        ClientboundPacket::MultiBlockChange { records, .. } => {
            ("multi_block", records.len() as u32)
        }
        ClientboundPacket::UpdateBlockEntity { .. } => ("block_entity", 1),
        ClientboundPacket::Particle { count, .. } => ("particle", (*count).max(0) as u32),
        ClientboundPacket::PluginMessage { data, .. } => ("plugin", data.len() as u32),
        ClientboundPacket::Teams { players, .. } => ("team", players.len() as u32),
        ClientboundPacket::ScoreboardObjective { .. } => ("objective", 1),
        ClientboundPacket::UpdateScore { .. } => ("score", 1),
        ClientboundPacket::DestroyEntities { ids } => ("destroy_entities", ids.len() as u32),
        ClientboundPacket::EntityMetadata { .. } => ("entity_metadata", 1),
        ClientboundPacket::EntitySpawn { .. } => ("entity_spawn", 1),
        ClientboundPacket::Respawn { .. } => ("respawn", 1),
        ClientboundPacket::JoinGame { .. } => ("join", 1),
        _ => ("other", 1),
    }
}

fn budget_server_particle_count(count: i32, remaining: &mut usize) -> Option<i32> {
    let requested = if count == 0 {
        1
    } else {
        usize::try_from(count).unwrap_or(0)
    };
    let allowed = requested.min(*remaining);
    if allowed == 0 {
        return None;
    }
    *remaining -= allowed;
    // In protocol 47 count=0 is the special exact-position single-particle
    // form; preserve it when the one required token is available.
    Some(if count == 0 { 0 } else { allowed as i32 })
}

fn stage_inbound_packets(
    deferred: &mut VecDeque<net::packet::ClientboundPacket>,
    deferred_barrier_count: &mut usize,
    incoming: impl IntoIterator<Item = net::packet::ClientboundPacket>,
    output: &mut Vec<net::packet::ClientboundPacket>,
) {
    // Once a join/respawn barrier is queued, every later packet in the wire
    // stream must stay behind it, including packets received on later frames.
    let mut preserve_order = *deferred_barrier_count > 0;
    for packet in incoming {
        if preserve_order || is_deferred_world_packet(&packet) {
            *deferred_barrier_count += usize::from(is_world_ordering_barrier(&packet));
            deferred.push_back(packet);
        } else if is_world_ordering_barrier(&packet) && !deferred.is_empty() {
            deferred.push_back(packet);
            *deferred_barrier_count += 1;
            preserve_order = true;
        } else {
            output.push(packet);
        }
    }
}

fn drain_deferred_packets(
    deferred: &mut VecDeque<net::packet::ClientboundPacket>,
    deferred_barrier_count: &mut usize,
    output: &mut Vec<net::packet::ClientboundPacket>,
    mut remaining_work: usize,
) {
    let mut scanned = 0;
    let mut remaining_multi_block_records = MULTI_BLOCK_RECORD_BUDGET;
    while scanned < INBOUND_SCAN_BUDGET {
        let Some(next) = deferred.front() else {
            break;
        };
        let consumes_world_work = match next {
            net::packet::ClientboundPacket::MapChunkBulk { chunks, .. } => !chunks.is_empty(),
            packet => is_deferred_world_packet(packet),
        };
        if consumes_world_work && remaining_work == 0 {
            break;
        }
        if matches!(
            next,
            net::packet::ClientboundPacket::MultiBlockChange { records, .. }
                if !records.is_empty() && remaining_multi_block_records == 0
        ) {
            break;
        }
        let Some(packet) = deferred.pop_front() else {
            break;
        };
        if is_world_ordering_barrier(&packet) {
            *deferred_barrier_count = deferred_barrier_count.saturating_sub(1);
        }
        scanned += 1;
        match packet {
            net::packet::ClientboundPacket::MapChunkBulk {
                sky_light,
                mut chunks,
            } => {
                if chunks.is_empty() {
                    continue;
                }
                let take = remaining_work.min(chunks.len());
                let remaining = chunks.split_off(take);
                output.push(net::packet::ClientboundPacket::MapChunkBulk { sky_light, chunks });
                remaining_work -= take;
                if !remaining.is_empty() {
                    deferred.push_front(net::packet::ClientboundPacket::MapChunkBulk {
                        sky_light,
                        chunks: remaining,
                    });
                }
            }
            net::packet::ClientboundPacket::MultiBlockChange {
                chunk_x,
                chunk_z,
                mut records,
            } => {
                if records.is_empty() {
                    continue;
                }
                let take = remaining_multi_block_records.min(records.len());
                let remaining = records.split_off(take);
                output.push(net::packet::ClientboundPacket::MultiBlockChange {
                    chunk_x,
                    chunk_z,
                    records,
                });
                remaining_multi_block_records -= take;
                remaining_work -= 1;
                if !remaining.is_empty() {
                    deferred.push_front(net::packet::ClientboundPacket::MultiBlockChange {
                        chunk_x,
                        chunk_z,
                        records: remaining,
                    });
                }
            }
            packet => {
                let consumes_world_work = is_deferred_world_packet(&packet);
                output.push(packet);
                if consumes_world_work {
                    remaining_work -= 1;
                }
            }
        }
    }
}

fn is_world_ordering_barrier(packet: &net::packet::ClientboundPacket) -> bool {
    matches!(
        packet,
        net::packet::ClientboundPacket::JoinGame { .. }
            | net::packet::ClientboundPacket::Respawn { .. }
    )
}

/// Packets whose application can mutate a chunk or its tile-entity data are
/// delayed together, so a later block/tile update cannot overtake its chunk.
fn is_deferred_world_packet(packet: &net::packet::ClientboundPacket) -> bool {
    matches!(
        packet,
        net::packet::ClientboundPacket::ChunkData { .. }
            | net::packet::ClientboundPacket::MapChunkBulk { .. }
            | net::packet::ClientboundPacket::BlockChange { .. }
            | net::packet::ClientboundPacket::MultiBlockChange { .. }
            | net::packet::ClientboundPacket::UpdateBlockEntity { .. }
    )
}

fn chunk_options_for_dimension(dimension: i8) -> ChunkDataOptions {
    ChunkDataOptions::sky_light(dimension == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::packet::{ChunkBulkData, ClientboundPacket};

    fn bulk_chunk(chunk_x: i32) -> ChunkBulkData {
        ChunkBulkData {
            chunk_x,
            chunk_z: 0,
            primary_bit_mask: 0,
            data: Vec::new(),
        }
    }

    #[test]
    fn bulk_chunk_columns_are_split_by_world_work_budget_in_fifo_order() {
        let mut deferred = VecDeque::from([
            ClientboundPacket::MapChunkBulk {
                sky_light: true,
                chunks: (0..6).map(bulk_chunk).collect(),
            },
            ClientboundPacket::BlockChange {
                x: 9,
                y: 64,
                z: 9,
                block_state: 1 << 4,
            },
        ]);
        let mut deferred_barrier_count = 0;
        let mut first = Vec::new();
        drain_deferred_packets(&mut deferred, &mut deferred_barrier_count, &mut first, 4);

        assert_eq!(first.len(), 1);
        let ClientboundPacket::MapChunkBulk { chunks, .. } = &first[0] else {
            panic!("first work item must remain a bulk packet");
        };
        assert_eq!(
            chunks.iter().map(|chunk| chunk.chunk_x).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );

        let mut second = Vec::new();
        drain_deferred_packets(&mut deferred, &mut deferred_barrier_count, &mut second, 4);
        assert_eq!(second.len(), 2);
        let ClientboundPacket::MapChunkBulk { chunks, .. } = &second[0] else {
            panic!("remaining bulk columns must stay ahead of later updates");
        };
        assert_eq!(
            chunks.iter().map(|chunk| chunk.chunk_x).collect::<Vec<_>>(),
            [4, 5]
        );
        assert!(matches!(
            second[1],
            ClientboundPacket::BlockChange { x: 9, .. }
        ));
        assert!(deferred.is_empty());
    }

    #[test]
    fn respawn_barrier_cannot_overtake_deferred_bulk_columns() {
        let incoming = vec![
            ClientboundPacket::MapChunkBulk {
                sky_light: true,
                chunks: (0..6).map(bulk_chunk).collect(),
            },
            ClientboundPacket::Respawn {
                dimension: -1,
                difficulty: 2,
                gamemode: 0,
                level_type: "default".to_string(),
            },
            ClientboundPacket::BlockChange {
                x: 9,
                y: 64,
                z: 9,
                block_state: 1 << 4,
            },
        ];
        let mut deferred = VecDeque::new();
        let mut deferred_barrier_count = 0;
        let mut immediate = Vec::new();
        stage_inbound_packets(
            &mut deferred,
            &mut deferred_barrier_count,
            incoming,
            &mut immediate,
        );
        assert!(immediate.is_empty());
        assert_eq!(deferred_barrier_count, 1);

        let mut first = Vec::new();
        drain_deferred_packets(&mut deferred, &mut deferred_barrier_count, &mut first, 4);
        assert_eq!(first.len(), 1);
        let ClientboundPacket::MapChunkBulk { chunks, .. } = &first[0] else {
            panic!("old bulk data must remain first");
        };
        assert_eq!(chunks.len(), 4);

        let mut second = Vec::new();
        drain_deferred_packets(&mut deferred, &mut deferred_barrier_count, &mut second, 4);
        assert_eq!(second.len(), 3);
        let ClientboundPacket::MapChunkBulk { chunks, .. } = &second[0] else {
            panic!("the rest of the old bulk must precede respawn");
        };
        assert_eq!(chunks.len(), 2);
        assert!(matches!(second[1], ClientboundPacket::Respawn { .. }));
        assert!(matches!(second[2], ClientboundPacket::BlockChange { .. }));
        assert!(deferred.is_empty());
        assert_eq!(deferred_barrier_count, 0);
    }

    #[test]
    fn independent_packets_can_scan_ahead_of_a_cross_frame_chunk_backlog() {
        let mut deferred = VecDeque::from([ClientboundPacket::MapChunkBulk {
            sky_light: true,
            chunks: (0..6).map(bulk_chunk).collect(),
        }]);
        let mut deferred_barrier_count = 0;
        let mut immediate = Vec::new();

        stage_inbound_packets(
            &mut deferred,
            &mut deferred_barrier_count,
            [ClientboundPacket::KeepAlive { id: 42 }],
            &mut immediate,
        );

        assert!(matches!(
            immediate.as_slice(),
            [ClientboundPacket::KeepAlive { id: 42 }]
        ));
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred_barrier_count, 0);
    }

    #[test]
    fn a_deferred_respawn_barrier_persists_across_scan_frames() {
        let mut deferred = VecDeque::from([
            ClientboundPacket::MapChunkBulk {
                sky_light: true,
                chunks: (0..6).map(bulk_chunk).collect(),
            },
            ClientboundPacket::Respawn {
                dimension: -1,
                difficulty: 2,
                gamemode: 0,
                level_type: "default".to_string(),
            },
        ]);
        let mut deferred_barrier_count = 1;
        let mut immediate = Vec::new();

        stage_inbound_packets(
            &mut deferred,
            &mut deferred_barrier_count,
            [ClientboundPacket::KeepAlive { id: 42 }],
            &mut immediate,
        );

        assert!(immediate.is_empty());
        assert!(matches!(
            deferred.back(),
            Some(ClientboundPacket::KeepAlive { id: 42 })
        ));
        assert_eq!(deferred_barrier_count, 1);
    }

    #[test]
    fn server_particle_packets_share_one_frame_budget() {
        let mut remaining = 5;

        assert_eq!(budget_server_particle_count(4_096, &mut remaining), Some(5));
        assert_eq!(remaining, 0);
        assert_eq!(budget_server_particle_count(1, &mut remaining), None);

        let mut remaining = 1;
        assert_eq!(budget_server_particle_count(0, &mut remaining), Some(0));
        assert_eq!(remaining, 0);
        assert_eq!(budget_server_particle_count(-1, &mut remaining), None);
    }

    #[test]
    fn multi_block_records_are_split_across_frame_budgets_in_fifo_order() {
        let records = (0..600)
            .map(|index| ((index % u16::MAX as usize) as u16, 1 << 4))
            .collect();
        let mut deferred = VecDeque::from([
            ClientboundPacket::MultiBlockChange {
                chunk_x: 2,
                chunk_z: 3,
                records,
            },
            ClientboundPacket::BlockChange {
                x: 9,
                y: 64,
                z: 9,
                block_state: 1 << 4,
            },
        ]);
        let mut deferred_barrier_count = 0;

        for expected in [256, 256] {
            let mut output = Vec::new();
            drain_deferred_packets(
                &mut deferred,
                &mut deferred_barrier_count,
                &mut output,
                WORLD_WORK_BUDGET,
            );
            assert_eq!(output.len(), 1);
            assert!(matches!(
                &output[0],
                ClientboundPacket::MultiBlockChange { records, .. }
                    if records.len() == expected
            ));
        }

        let mut final_output = Vec::new();
        drain_deferred_packets(
            &mut deferred,
            &mut deferred_barrier_count,
            &mut final_output,
            WORLD_WORK_BUDGET,
        );
        assert!(matches!(
            &final_output[0],
            ClientboundPacket::MultiBlockChange { records, .. } if records.len() == 88
        ));
        assert!(matches!(
            final_output[1],
            ClientboundPacket::BlockChange { .. }
        ));
        assert!(deferred.is_empty());
    }
}
