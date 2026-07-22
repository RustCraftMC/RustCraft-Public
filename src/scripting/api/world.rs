//! Bounded, read-only world snapshots for Lua mods.
//!
//! The API deliberately owns its data. It never retains a [`crate::world::World`]
//! reference, so a script cannot keep a borrow alive across a frame or reach
//! mutable world internals.

use mlua::{Lua, Table, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::scripting::permissions::{Permission, PermissionSet};

pub const READ_PERMISSION: &str = "client.read";

/// Furthest block coordinate a mod may inspect from the local player snapshot.
pub const MAX_BLOCK_QUERY_RADIUS: i32 = 15;
/// Maximum number of blocks retained in one published snapshot.
pub const MAX_BLOCK_SNAPSHOT_ENTRIES: usize = 32_768;
/// Furthest radius accepted by a single batched block query.
pub const MAX_BLOCK_BATCH_RADIUS: i32 = 8;
/// Maximum number of blocks returned by a single batched query.
pub const MAX_BLOCK_RESULTS: usize = 4_096;
/// Furthest entity distance retained and queryable from the local player.
pub const MAX_ENTITY_QUERY_RADIUS: f64 = 128.0;
/// Maximum number of entity summaries retained in one published snapshot.
pub const MAX_ENTITY_SNAPSHOT_ENTRIES: usize = 512;
/// Maximum number of entity summaries returned by one Lua call.
pub const MAX_ENTITY_RESULTS: usize = 128;

pub type BlockPosition = (i32, i32, i32);
pub type SharedWorldState = Rc<RefCell<WorldApiState>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeatherSnapshot {
    pub raining: bool,
    pub thundering: bool,
    pub rain_strength: f32,
    pub thunder_strength: f32,
}

impl Default for WeatherSnapshot {
    fn default() -> Self {
        Self {
            raining: false,
            thundering: false,
            rain_strength: 0.0,
            thunder_strength: 0.0,
        }
    }
}

/// The state id is the Minecraft 1.8 `(block_id << 4) | metadata` value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockSnapshot {
    pub state: u16,
    pub sky_light: u8,
    pub block_light: u8,
    pub biome: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub id: i32,
    pub kind: String,
    pub name: Option<String>,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub health: Option<f32>,
    pub max_health: Option<f32>,
}

impl Default for EntitySnapshot {
    fn default() -> Self {
        Self {
            id: 0,
            kind: "unknown".into(),
            name: None,
            position: [0.0; 3],
            velocity: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            health: None,
            max_health: None,
        }
    }
}

/// Owned data published by the client for one frame/tick.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldSnapshot {
    pub dimension_id: i32,
    pub dimension_name: String,
    pub game_time: i64,
    pub day_time: i64,
    pub weather: WeatherSnapshot,
    pub loaded_chunks: usize,
    pub player_position: [f64; 3],
    pub blocks: BTreeMap<BlockPosition, BlockSnapshot>,
    pub entities: Vec<EntitySnapshot>,
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self {
            dimension_id: 0,
            dimension_name: "overworld".into(),
            game_time: 0,
            day_time: 0,
            weather: WeatherSnapshot::default(),
            loaded_chunks: 0,
            player_position: [0.0; 3],
            blocks: BTreeMap::new(),
            entities: Vec::new(),
        }
    }
}

/// Compact owned copy used while constructing Lua summary tables.
///
/// Keeping this separate from [`WorldSnapshot`] prevents summary reads from cloning the retained
/// block and entity collections merely to release the shared-state borrow before touching Lua.
struct WorldSummarySnapshot {
    dimension_id: i32,
    dimension_name: String,
    game_time: i64,
    day_time: i64,
    weather: WeatherSnapshot,
    loaded_chunks: usize,
    player_position: [f64; 3],
}

impl From<&WorldSnapshot> for WorldSummarySnapshot {
    fn from(snapshot: &WorldSnapshot) -> Self {
        Self {
            dimension_id: snapshot.dimension_id,
            dimension_name: snapshot.dimension_name.clone(),
            game_time: snapshot.game_time,
            day_time: snapshot.day_time,
            weather: snapshot.weather,
            loaded_chunks: snapshot.loaded_chunks,
            player_position: snapshot.player_position,
        }
    }
}

#[derive(Clone, Copy)]
struct BlockQueryOptions {
    x: Option<i32>,
    y: Option<i32>,
    z: Option<i32>,
    radius: i32,
    limit: usize,
}

struct BlockQueryResult {
    blocks: Vec<(BlockPosition, BlockSnapshot)>,
    truncated: bool,
}

#[derive(Clone, Copy)]
struct EntityQueryOptions {
    radius: f64,
    limit: usize,
}

