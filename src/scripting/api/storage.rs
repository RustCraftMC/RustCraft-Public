//! Permission-scoped, persistent JSON storage for one mod.
//!
//! Lua never receives a filesystem path or file handle. Every runtime is confined to the fixed
//! `data/storage.json` file below its own mod root. The store is deliberately read before every
//! operation so a freshly loaded runtime and the runtime it replaces cannot overwrite unrelated
//! keys using stale in-memory snapshots.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use mlua::{Lua, LuaSerdeExt, Table, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::scripting::errors::{ScriptError, ScriptResult};
use crate::scripting::permissions::{Permission, PermissionSet};

pub const READ_PERMISSION: &str = "storage.read";
pub const WRITE_PERMISSION: &str = "storage.write";

pub const MAX_STORAGE_BYTES: u64 = 512 * 1024;
pub const MAX_STORAGE_ENTRIES: usize = 512;
pub const MAX_STORAGE_KEY_BYTES: usize = 128;
pub const MAX_STORAGE_VALUE_DEPTH: usize = 16;
pub const MAX_STORAGE_VALUE_NODES: usize = 4_096;
pub const MAX_STORAGE_TOTAL_NODES: usize = 16_384;
pub const MAX_STORAGE_CONTAINER_ENTRIES: usize = 1_024;
pub const MAX_STORAGE_STRING_BYTES: usize = 64 * 1024;

const STORAGE_FILE: &str = "storage.json";
const STORAGE_FILE_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub(crate) struct ModDataDirectory {
    root: PathBuf,
}

impl ModDataDirectory {
    pub(crate) fn new(mod_root: &Path) -> ScriptResult<Self> {
        let root = mod_root.canonicalize()?;
        if !root.is_dir() {
            return Err(ScriptError::InvalidPath(root));
        }
        Ok(Self { root })
    }

    pub(crate) fn read(&self, file_name: &str, max_bytes: u64) -> ScriptResult<Option<Vec<u8>>> {
        let Some(data_dir) = self.data_dir(false)? else {
            return Ok(None);
        };
        recover_pending_write(&data_dir, file_name)?;
        let path = checked_child(&data_dir, file_name)?;
        if !checked_existing_file(&data_dir, &path)? {
            return Ok(None);
        }

        let canonical = path.canonicalize()?;
        if canonical.metadata()?.len() > max_bytes {
            return Err(data_configuration(format!(
                "'{file_name}' exceeds the {max_bytes}-byte storage limit"
            )));
        }
        let bytes = fs::read(canonical)?;
        // Recheck after reading in case a file was replaced or extended after the metadata query.
        if bytes.len() as u64 > max_bytes {
            return Err(data_configuration(format!(
                "'{file_name}' exceeds the {max_bytes}-byte storage limit"
            )));
        }
        Ok(Some(bytes))
    }

    pub(crate) fn write(
        &self,
        file_name: &str,
        contents: &[u8],
        max_bytes: u64,
    ) -> ScriptResult<()> {
        if contents.len() as u64 > max_bytes {
            return Err(data_configuration(format!(
                "'{file_name}' exceeds the {max_bytes}-byte storage limit"
            )));
        }
        let data_dir = self
            .data_dir(true)?
            .expect("creating a mod data directory must return its path");
        recover_pending_write(&data_dir, file_name)?;
        let path = checked_child(&data_dir, file_name)?;
        let target_exists = checked_existing_file(&data_dir, &path)?;
        let temporary = checked_child(&data_dir, &format!(".{file_name}.tmp"))?;
        let backup = checked_child(&data_dir, &format!(".{file_name}.bak"))?;
        remove_checked_file(&data_dir, &temporary)?;
        remove_checked_file(&data_dir, &backup)?;

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        drop(file);

        if target_exists {
            fs::rename(&path, &backup)?;
        }
        if let Err(error) = fs::rename(&temporary, &path) {
            if target_exists {
                let _ = fs::rename(&backup, &path);
            }
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        if target_exists {
            // The new target is already committed. A stale backup is harmless and will be
            // removed by recovery before the next operation, so cleanup must not turn a
            // successful write into an apparent failure.
            let _ = fs::remove_file(backup);
        }
        Ok(())
    }

    fn data_dir(&self, create: bool) -> ScriptResult<Option<PathBuf>> {
        let path = self.root.join("data");
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(ScriptError::InvalidPath(path));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                fs::create_dir(&path)?;
            }
            Err(error) => return Err(error.into()),
        }
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(&self.root) || !canonical.is_dir() {
            return Err(ScriptError::InvalidPath(path));
        }
        Ok(Some(canonical))
    }
}