struct EntityQueryResult {
    entities: Vec<(EntitySnapshot, f64)>,
    truncated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WorldApiState {
    snapshot: Option<WorldSnapshot>,
}

impl WorldApiState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes an owned snapshot after enforcing all retention limits.
    pub fn set_snapshot(&mut self, mut snapshot: WorldSnapshot) {
        sanitize_snapshot_metadata(&mut snapshot);
        sanitize_snapshot_blocks(&mut snapshot);
        self.snapshot = Some(snapshot);
    }

    /// Updates tick-varying metadata while retaining the already-sanitized
    /// block volume. The client uses this when neither the player block nor the
    /// world generation changed since the previous logical tick.
    pub(crate) fn set_snapshot_reusing_blocks(&mut self, mut snapshot: WorldSnapshot) -> bool {
        let Some(mut previous) = self.snapshot.take() else {
            return false;
        };
        sanitize_snapshot_metadata(&mut snapshot);
        let same_player_block = player_block_position(previous.player_position)
            == player_block_position(snapshot.player_position);
        debug_assert!(
            same_player_block,
            "cached world blocks may only be reused inside the same player block"
        );
        if !same_player_block {
            self.snapshot = Some(previous);
            return false;
        }
        snapshot.blocks = std::mem::take(&mut previous.blocks);
        self.snapshot = Some(snapshot);
        true
    }

    /// Applies changed block entries while retaining the rest of the cached
    /// cube. When the player block is unchanged (patch path), entries are
    /// appended directly. When the player crossed a block boundary (shift
    /// path), stale entries outside the new range are removed before appending.
    pub(crate) fn set_snapshot_merging_blocks(&mut self, mut snapshot: WorldSnapshot) -> bool {
        let Some(mut previous) = self.snapshot.take() else {
            return false;
        };
        sanitize_snapshot_metadata(&mut snapshot);
        sanitize_snapshot_blocks(&mut snapshot);
        let same_player_block = player_block_position(previous.player_position)
            == player_block_position(snapshot.player_position);
        let mut blocks = std::mem::take(&mut previous.blocks);
        if !same_player_block {
            let player_block = player_block_position(snapshot.player_position);
            blocks.retain(|&(x, y, z), _| valid_block_coordinate(x, y, z, player_block));
        }
        blocks.append(&mut snapshot.blocks);
        snapshot.blocks = blocks;
        if !same_player_block && snapshot.blocks.len() > MAX_BLOCK_SNAPSHOT_ENTRIES {
            sanitize_snapshot_blocks(&mut snapshot);
        }
        self.snapshot = Some(snapshot);
        true
    }

    pub fn clear(&mut self) {
        self.snapshot = None;
    }

    pub fn snapshot(&self) -> Option<&WorldSnapshot> {
        self.snapshot.as_ref()
    }

    pub(crate) fn has_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }
}

fn sanitize_snapshot_metadata(snapshot: &mut WorldSnapshot) {
    sanitize_position(&mut snapshot.player_position);
    snapshot.dimension_name =
        bounded_text(std::mem::take(&mut snapshot.dimension_name), 64, "unknown");
    snapshot.weather.rain_strength = unit_finite(snapshot.weather.rain_strength);
    snapshot.weather.thunder_strength = unit_finite(snapshot.weather.thunder_strength);

    for entity in &mut snapshot.entities {
        entity.kind = bounded_text(std::mem::take(&mut entity.kind), 96, "unknown");
        entity.name = entity
            .name
            .take()
            .map(|name| bounded_text(name, 256, ""))
            .filter(|name| !name.is_empty());
        entity.yaw = finite_f32(entity.yaw);
        entity.pitch = finite_f32(entity.pitch);
        entity.health = entity.health.filter(|value| value.is_finite());
        entity.max_health = entity.max_health.filter(|value| value.is_finite());
    }
    snapshot.entities.retain(|entity| {
        entity.position.iter().all(|value| value.is_finite())
            && entity.velocity.iter().all(|value| value.is_finite())
            && distance_squared(entity.position, snapshot.player_position)
                <= MAX_ENTITY_QUERY_RADIUS * MAX_ENTITY_QUERY_RADIUS
    });
    snapshot.entities.sort_by(|left, right| {
        distance_squared(left.position, snapshot.player_position)
            .total_cmp(&distance_squared(right.position, snapshot.player_position))
            .then_with(|| left.id.cmp(&right.id))
    });
    snapshot.entities.truncate(MAX_ENTITY_SNAPSHOT_ENTRIES);
}

fn sanitize_snapshot_blocks(snapshot: &mut WorldSnapshot) {
    let player_block = player_block_position(snapshot.player_position);
    snapshot.blocks.retain(|&(x, y, z), block| {
        block.sky_light = block.sky_light.min(15);
        block.block_light = block.block_light.min(15);
        valid_block_coordinate(x, y, z, player_block)
    });
    if snapshot.blocks.len() > MAX_BLOCK_SNAPSHOT_ENTRIES {
        let mut blocks = std::mem::take(&mut snapshot.blocks)
            .into_iter()
            .collect::<Vec<_>>();
        blocks.sort_by_key(|entry| {
            let (x, y, z) = entry.0;
            (
                coordinate_distance(x, player_block.0)
                    .max(coordinate_distance(y, player_block.1))
                    .max(coordinate_distance(z, player_block.2)),
                x,
                y,
                z,
            )
        });
        snapshot.blocks = blocks
            .into_iter()
            .take(MAX_BLOCK_SNAPSHOT_ENTRIES)
            .collect();
    }
}

pub fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    state: SharedWorldState,
) -> mlua::Result<()> {
    if !permissions.contains(Permission::ClientRead) {
        return Ok(());
    }

    let world = lua.create_table()?;
    world.set("max_block_query_radius", MAX_BLOCK_QUERY_RADIUS)?;
    world.set("max_block_batch_radius", MAX_BLOCK_BATCH_RADIUS)?;
    world.set("max_block_results", MAX_BLOCK_RESULTS)?;
    world.set("max_entity_query_radius", MAX_ENTITY_QUERY_RADIUS)?;
    world.set("max_entity_results", MAX_ENTITY_RESULTS)?;

    let snapshot_state = state.clone();
    world.set(
        "snapshot",
        lua.create_function(move |lua, ()| {
            let summary = snapshot_state
                .borrow()
                .snapshot()
                .map(WorldSummarySnapshot::from);
            summary
                .map(|summary| summary_table(lua, &summary).map(Value::Table))
                .unwrap_or(Ok(Value::Nil))
        })?,
    )?;

    let dimension_state = state.clone();
    world.set(
        "dimension",
        lua.create_function(move |lua, ()| {
            let dimension = dimension_state
                .borrow()
                .snapshot()
                .map(|snapshot| (snapshot.dimension_id, snapshot.dimension_name.clone()));
            dimension
                .map(|(id, name)| dimension_table(lua, id, &name).map(Value::Table))
                .unwrap_or(Ok(Value::Nil))
        })?,
    )?;

    let time_state = state.clone();
    world.set(
        "time",
        lua.create_function(move |lua, ()| {
            let time = time_state
                .borrow()
                .snapshot()
                .map(|snapshot| (snapshot.game_time, snapshot.day_time));
            time.map(|(game, day)| time_table(lua, game, day).map(Value::Table))
                .unwrap_or(Ok(Value::Nil))
        })?,
    )?;

    let weather_state = state.clone();
    world.set(
        "weather",
        lua.create_function(move |lua, ()| {
            let weather = weather_state
                .borrow()
                .snapshot()
                .map(|snapshot| snapshot.weather);
            weather
                .map(|weather| weather_table(lua, &weather).map(Value::Table))
                .unwrap_or(Ok(Value::Nil))
        })?,
    )?;

    let chunk_state = state.clone();
    world.set(
        "loaded_chunk_count",
        lua.create_function(move |_, ()| {
            Ok(chunk_state
                .borrow()
                .snapshot()
                .map(|snapshot| snapshot.loaded_chunks))
        })?,
    )?;

    let block_state = state.clone();
    world.set(
        "get_block",
        lua.create_function(move |lua, (x, y, z): (i32, i32, i32)| {
            let block = {
                let state = block_state.borrow();
                let Some(snapshot) = state.snapshot() else {
                    return Ok(Value::Nil);
                };
                ensure_block_query_in_bounds(x, y, z, snapshot)?;
                snapshot.blocks.get(&(x, y, z)).copied()
            };
            block
                .map(|block| block_table(lua, x, y, z, block).map(Value::Table))
                .unwrap_or(Ok(Value::Nil))
        })?,
    )?;

    let blocks_state = state.clone();
    world.set(
        "get_blocks",
        lua.create_function(move |lua, options: Option<Table>| {
            let has_snapshot = { blocks_state.borrow().snapshot().is_some() };
            if !has_snapshot {
                return empty_results_table(lua);
            }
            let options = block_query_options(&options)?;
            let result = {
                let state = blocks_state.borrow();
                state
                    .snapshot()
                    .map(|snapshot| collect_nearby_blocks(snapshot, options))
                    .transpose()?
            };
            result
                .map(|result| blocks_result_table(lua, result))
                .unwrap_or_else(|| empty_results_table(lua))
        })?,
    )?;

    world.set(
        "get_entities",
        lua.create_function(move |lua, options: Option<Table>| {
            let has_snapshot = { state.borrow().snapshot().is_some() };
            if !has_snapshot {
                return empty_results_table(lua);
            }
            let options = entity_query_options(&options)?;
            let result = {
                let state = state.borrow();
                state
                    .snapshot()
                    .map(|snapshot| collect_nearby_entities(snapshot, options))
            };
            result
                .map(|result| entities_result_table(lua, result))
                .unwrap_or_else(|| empty_results_table(lua))
        })?,
    )?;

    game.set("world", world)
}