fn checked_existing_file(data_dir: &Path, path: &Path) -> ScriptResult<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ScriptError::InvalidPath(path.to_path_buf()));
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(data_dir) {
        return Err(ScriptError::InvalidPath(path.to_path_buf()));
    }
    Ok(true)
}

fn remove_checked_file(data_dir: &Path, path: &Path) -> ScriptResult<()> {
    if checked_existing_file(data_dir, path)? {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn recover_pending_write(data_dir: &Path, file_name: &str) -> ScriptResult<()> {
    let target = checked_child(data_dir, file_name)?;
    let temporary = checked_child(data_dir, &format!(".{file_name}.tmp"))?;
    let backup = checked_child(data_dir, &format!(".{file_name}.bak"))?;
    let target_exists = checked_existing_file(data_dir, &target)?;
    let temporary_exists = checked_existing_file(data_dir, &temporary)?;
    let backup_exists = checked_existing_file(data_dir, &backup)?;

    if target_exists {
        if temporary_exists {
            fs::remove_file(&temporary)?;
        }
        if backup_exists {
            fs::remove_file(&backup)?;
        }
    } else if temporary_exists {
        fs::rename(&temporary, &target)?;
        if backup_exists {
            fs::remove_file(&backup)?;
        }
    } else if backup_exists {
        fs::rename(&backup, &target)?;
    }
    Ok(())
}

fn checked_child(data_dir: &Path, file_name: &str) -> ScriptResult<PathBuf> {
    let relative = Path::new(file_name);
    let valid = !relative.as_os_str().is_empty()
        && !relative.is_absolute()
        && relative.components().count() == 1
        && relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        return Err(ScriptError::InvalidPath(relative.to_path_buf()));
    }
    Ok(data_dir.join(relative))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StorageDocument {
    version: u32,
    values: BTreeMap<String, JsonValue>,
}

impl Default for StorageDocument {
    fn default() -> Self {
        Self {
            version: STORAGE_FILE_VERSION,
            values: BTreeMap::new(),
        }
    }
}

/// Stateless per-mod storage boundary retained by Lua closures.
///
/// There is intentionally no cached document: every operation observes the last committed file,
/// including operations made by a replacement runtime during transactional hot reload.
#[derive(Clone, Debug)]
pub(crate) struct StorageApiState {
    directory: ModDataDirectory,
}

impl StorageApiState {
    pub(crate) fn open(mod_root: &Path) -> ScriptResult<Self> {
        Ok(Self {
            directory: ModDataDirectory::new(mod_root)?,
        })
    }

    pub(crate) fn validate(&self) -> ScriptResult<()> {
        self.read_document().map(drop)
    }

    fn read_document(&self) -> ScriptResult<StorageDocument> {
        let Some(bytes) = self.directory.read(STORAGE_FILE, MAX_STORAGE_BYTES)? else {
            return Ok(StorageDocument::default());
        };
        let document: StorageDocument = serde_json::from_slice(&bytes).map_err(|error| {
            storage_configuration(format!(
                "'{STORAGE_FILE}' is not valid storage JSON: {error}"
            ))
        })?;
        validate_document(&document)?;
        Ok(document)
    }

    fn write_document(&self, document: &StorageDocument) -> ScriptResult<()> {
        validate_document(document)?;
        let mut output = LimitedJsonBuffer::new(MAX_STORAGE_BYTES as usize);
        serde_json::to_writer(&mut output, document).map_err(|error| {
            storage_configuration(format!("could not encode '{STORAGE_FILE}': {error}"))
        })?;
        let bytes = output.into_bytes();
        self.directory
            .write(STORAGE_FILE, &bytes, MAX_STORAGE_BYTES)
    }

    fn get(&self, key: &str) -> ScriptResult<Option<JsonValue>> {
        validate_storage_key(key)?;
        Ok(self.read_document()?.values.get(key).cloned())
    }

    fn contains(&self, key: &str) -> ScriptResult<bool> {
        validate_storage_key(key)?;
        Ok(self.read_document()?.values.contains_key(key))
    }

    fn values(&self) -> ScriptResult<BTreeMap<String, JsonValue>> {
        Ok(self.read_document()?.values)
    }

    fn keys(&self) -> ScriptResult<Vec<String>> {
        Ok(self.read_document()?.values.into_keys().collect())
    }

    fn len(&self) -> ScriptResult<usize> {
        Ok(self.read_document()?.values.len())
    }

    fn set(&self, key: String, value: JsonValue) -> ScriptResult<()> {
        validate_storage_key(&key)?;
        let mut value_nodes = 0usize;
        validate_json_value(&value, 0, &mut value_nodes, MAX_STORAGE_VALUE_NODES)?;

        let mut document = self.read_document()?;
        if !document.values.contains_key(&key) && document.values.len() == MAX_STORAGE_ENTRIES {
            return Err(storage_configuration(format!(
                "storage may contain at most {MAX_STORAGE_ENTRIES} keys"
            )));
        }
        if document.values.get(&key) == Some(&value) {
            return Ok(());
        }
        document.values.insert(key, value);
        self.write_document(&document)
    }

    fn delete(&self, key: &str) -> ScriptResult<bool> {
        validate_storage_key(key)?;
        let mut document = self.read_document()?;
        let removed = document.values.remove(key).is_some();
        if removed {
            self.write_document(&document)?;
        }
        Ok(removed)
    }

    fn clear(&self) -> ScriptResult<usize> {
        let mut document = self.read_document()?;
        let removed = document.values.len();
        if removed != 0 {
            document.values.clear();
            self.write_document(&document)?;
        }
        Ok(removed)
    }
}

pub(crate) fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    state: StorageApiState,
) -> mlua::Result<()> {
    let can_read = permissions.contains(Permission::StorageRead);
    let can_write = permissions.contains(Permission::StorageWrite);
    if !can_read && !can_write {
        return Ok(());
    }

    let storage = lua.create_table()?;
    storage.set("version", STORAGE_FILE_VERSION)?;
    storage.set("max_bytes", MAX_STORAGE_BYTES)?;
    storage.set("max_entries", MAX_STORAGE_ENTRIES)?;
    storage.set("max_key_bytes", MAX_STORAGE_KEY_BYTES)?;
    storage.set("max_value_depth", MAX_STORAGE_VALUE_DEPTH)?;
    storage.set("max_value_nodes", MAX_STORAGE_VALUE_NODES)?;
    storage.set("max_total_nodes", MAX_STORAGE_TOTAL_NODES)?;
    storage.set("max_container_entries", MAX_STORAGE_CONTAINER_ENTRIES)?;
    storage.set("max_string_bytes", MAX_STORAGE_STRING_BYTES)?;
    storage.set("null", lua.null())?;

    if can_read {
        let get_state = state.clone();
        storage.set(
            "get",
            lua.create_function(move |lua, key: String| {
                match get_state.get(&key).map_err(lua_storage_error)? {
                    Some(value) => lua.to_value(&value),
                    None => Ok(Value::Nil),
                }
            })?,
        )?;

        let contains_state = state.clone();
        storage.set(
            "has",
            lua.create_function(move |_, key: String| {
                contains_state.contains(&key).map_err(lua_storage_error)
            })?,
        )?;

        let keys_state = state.clone();
        storage.set(
            "keys",
            lua.create_function(move |lua, ()| {
                let keys = keys_state.keys().map_err(lua_storage_error)?;
                lua.create_sequence_from(keys)
            })?,
        )?;

        let values_state = state.clone();
        storage.set(
            "all",
            lua.create_function(move |lua, ()| {
                let values = values_state.values().map_err(lua_storage_error)?;
                lua.to_value(&values)
            })?,
        )?;

        let len_state = state.clone();
        storage.set(
            "len",
            lua.create_function(move |_, ()| len_state.len().map_err(lua_storage_error))?,
        )?;
    }

    if can_write {
        let set_state = state.clone();
        storage.set(
            "set",
            lua.create_function(move |lua, (key, value): (String, Value)| {
                let value = lua_value_to_json(lua, value)?;
                set_state.set(key, value).map_err(lua_storage_error)
            })?,
        )?;

        let delete_state = state.clone();
        storage.set(
            "delete",
            lua.create_function(move |_, key: String| {
                delete_state.delete(&key).map_err(lua_storage_error)
            })?,
        )?;

        storage.set(
            "clear",
            lua.create_function(move |_, ()| state.clear().map_err(lua_storage_error))?,
        )?;

        storage.set(
            "array",
            lua.create_function(move |lua, values: Option<Table>| {
                let values = values.unwrap_or(lua.create_table()?);
                values.set_metatable(Some(lua.array_metatable()))?;
                Ok(values)
            })?,
        )?;
    }

    game.set("storage", storage)
}

fn validate_document(document: &StorageDocument) -> ScriptResult<()> {
    if document.version != STORAGE_FILE_VERSION {
        return Err(storage_configuration(format!(
            "unsupported storage version {} (expected {STORAGE_FILE_VERSION})",
            document.version
        )));
    }
    if document.values.len() > MAX_STORAGE_ENTRIES {
        return Err(storage_configuration(format!(
            "storage may contain at most {MAX_STORAGE_ENTRIES} keys"
        )));
    }

    let mut total_nodes = 0usize;
    for (key, value) in &document.values {
        validate_storage_key(key)?;
        let mut value_nodes = 0usize;
        validate_json_value(value, 0, &mut value_nodes, MAX_STORAGE_VALUE_NODES)?;
        validate_json_value(value, 0, &mut total_nodes, MAX_STORAGE_TOTAL_NODES)?;
    }
    Ok(())
}

fn validate_storage_key(key: &str) -> ScriptResult<()> {
    if key.is_empty() || key.len() > MAX_STORAGE_KEY_BYTES {
        return Err(storage_configuration(format!(
            "storage keys must contain between 1 and {MAX_STORAGE_KEY_BYTES} UTF-8 bytes"
        )));
    }
    if key.chars().any(char::is_control) {
        return Err(storage_configuration(
            "storage keys may not contain control characters",
        ));
    }
    Ok(())
}

fn validate_json_member_key(key: &str) -> ScriptResult<()> {
    if key.len() > MAX_STORAGE_KEY_BYTES || key.chars().any(char::is_control) {
        return Err(storage_configuration(format!(
            "JSON object keys must be at most {MAX_STORAGE_KEY_BYTES} UTF-8 bytes and contain no control characters"
        )));
    }
    Ok(())
}

fn validate_json_value(
    value: &JsonValue,
    depth: usize,
    nodes: &mut usize,
    node_limit: usize,
) -> ScriptResult<()> {
    if depth > MAX_STORAGE_VALUE_DEPTH {
        return Err(storage_configuration(format!(
            "stored values may be nested at most {MAX_STORAGE_VALUE_DEPTH} levels"
        )));
    }
    *nodes = nodes.saturating_add(1);
    if *nodes > node_limit {
        return Err(storage_configuration(format!(
            "stored values exceed the {node_limit}-node limit"
        )));
    }

    match value {
        JsonValue::String(value) if value.len() > MAX_STORAGE_STRING_BYTES => {
            Err(storage_configuration(format!(
                "stored strings may contain at most {MAX_STORAGE_STRING_BYTES} UTF-8 bytes"
            )))
        }
        JsonValue::Array(values) => {
            if values.len() > MAX_STORAGE_CONTAINER_ENTRIES {
                return Err(storage_configuration(format!(
                    "stored arrays may contain at most {MAX_STORAGE_CONTAINER_ENTRIES} elements"
                )));
            }
            for value in values {
                validate_json_value(value, depth + 1, nodes, node_limit)?;
            }
            Ok(())
        }
        JsonValue::Object(values) => {
            if values.len() > MAX_STORAGE_CONTAINER_ENTRIES {
                return Err(storage_configuration(format!(
                    "stored objects may contain at most {MAX_STORAGE_CONTAINER_ENTRIES} members"
                )));
            }
            for (key, value) in values {
                validate_json_member_key(key)?;
                validate_json_value(value, depth + 1, nodes, node_limit)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

struct LuaConversionBudget {
    nodes: usize,
    serialized_bytes: usize,
    active_tables: HashSet<usize>,
}

fn lua_value_to_json(lua: &Lua, value: Value) -> mlua::Result<JsonValue> {
    let mut budget = LuaConversionBudget {
        nodes: 0,
        serialized_bytes: 0,
        active_tables: HashSet::new(),
    };
    convert_lua_value(lua, value, 0, &mut budget)
}

fn convert_lua_value(
    lua: &Lua,
    value: Value,
    depth: usize,
    budget: &mut LuaConversionBudget,
) -> mlua::Result<JsonValue> {
    if depth > MAX_STORAGE_VALUE_DEPTH {
        return Err(storage_runtime_error(format!(
            "stored values may be nested at most {MAX_STORAGE_VALUE_DEPTH} levels"
        )));
    }
    budget.nodes = budget.nodes.saturating_add(1);
    if budget.nodes > MAX_STORAGE_VALUE_NODES {
        return Err(storage_runtime_error(format!(
            "a stored value may contain at most {MAX_STORAGE_VALUE_NODES} nodes"
        )));
    }

    match value {
        Value::Nil => {
            charge_serialized_bytes(budget, 4)?;
            Ok(JsonValue::Null)
        }
        Value::LightUserData(value) if value.0.is_null() => {
            charge_serialized_bytes(budget, 4)?;
            Ok(JsonValue::Null)
        }
        Value::Boolean(value) => {
            charge_serialized_bytes(budget, if value { 4 } else { 5 })?;
            Ok(JsonValue::Bool(value))
        }
        Value::Integer(value) => {
            charge_serialized_bytes(budget, value.to_string().len())?;
            Ok(JsonValue::from(value))
        }
        Value::Number(value) => {
            let value = serde_json::Number::from_f64(value)
                .ok_or_else(|| storage_runtime_error("stored numbers must be finite"))?;
            charge_serialized_bytes(budget, value.to_string().len())?;
            Ok(JsonValue::Number(value))
        }
        Value::String(value) => {
            let value = value.to_str()?;
            if value.len() > MAX_STORAGE_STRING_BYTES {
                return Err(storage_runtime_error(format!(
                    "stored strings may contain at most {MAX_STORAGE_STRING_BYTES} UTF-8 bytes"
                )));
            }
            charge_serialized_bytes(budget, json_string_size(&value))?;
            Ok(JsonValue::String(value.to_string()))
        }
        Value::Table(table) => convert_lua_table(lua, table, depth, budget),
        unsupported => Err(storage_runtime_error(format!(
            "values of type '{}' cannot be stored; use nil, booleans, finite numbers, UTF-8 strings, and tables",
            unsupported.type_name()
        ))),
    }
}

fn convert_lua_table(
    lua: &Lua,
    table: Table,
    depth: usize,
    budget: &mut LuaConversionBudget,
) -> mlua::Result<JsonValue> {
    let pointer = table.to_pointer() as usize;
    if !budget.active_tables.insert(pointer) {
        return Err(storage_runtime_error(
            "cyclic Lua tables cannot be stored as JSON",
        ));
    }

    let result = convert_lua_table_inner(lua, &table, depth, budget);
    budget.active_tables.remove(&pointer);
    result
}

fn convert_lua_table_inner(
    lua: &Lua,
    table: &Table,
    depth: usize,
    budget: &mut LuaConversionBudget,
) -> mlua::Result<JsonValue> {
    let array_metatable = lua.array_metatable();
    let marked_array = table
        .metatable()
        .is_some_and(|metatable| metatable.to_pointer() == array_metatable.to_pointer());
    let mut entries = Vec::new();
    for pair in table.clone().pairs::<Value, Value>() {
        if entries.len() == MAX_STORAGE_CONTAINER_ENTRIES {
            return Err(storage_runtime_error(format!(
                "stored tables may contain at most {MAX_STORAGE_CONTAINER_ENTRIES} entries"
            )));
        }
        entries.push(pair?);
    }

    let integer_keys = entries
        .iter()
        .all(|(key, _)| matches!(key, Value::Integer(_)));
    let string_keys = entries
        .iter()
        .all(|(key, _)| matches!(key, Value::String(_)));
    if marked_array || (!entries.is_empty() && integer_keys) {
        charge_serialized_bytes(budget, 2 + entries.len().saturating_sub(1))?;
        let mut indexed = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let Value::Integer(index) = key else {
                return Err(storage_runtime_error(
                    "JSON arrays may contain only positive, contiguous integer keys",
                ));
            };
            let index = usize::try_from(index).map_err(|_| {
                storage_runtime_error(
                    "JSON arrays may contain only positive, contiguous integer keys",
                )
            })?;
            if index == 0 {
                return Err(storage_runtime_error(
                    "JSON arrays may contain only positive, contiguous integer keys",
                ));
            }
            indexed.push((index, value));
        }
        indexed.sort_by_key(|(index, _)| *index);
        let mut array = Vec::with_capacity(indexed.len());
        for (position, (index, value)) in indexed.into_iter().enumerate() {
            if index != position + 1 {
                return Err(storage_runtime_error("JSON arrays may not contain holes"));
            }
            array.push(convert_lua_value(lua, value, depth + 1, budget)?);
        }
        return Ok(JsonValue::Array(array));
    }

    if entries.is_empty() || string_keys {
        charge_serialized_bytes(budget, 2 + entries.len().saturating_sub(1))?;
        let mut object = serde_json::Map::with_capacity(entries.len());
        for (key, value) in entries {
            let Value::String(key) = key else {
                unreachable!("only empty tables and string-keyed tables reach object conversion")
            };
            let key = key.to_str()?;
            validate_json_member_key(&key).map_err(lua_storage_error)?;
            charge_serialized_bytes(budget, json_string_size(&key).saturating_add(1))?;
            object.insert(
                key.to_string(),
                convert_lua_value(lua, value, depth + 1, budget)?,
            );
        }
        return Ok(JsonValue::Object(object));
    }

    Err(storage_runtime_error(
        "stored tables must be either contiguous arrays or string-keyed objects",
    ))
}

fn charge_serialized_bytes(budget: &mut LuaConversionBudget, bytes: usize) -> mlua::Result<()> {
    budget.serialized_bytes = budget.serialized_bytes.saturating_add(bytes);
    if budget.serialized_bytes > MAX_STORAGE_BYTES as usize {
        return Err(storage_runtime_error(format!(
            "a stored value may encode to at most {MAX_STORAGE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn json_string_size(value: &str) -> usize {
    value.chars().fold(2usize, |size, character| {
        let encoded = match character {
            '"' | '\\' | '\u{08}' | '\u{0c}' | '\n' | '\r' | '\t' => 2,
            '\u{00}'..='\u{1f}' => 6,
            _ => character.len_utf8(),
        };
        size.saturating_add(encoded)
    })
}

struct LimitedJsonBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl LimitedJsonBuffer {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for LimitedJsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "serialized mod storage exceeds its byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn storage_configuration(message: impl Into<String>) -> ScriptError {
    ScriptError::Configuration(format!("mod storage: {}", message.into()))
}

fn data_configuration(message: impl Into<String>) -> ScriptError {
    ScriptError::Configuration(message.into())
}

fn lua_storage_error(error: ScriptError) -> mlua::Error {
    let message = match error {
        ScriptError::Configuration(message) => message,
        ScriptError::InvalidPath(_) => "mod storage data boundary is invalid".to_owned(),
        ScriptError::Io(_) => "mod storage I/O failed".to_owned(),
        other => format!("mod storage operation failed: {other}"),
    };
    storage_runtime_error(message)
}

fn storage_runtime_error(message: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(message.into())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::scripting::permissions::PermissionPolicy;

    use super::*;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "rustcraft-storage-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn permissions(requested: &[Permission]) -> PermissionSet {
        PermissionSet::resolve("storage-test", requested, &PermissionPolicy::default())
    }

    fn install_for_test(lua: &Lua, state: StorageApiState, requested: &[Permission]) -> Table {
        let game = lua.create_table().unwrap();
        install(lua, &game, &permissions(requested), state).unwrap();
        lua.globals().set("game", game.clone()).unwrap();
        game
    }

    #[test]
    fn api_is_absent_without_permission_and_pruned_per_capability() {
        let root = TempRoot::new("permissions");
        let state = StorageApiState::open(&root.0).unwrap();

        let lua = Lua::new();
        let game = install_for_test(&lua, state.clone(), &[]);
        assert!(!game.contains_key("storage").unwrap());

        let lua = Lua::new();
        let game = install_for_test(&lua, state.clone(), &[Permission::StorageRead]);
        let storage: Table = game.get("storage").unwrap();
        assert!(storage.contains_key("get").unwrap());
        assert!(storage.contains_key("keys").unwrap());
        assert!(!storage.contains_key("set").unwrap());
        assert!(!storage.contains_key("delete").unwrap());

        let lua = Lua::new();
        let game = install_for_test(&lua, state, &[Permission::StorageWrite]);
        let storage: Table = game.get("storage").unwrap();
        assert!(!storage.contains_key("get").unwrap());
        assert!(!storage.contains_key("keys").unwrap());
        assert!(storage.contains_key("set").unwrap());
        assert!(storage.contains_key("delete").unwrap());
    }

    #[test]
    fn json_values_persist_across_fresh_lua_states() {
        let root = TempRoot::new("persist");
        let lua = Lua::new();
        install_for_test(
            &lua,
            StorageApiState::open(&root.0).unwrap(),
            &[Permission::StorageRead, Permission::StorageWrite],
        );
        lua.load(
            r#"
                game.storage.set("enabled", true)
                game.storage.set("count", 42)
                game.storage.set("profile", {
                    name = "Alex",
                    flags = { "fast", "quiet" },
                    optional = game.storage.null,
                    empty = game.storage.array()
                })
                assert(game.storage.len() == 3)
                assert(game.storage.has("profile"))
            "#,
        )
        .exec()
        .unwrap();
        drop(lua);

        let lua = Lua::new();
        install_for_test(
            &lua,
            StorageApiState::open(&root.0).unwrap(),
            &[Permission::StorageRead],
        );
        lua.load(
            r#"
                local profile = game.storage.get("profile")
                assert(profile.name == "Alex")
                assert(profile.flags[1] == "fast" and profile.flags[2] == "quiet")
                assert(profile.optional == game.storage.null)
                assert(#profile.empty == 0)
                local keys = game.storage.keys()
                assert(keys[1] == "count" and keys[2] == "enabled" and keys[3] == "profile")
                local all = game.storage.all()
                assert(all.enabled == true and all.count == 42)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn overlapping_runtime_views_merge_latest_document_and_delete_and_clear_report_changes() {
        let root = TempRoot::new("overlap");
        let first = StorageApiState::open(&root.0).unwrap();
        let replacement = StorageApiState::open(&root.0).unwrap();

        first.set("first".into(), JsonValue::from(1)).unwrap();
        replacement
            .set("replacement".into(), JsonValue::from(2))
            .unwrap();
        first.set("last".into(), JsonValue::from(3)).unwrap();

        let values = replacement.values().unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values["first"], 1);
        assert_eq!(values["replacement"], 2);
        assert_eq!(values["last"], 3);
        assert!(replacement.delete("first").unwrap());
        assert!(!replacement.delete("first").unwrap());
        assert_eq!(first.clear().unwrap(), 2);
        assert_eq!(replacement.clear().unwrap(), 0);
    }

    #[test]
    fn rejects_invalid_keys_unsupported_values_cycles_holes_and_non_finite_numbers() {
        let root = TempRoot::new("invalid-values");
        let lua = Lua::new();
        install_for_test(
            &lua,
            StorageApiState::open(&root.0).unwrap(),
            &[Permission::StorageRead, Permission::StorageWrite],
        );
        lua.load(
            r#"
                local function rejected(callback)
                    local ok = pcall(callback)
                    assert(not ok)
                end
                rejected(function() game.storage.set("", true) end)
                rejected(function() game.storage.set("bad\nkey", true) end)
                rejected(function() game.storage.set("function", function() end) end)
                rejected(function() game.storage.set("nan", 0 / 0) end)
                rejected(function() game.storage.set("hole", { [1] = true, [3] = true }) end)
                rejected(function() game.storage.set("mixed", { [1] = true, name = "x" }) end)
                local cyclic = {}
                cyclic.self = cyclic
                rejected(function() game.storage.set("cycle", cyclic) end)
                local repeated = string.rep("x", game.storage.max_string_bytes)
                rejected(function()
                    game.storage.set("encoded-too-large", {
                        repeated, repeated, repeated, repeated, repeated,
                        repeated, repeated, repeated, repeated
                    })
                end)
                assert(game.storage.len() == 0)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn enforces_depth_string_entry_and_file_limits() {
        let root = TempRoot::new("limits");
        let state = StorageApiState::open(&root.0).unwrap();
        assert!(state
            .set(
                "too-large".into(),
                JsonValue::String("x".repeat(MAX_STORAGE_STRING_BYTES + 1)),
            )
            .is_err());

        let mut deep = JsonValue::Bool(true);
        for _ in 0..=MAX_STORAGE_VALUE_DEPTH {
            deep = JsonValue::Array(vec![deep]);
        }
        assert!(state.set("too-deep".into(), deep).is_err());
        assert!(state
            .set(
                "serialized-too-large".into(),
                JsonValue::Array(
                    (0..9)
                        .map(|_| JsonValue::String("x".repeat(MAX_STORAGE_STRING_BYTES)))
                        .collect(),
                ),
            )
            .is_err());

        let mut document = StorageDocument::default();
        for index in 0..=MAX_STORAGE_ENTRIES {
            document
                .values
                .insert(format!("key-{index}"), JsonValue::Bool(true));
        }
        assert!(state.write_document(&document).is_err());

        fs::create_dir_all(root.0.join("data")).unwrap();
        fs::write(
            root.0.join("data").join(STORAGE_FILE),
            vec![b' '; MAX_STORAGE_BYTES as usize + 1],
        )
        .unwrap();
        assert!(StorageApiState::open(&root.0).unwrap().validate().is_err());
    }

    #[test]
    fn corrupt_or_unknown_version_documents_fail_closed() {
        let root = TempRoot::new("corrupt");
        fs::create_dir_all(root.0.join("data")).unwrap();
        let path = root.0.join("data").join(STORAGE_FILE);
        fs::write(&path, b"not json").unwrap();
        assert!(StorageApiState::open(&root.0).unwrap().validate().is_err());

        fs::write(&path, br#"{"version":2,"values":{}}"#).unwrap();
        assert!(StorageApiState::open(&root.0).unwrap().validate().is_err());
    }

    #[test]
    fn two_mod_roots_are_strictly_isolated() {
        let first_root = TempRoot::new("isolation-first");
        let second_root = TempRoot::new("isolation-second");
        let first = StorageApiState::open(&first_root.0).unwrap();
        let second = StorageApiState::open(&second_root.0).unwrap();
        first
            .set("owner".into(), JsonValue::String("first".into()))
            .unwrap();
        second
            .set("owner".into(), JsonValue::String("second".into()))
            .unwrap();
        assert_eq!(first.get("owner").unwrap().unwrap(), "first");
        assert_eq!(second.get("owner").unwrap().unwrap(), "second");
    }

    #[test]
    fn data_boundary_rejects_escape_and_recovers_interrupted_commits() {
        let root = TempRoot::new("commit-recovery");
        let directory = ModDataDirectory::new(&root.0).unwrap();
        assert!(directory.write("../escape.json", b"no", 16).is_err());
        directory.write("state.json", b"old", 16).unwrap();

        let data = root.0.join("data");
        let target = data.join("state.json");
        let temporary = data.join(".state.json.tmp");
        let backup = data.join(".state.json.bak");
        fs::rename(&target, &backup).unwrap();
        fs::write(&temporary, b"new").unwrap();
        assert_eq!(directory.read("state.json", 16).unwrap().unwrap(), b"new");
        assert!(target.is_file());
        assert!(!temporary.exists());
        assert!(!backup.exists());

        fs::rename(&target, &backup).unwrap();
        assert_eq!(directory.read("state.json", 16).unwrap().unwrap(), b"new");
        assert!(target.is_file());
        assert!(!backup.exists());
    }
}