fn summary_table(lua: &Lua, snapshot: &WorldSummarySnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set(
        "dimension",
        dimension_table(lua, snapshot.dimension_id, &snapshot.dimension_name)?,
    )?;
    table.set(
        "time",
        time_table(lua, snapshot.game_time, snapshot.day_time)?,
    )?;
    table.set("weather", weather_table(lua, &snapshot.weather)?)?;
    table.set("loaded_chunks", snapshot.loaded_chunks)?;
    table.set(
        "player_position",
        vector_table(lua, snapshot.player_position)?,
    )?;
    table.set("block_query_radius", MAX_BLOCK_QUERY_RADIUS)?;
    table.set("entity_query_radius", MAX_ENTITY_QUERY_RADIUS)?;
    Ok(table)
}

fn dimension_table(lua: &Lua, id: i32, name: &str) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("id", id)?;
    table.set("name", name)?;
    Ok(table)
}

fn time_table(lua: &Lua, game_time: i64, day_time: i64) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("game", game_time)?;
    table.set("day", day_time)?;
    table.set("day_index", day_time.div_euclid(24_000))?;
    table.set("day_tick", day_time.rem_euclid(24_000))?;
    Ok(table)
}

fn weather_table(lua: &Lua, weather: &WeatherSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("raining", weather.raining)?;
    table.set("thundering", weather.thundering)?;
    table.set("rain_strength", weather.rain_strength)?;
    table.set("thunder_strength", weather.thunder_strength)?;
    Ok(table)
}

fn block_table(lua: &Lua, x: i32, y: i32, z: i32, block: BlockSnapshot) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("x", x)?;
    table.set("y", y)?;
    table.set("z", z)?;
    table.set("state", block.state)?;
    table.set("id", block.state >> 4)?;
    table.set("metadata", block.state & 0x0f)?;
    table.set("sky_light", block.sky_light)?;
    table.set("block_light", block.block_light)?;
    table.set("light", block.sky_light.max(block.block_light))?;
    table.set("biome", block.biome)?;
    Ok(table)
}

fn block_query_options(options: &Option<Table>) -> mlua::Result<BlockQueryOptions> {
    let radius = option_i32(options, "radius")?.unwrap_or(4);
    let limit = option_usize(options, "limit")?.unwrap_or(512);
    if !(0..=MAX_BLOCK_BATCH_RADIUS).contains(&radius) {
        return Err(mlua::Error::RuntimeError(format!(
            "block batch radius must be between 0 and {MAX_BLOCK_BATCH_RADIUS}"
        )));
    }
    if !(1..=MAX_BLOCK_RESULTS).contains(&limit) {
        return Err(mlua::Error::RuntimeError(format!(
            "block result limit must be between 1 and {MAX_BLOCK_RESULTS}"
        )));
    }
    Ok(BlockQueryOptions {
        x: option_i32(options, "x")?,
        y: option_i32(options, "y")?,
        z: option_i32(options, "z")?,
        radius,
        limit,
    })
}

fn collect_nearby_blocks(
    snapshot: &WorldSnapshot,
    options: BlockQueryOptions,
) -> mlua::Result<BlockQueryResult> {
    let player = player_block_position(snapshot.player_position);
    let center = (
        options.x.unwrap_or(player.0),
        options.y.unwrap_or(player.1),
        options.z.unwrap_or(player.2),
    );
    ensure_block_query_in_bounds(center.0, center.1, center.2, snapshot)?;
    if coordinate_distance(center.0, player.0) + i64::from(options.radius)
        > i64::from(MAX_BLOCK_QUERY_RADIUS)
        || coordinate_distance(center.1, player.1) + i64::from(options.radius)
            > i64::from(MAX_BLOCK_QUERY_RADIUS)
        || coordinate_distance(center.2, player.2) + i64::from(options.radius)
            > i64::from(MAX_BLOCK_QUERY_RADIUS)
    {
        return Err(mlua::Error::RuntimeError(format!(
            "block batch must remain within {MAX_BLOCK_QUERY_RADIUS} blocks of the player"
        )));
    }

    let mut blocks = Vec::with_capacity(options.limit.min(snapshot.blocks.len()));
    for (&(x, y, z), &block) in &snapshot.blocks {
        if coordinate_distance(x, center.0) > i64::from(options.radius)
            || coordinate_distance(y, center.1) > i64::from(options.radius)
            || coordinate_distance(z, center.2) > i64::from(options.radius)
        {
            continue;
        }
        if blocks.len() == options.limit {
            return Ok(BlockQueryResult {
                blocks,
                truncated: true,
            });
        }
        blocks.push(((x, y, z), block));
    }
    Ok(BlockQueryResult {
        blocks,
        truncated: false,
    })
}

fn blocks_result_table(lua: &Lua, result: BlockQueryResult) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let count = result.blocks.len();
    for (index, ((x, y, z), block)) in result.blocks.into_iter().enumerate() {
        table.raw_set(index + 1, block_table(lua, x, y, z, block)?)?;
    }
    table.set("count", count)?;
    table.set("truncated", result.truncated)?;
    Ok(table)
}

fn entity_query_options(options: &Option<Table>) -> mlua::Result<EntityQueryOptions> {
    let radius = option_f64(options, "radius")?.unwrap_or(32.0);
    let limit = option_usize(options, "limit")?.unwrap_or(64);
    if !radius.is_finite() || !(0.0..=MAX_ENTITY_QUERY_RADIUS).contains(&radius) {
        return Err(mlua::Error::RuntimeError(format!(
            "entity radius must be finite and between 0 and {MAX_ENTITY_QUERY_RADIUS}"
        )));
    }
    if !(1..=MAX_ENTITY_RESULTS).contains(&limit) {
        return Err(mlua::Error::RuntimeError(format!(
            "entity result limit must be between 1 and {MAX_ENTITY_RESULTS}"
        )));
    }
    Ok(EntityQueryOptions { radius, limit })
}

fn collect_nearby_entities(
    snapshot: &WorldSnapshot,
    options: EntityQueryOptions,
) -> EntityQueryResult {
    let radius_squared = options.radius * options.radius;
    let mut entities = Vec::with_capacity(options.limit.min(snapshot.entities.len()));
    for entity in &snapshot.entities {
        let distance_squared = distance_squared(entity.position, snapshot.player_position);
        if distance_squared > radius_squared {
            continue;
        }
        if entities.len() == options.limit {
            return EntityQueryResult {
                entities,
                truncated: true,
            };
        }
        entities.push((entity.clone(), distance_squared.sqrt()));
    }
    EntityQueryResult {
        entities,
        truncated: false,
    }
}

fn entities_result_table(lua: &Lua, result: EntityQueryResult) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let count = result.entities.len();
    for (index, (entity, distance)) in result.entities.into_iter().enumerate() {
        table.raw_set(index + 1, entity_table(lua, &entity, distance)?)?;
    }
    table.set("count", count)?;
    table.set("truncated", result.truncated)?;
    Ok(table)
}

fn entity_table(lua: &Lua, entity: &EntitySnapshot, distance: f64) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("id", entity.id)?;
    table.set("kind", entity.kind.as_str())?;
    table.set("name", entity.name.as_deref())?;
    table.set("position", vector_table(lua, entity.position)?)?;
    table.set("velocity", vector_table(lua, entity.velocity)?)?;
    table.set("distance", distance)?;
    table.set("yaw", entity.yaw)?;
    table.set("pitch", entity.pitch)?;
    table.set("on_ground", entity.on_ground)?;
    table.set("health", entity.health)?;
    table.set("max_health", entity.max_health)?;
    Ok(table)
}

fn vector_table(lua: &Lua, value: [f64; 3]) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("x", value[0])?;
    table.set("y", value[1])?;
    table.set("z", value[2])?;
    Ok(table)
}

fn empty_results_table(lua: &Lua) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("count", 0)?;
    table.set("truncated", false)?;
    Ok(table)
}

fn ensure_block_query_in_bounds(
    x: i32,
    y: i32,
    z: i32,
    snapshot: &WorldSnapshot,
) -> mlua::Result<()> {
    let player = player_block_position(snapshot.player_position);
    if valid_block_coordinate(x, y, z, player) {
        Ok(())
    } else {
        Err(mlua::Error::RuntimeError(format!(
            "block query must be within {MAX_BLOCK_QUERY_RADIUS} blocks of the player and y must be between 0 and 255"
        )))
    }
}

fn valid_block_coordinate(x: i32, y: i32, z: i32, player: BlockPosition) -> bool {
    (0..=255).contains(&y)
        && coordinate_distance(x, player.0) <= i64::from(MAX_BLOCK_QUERY_RADIUS)
        && coordinate_distance(y, player.1) <= i64::from(MAX_BLOCK_QUERY_RADIUS)
        && coordinate_distance(z, player.2) <= i64::from(MAX_BLOCK_QUERY_RADIUS)
}

fn coordinate_distance(left: i32, right: i32) -> i64 {
    (i64::from(left) - i64::from(right)).abs()
}

fn player_block_position(position: [f64; 3]) -> BlockPosition {
    (
        position[0].floor() as i32,
        position[1].floor() as i32,
        position[2].floor() as i32,
    )
}

fn distance_squared(left: [f64; 3], right: [f64; 3]) -> f64 {
    let x = left[0] - right[0];
    let y = left[1] - right[1];
    let z = left[2] - right[2];
    x * x + y * y + z * z
}

fn option_i32(options: &Option<Table>, key: &str) -> mlua::Result<Option<i32>> {
    options
        .as_ref()
        .map(|options| options.get::<Option<i32>>(key))
        .transpose()
        .map(Option::flatten)
}

fn option_usize(options: &Option<Table>, key: &str) -> mlua::Result<Option<usize>> {
    options
        .as_ref()
        .map(|options| options.get::<Option<usize>>(key))
        .transpose()
        .map(Option::flatten)
}

fn option_f64(options: &Option<Table>, key: &str) -> mlua::Result<Option<f64>> {
    options
        .as_ref()
        .map(|options| options.get::<Option<f64>>(key))
        .transpose()
        .map(Option::flatten)
}

fn sanitize_position(position: &mut [f64; 3]) {
    for value in position {
        if !value.is_finite() || value.abs() > 30_000_000.0 {
            *value = 0.0;
        }
    }
}

fn finite_f32(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn unit_finite(value: f32) -> f32 {
    finite_f32(value).clamp(0.0, 1.0)
}

fn bounded_text(value: String, max_chars: usize, fallback: &str) -> String {
    let value: String = value.chars().take(max_chars).collect();
    if value.is_empty() {
        fallback.into()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::permissions::PermissionPolicy;

    fn permissions(requested: &[Permission]) -> PermissionSet {
        PermissionSet::resolve("world-test", requested, &PermissionPolicy::default())
    }

    fn installed(snapshot: WorldSnapshot) -> (Lua, SharedWorldState) {
        let lua = Lua::new();
        let game = lua.create_table().unwrap();
        let state = Rc::new(RefCell::new(WorldApiState::new()));
        state.borrow_mut().set_snapshot(snapshot);
        install(
            &lua,
            &game,
            &permissions(&[Permission::ClientRead]),
            state.clone(),
        )
        .unwrap();
        lua.globals().set("game", game).unwrap();
        (lua, state)
    }

    #[test]
    fn world_module_requires_client_read() {
        let lua = Lua::new();
        let game = lua.create_table().unwrap();
        install(
            &lua,
            &game,
            &permissions(&[]),
            Rc::new(RefCell::new(WorldApiState::new())),
        )
        .unwrap();
        assert!(game.get::<Option<Table>>("world").unwrap().is_none());
    }

    #[test]
    fn advertised_block_query_cube_fits_the_retained_snapshot() {
        let side = usize::try_from(MAX_BLOCK_QUERY_RADIUS * 2 + 1).unwrap();
        assert_eq!(side, 31);
        assert!(side.pow(3) <= MAX_BLOCK_SNAPSHOT_ENTRIES);
    }

    #[test]
    fn exposes_owned_block_and_world_summary() {
        let mut snapshot = WorldSnapshot {
            dimension_id: -1,
            dimension_name: "the_nether".into(),
            game_time: 48_123,
            day_time: 25_000,
            loaded_chunks: 17,
            player_position: [10.5, 64.0, -3.5],
            weather: WeatherSnapshot {
                raining: true,
                rain_strength: 0.75,
                ..WeatherSnapshot::default()
            },
            ..WorldSnapshot::default()
        };
        snapshot.blocks.insert(
            (11, 64, -3),
            BlockSnapshot {
                state: (5 << 4) | 2,
                sky_light: 12,
                block_light: 3,
                biome: 8,
            },
        );
        let (lua, _state) = installed(snapshot);
        lua.load(
            r#"
                local summary = game.world.snapshot()
                assert(summary.dimension.id == -1)
                assert(summary.dimension.name == "the_nether")
                assert(summary.time.game == 48123)
                assert(summary.time.day_index == 1)
                assert(summary.time.day_tick == 1000)
                assert(summary.weather.raining == true)
                assert(summary.loaded_chunks == 17)
                local block = game.world.get_block(11, 64, -3)
                assert(block.state == 82)
                assert(block.id == 5 and block.metadata == 2)
                assert(block.sky_light == 12 and block.block_light == 3)
                assert(block.light == 12 and block.biome == 8)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn lua_reads_release_the_shared_borrow_and_observe_replacement_snapshots() {
        let mut snapshot = WorldSnapshot {
            player_position: [0.0, 64.0, 0.0],
            entities: vec![EntitySnapshot {
                id: 7,
                position: [0.0, 64.0, 0.0],
                ..Default::default()
            }],
            ..Default::default()
        };
        snapshot.blocks.insert(
            (0, 64, 0),
            BlockSnapshot {
                state: 1 << 4,
                ..Default::default()
            },
        );
        let (lua, state) = installed(snapshot);

        lua.load(
            r#"
                assert(game.world.snapshot() ~= nil)
                assert(game.world.dimension() ~= nil)
                assert(game.world.time() ~= nil)
                assert(game.world.weather() ~= nil)
                assert(game.world.get_block(0, 64, 0).id == 1)
                assert(game.world.get_blocks({ radius = 0 }).count == 1)
                assert(game.world.get_entities({ radius = 0 }).count == 1)
            "#,
        )
        .exec()
        .unwrap();
        assert!(state.try_borrow_mut().is_ok());

        let mut replacement = WorldSnapshot {
            player_position: [0.0, 64.0, 0.0],
            ..Default::default()
        };
        replacement.blocks.insert(
            (0, 64, 0),
            BlockSnapshot {
                state: 2 << 4,
                ..Default::default()
            },
        );
        state.borrow_mut().set_snapshot(replacement);
        let block_id: u16 = lua
            .load("return game.world.get_block(0, 64, 0).id")
            .eval()
            .unwrap();
        assert_eq!(block_id, 2);
    }

    #[test]
    fn rejects_out_of_bounds_queries_and_drops_out_of_bounds_snapshot_data() {
        let mut snapshot = WorldSnapshot {
            player_position: [0.0, 64.0, 0.0],
            ..WorldSnapshot::default()
        };
        snapshot
            .blocks
            .insert((15, 64, 0), BlockSnapshot::default());
        snapshot
            .blocks
            .insert((16, 64, 0), BlockSnapshot::default());
        snapshot.entities.push(EntitySnapshot {
            id: 1,
            position: [127.0, 64.0, 0.0],
            ..EntitySnapshot::default()
        });
        snapshot.entities.push(EntitySnapshot {
            id: 2,
            position: [129.0, 64.0, 0.0],
            ..EntitySnapshot::default()
        });
        let (lua, state) = installed(snapshot);
        assert_eq!(state.borrow().snapshot().unwrap().blocks.len(), 1);
        assert_eq!(state.borrow().snapshot().unwrap().entities.len(), 1);
        lua.load(
            r#"
                assert(game.world.get_block(15, 64, 0) ~= nil)
                assert(not pcall(game.world.get_block, 16, 64, 0))
                assert(not pcall(game.world.get_block, 0, 300, 0))
                assert(not pcall(game.world.get_blocks, { radius = 9 }))
                assert(not pcall(game.world.get_entities, { radius = 129 }))
                assert(not pcall(game.world.get_entities, { limit = 129 }))
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn entity_results_are_distance_sorted_and_limited() {
        let snapshot = WorldSnapshot {
            player_position: [0.0, 64.0, 0.0],
            entities: vec![
                EntitySnapshot {
                    id: 20,
                    kind: "zombie".into(),
                    position: [10.0, 64.0, 0.0],
                    ..EntitySnapshot::default()
                },
                EntitySnapshot {
                    id: 10,
                    kind: "pig".into(),
                    position: [2.0, 64.0, 0.0],
                    health: Some(10.0),
                    max_health: Some(10.0),
                    ..EntitySnapshot::default()
                },
            ],
            ..WorldSnapshot::default()
        };
        let (lua, _state) = installed(snapshot);
        lua.load(
            r#"
                local entities = game.world.get_entities({ radius = 32, limit = 1 })
                assert(entities.count == 1 and entities.truncated == true)
                assert(entities[1].id == 10 and entities[1].kind == "pig")
                assert(entities[1].distance == 2)
                assert(entities[1].health == 10)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn metadata_updates_reuse_the_existing_block_volume() {
        let mut initial = WorldSnapshot {
            game_time: 1,
            day_time: 2,
            player_position: [0.25, 64.0, 0.25],
            entities: vec![EntitySnapshot {
                id: 1,
                position: [0.0, 64.0, 0.0],
                ..EntitySnapshot::default()
            }],
            ..WorldSnapshot::default()
        };
        initial.blocks.insert(
            (0, 64, 0),
            BlockSnapshot {
                state: 1 << 4,
                ..BlockSnapshot::default()
            },
        );

        let mut state = WorldApiState::new();
        state.set_snapshot(initial);
        let mut next = WorldSnapshot {
            game_time: 3,
            day_time: 4,
            player_position: [0.75, 64.9, 0.75],
            entities: vec![EntitySnapshot {
                id: 2,
                position: [1.0, 64.0, 0.0],
                ..EntitySnapshot::default()
            }],
            ..WorldSnapshot::default()
        };
        next.blocks.insert(
            (1, 64, 0),
            BlockSnapshot {
                state: 2 << 4,
                ..BlockSnapshot::default()
            },
        );
        assert!(state.set_snapshot_reusing_blocks(next));

        let snapshot = state.snapshot().unwrap();
        assert_eq!((snapshot.game_time, snapshot.day_time), (3, 4));
        assert_eq!(snapshot.entities[0].id, 2);
        assert_eq!(snapshot.blocks.len(), 1);
        assert_eq!(snapshot.blocks[&(0, 64, 0)].state, 1 << 4);
        assert!(!snapshot.blocks.contains_key(&(1, 64, 0)));
    }

    #[test]
    fn exact_block_patches_replace_only_changed_entries() {
        let mut initial = WorldSnapshot {
            player_position: [0.25, 64.0, 0.25],
            ..WorldSnapshot::default()
        };
        initial.blocks.insert(
            (0, 64, 0),
            BlockSnapshot {
                state: 1 << 4,
                ..BlockSnapshot::default()
            },
        );
        initial.blocks.insert(
            (1, 64, 0),
            BlockSnapshot {
                state: 2 << 4,
                ..BlockSnapshot::default()
            },
        );

        let mut state = WorldApiState::new();
        state.set_snapshot(initial);
        let mut patch = WorldSnapshot {
            game_time: 5,
            player_position: [0.75, 64.5, 0.75],
            ..WorldSnapshot::default()
        };
        patch.blocks.insert(
            (1, 64, 0),
            BlockSnapshot {
                state: 3 << 4,
                ..BlockSnapshot::default()
            },
        );
        assert!(state.set_snapshot_merging_blocks(patch));

        let snapshot = state.snapshot().unwrap();
        assert_eq!(snapshot.game_time, 5);
        assert_eq!(snapshot.blocks.len(), 2);
        assert_eq!(snapshot.blocks[&(0, 64, 0)].state, 1 << 4);
        assert_eq!(snapshot.blocks[&(1, 64, 0)].state, 3 << 4);
    }

    #[test]
    fn shifted_block_updates_match_full_snapshot_replacement() {
        fn cube(center: (i32, i32, i32)) -> BTreeMap<BlockPosition, BlockSnapshot> {
            let radius = MAX_BLOCK_QUERY_RADIUS;
            let mut blocks = BTreeMap::new();
            for x in (center.0 - radius)..=(center.0 + radius) {
                for y in (center.1 - radius).max(0)..=(center.1 + radius).min(255) {
                    for z in (center.2 - radius)..=(center.2 + radius) {
                        blocks.insert(
                            (x, y, z),
                            BlockSnapshot {
                                state: ((x.wrapping_mul(31)
                                    ^ y.wrapping_mul(17)
                                    ^ z.wrapping_mul(13))
                                    & 0xffff) as u16,
                                sky_light: 15,
                                block_light: 7,
                                biome: 1,
                            },
                        );
                    }
                }
            }
            blocks
        }

        let mut center = (0, 64, 0);
        let mut expected = cube(center);
        let mut state = WorldApiState::new();
        state.set_snapshot(WorldSnapshot {
            player_position: [center.0 as f64, center.1 as f64, center.2 as f64],
            blocks: expected.clone(),
            ..WorldSnapshot::default()
        });

        for next_center in [(1, 65, 1), (40, 64, -20), (40, 0, -20), (40, 255, -20)] {
            let next_expected = cube(next_center);
            let entering = next_expected
                .iter()
                .filter(|(position, _)| !expected.contains_key(position))
                .map(|(&position, &block)| (position, block))
                .collect();
            assert!(state.set_snapshot_merging_blocks(WorldSnapshot {
                player_position: [
                    next_center.0 as f64,
                    next_center.1 as f64,
                    next_center.2 as f64,
                ],
                blocks: entering,
                ..WorldSnapshot::default()
            }));
            assert_eq!(state.snapshot().unwrap().blocks, next_expected);
            center = next_center;
            expected = next_expected;
        }
        assert_eq!(center, (40, 255, -20));
    }

    #[test]
    fn partial_block_updates_fail_closed_without_a_cached_snapshot() {
        let mut state = WorldApiState::new();
        let mut patch = WorldSnapshot::default();
        patch.blocks.insert((0, 64, 0), BlockSnapshot::default());

        assert!(!state.set_snapshot_merging_blocks(patch));
        assert!(state.snapshot().is_none());
    }
}
