//! Namespaced, size-limited resource reads and controlled registration declarations.
//!
//! Resources are addressed only by `namespace:path`. A mod may read its own files below
//! `assets/<mod-id>/` or immutable bytes supplied by the host's provider snapshot. Lua never sees
//! a filesystem path. Registration records are data declarations for the host renderer; they do
//! not expose Vulkan objects or mutate a resource pack directly.

use mlua::{Lua, Table};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::scripting::permissions::{Permission, PermissionSet};

pub const READ_PERMISSION: &str = "resources.read";
pub const REGISTER_PERMISSION: &str = "resources.register";

pub const MAX_BINARY_RESOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_TEXT_RESOURCE_BYTES: usize = 256 * 1024;
pub const MAX_PROVIDER_RESOURCES: usize = 65_536;
pub const MAX_REGISTRATIONS_PER_MOD: usize = 256;
const MAX_RESOURCE_ID_BYTES: usize = 256;
const MAX_RESOLUTION_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ResourceId {
    namespace: String,
    path: String,
}

impl ResourceId {
    fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty() || value.len() > MAX_RESOURCE_ID_BYTES {
            return Err(format!(
                "resource id must contain between 1 and {MAX_RESOURCE_ID_BYTES} bytes"
            ));
        }
        let Some((namespace, path)) = value.split_once(':') else {
            return Err(
                "resource must be a namespaced id such as 'example:textures/gui/icon.png'".into(),
            );
        };
        if !valid_namespace(namespace) {
            return Err("resource namespace contains invalid characters".into());
        }
        let valid_path = !path.is_empty()
            && !path.starts_with('/')
            && !path.ends_with('/')
            && path.split('/').all(|component| {
                !component.is_empty()
                    && component != "."
                    && component != ".."
                    && component.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'_' | b'-' | b'.')
                    })
            });
        if !valid_path {
            return Err("resource path contains invalid characters or traversal components".into());
        }
        Ok(Self {
            namespace: namespace.into(),
            path: path.into(),
        })
    }

    fn as_string(&self) -> String {
        format!("{}:{}", self.namespace, self.path)
    }
}

fn valid_namespace(namespace: &str) -> bool {
    !namespace.is_empty()
        && namespace.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

/// Immutable host-provided resource bytes captured at a well-defined reload boundary.
///
/// Constructing a new snapshot validates every key up front. Values are reference-counted and a
/// Lua read still enforces the stricter per-call byte limits below.
#[derive(Clone, Debug, Default)]
pub struct ResourceProviderSnapshot {
    entries: Arc<HashMap<String, Arc<[u8]>>>,
}

impl ResourceProviderSnapshot {
    pub fn try_from_entries(
        entries: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self, String> {
        let mut validated = HashMap::new();
        for (id, bytes) in entries {
            ResourceId::parse(&id)?;
            if validated.len() >= MAX_PROVIDER_RESOURCES && !validated.contains_key(&id) {
                return Err(format!(
                    "resource provider snapshot exceeds {MAX_PROVIDER_RESOURCES} entries"
                ));
            }
            if validated.insert(id.clone(), Arc::from(bytes)).is_some() {
                return Err(format!("duplicate resource provider entry '{id}'"));
            }
        }
        Ok(Self {
            entries: Arc::new(validated),
        })
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn get(&self, id: &str) -> Option<Arc<[u8]>> {
        self.entries.get(id).cloned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceRegistrationKind {
    Alias,
    Replacement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRegistration {
    pub owner: String,
    pub kind: ResourceRegistrationKind,
    pub resource: String,
    pub target: String,
}

#[derive(Clone, Debug, Default)]
pub struct SharedResourceRegistry {
    registrations: Rc<RefCell<HashMap<String, ResourceRegistration>>>,
}

fn resolve_registration(
    registrations: &HashMap<String, ResourceRegistration>,
    id: &str,
) -> Result<String, String> {
    let mut current = id.to_owned();
    let mut visited = HashSet::new();
    for _ in 0..MAX_RESOLUTION_DEPTH {
        if !visited.insert(current.clone()) {
            return Err(format!("resource registration cycle contains '{current}'"));
        }
        let Some(registration) = registrations.get(&current) else {
            return Ok(current);
        };
        current = registration.target.clone();
    }
    Err(format!(
        "resource registration chain exceeds {MAX_RESOLUTION_DEPTH} entries"
    ))
}

impl SharedResourceRegistry {
    pub fn resolve(&self, id: &str) -> Result<String, String> {
        ResourceId::parse(id)?;
        let registrations = self.registrations.borrow();
        resolve_registration(&registrations, id)
    }

    pub fn declarations(&self) -> Vec<ResourceRegistration> {
        let mut declarations: Vec<_> = self.registrations.borrow().values().cloned().collect();
        declarations.sort_by(|left, right| left.resource.cmp(&right.resource));
        declarations
    }

    pub fn declarations_for_owner(&self, owner: &str) -> Vec<ResourceRegistration> {
        self.declarations()
            .into_iter()
            .filter(|declaration| declaration.owner == owner)
            .collect()
    }

    pub fn clear_owner(&self, owner: &str) {
        self.registrations
            .borrow_mut()
            .retain(|_, registration| registration.owner != owner);
    }

    pub(crate) fn clear(&self) {
        self.registrations.borrow_mut().clear();
    }

    pub fn len(&self) -> usize {
        self.registrations.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.registrations.borrow().is_empty()
    }

    pub(crate) fn restore_owner(
        &self,
        owner: &str,
        declarations: &[ResourceRegistration],
    ) -> Result<(), String> {
        if declarations.len() > MAX_REGISTRATIONS_PER_MOD {
            return Err(format!(
                "mod '{owner}' exceeds {MAX_REGISTRATIONS_PER_MOD} resource registrations"
            ));
        }

        let mut restored = Vec::with_capacity(declarations.len());
        for declaration in declarations {
            if declaration.owner != owner {
                return Err(format!(
                    "cannot restore resource registration owned by '{}' as '{owner}'",
                    declaration.owner
                ));
            }
            restored.push(ResourceRegistration {
                owner: owner.to_owned(),
                kind: declaration.kind,
                resource: ResourceId::parse(&declaration.resource)?.as_string(),
                target: ResourceId::parse(&declaration.target)?.as_string(),
            });
        }

        let mut registrations = self.registrations.borrow().clone();
        registrations.retain(|_, registration| registration.owner != owner);
        for declaration in restored {
            if let Some(existing) = registrations.get(&declaration.resource) {
                return Err(format!(
                    "resource '{}' is already registered by '{}'",
                    declaration.resource, existing.owner
                ));
            }
            registrations.insert(declaration.resource.clone(), declaration);
        }

        for resource in registrations.keys() {
            resolve_registration(&registrations, resource)?;
        }
        *self.registrations.borrow_mut() = registrations;
        Ok(())
    }

    fn register(
        &self,
        owner: &str,
        kind: ResourceRegistrationKind,
        resource: ResourceId,
        target: ResourceId,
    ) -> Result<(), String> {
        let resource = resource.as_string();
        let target = target.as_string();
        let previous = {
            let mut registrations = self.registrations.borrow_mut();
            if let Some(existing) = registrations.get(&resource) {
                if existing.owner != owner {
                    return Err(format!(
                        "resource '{resource}' is already registered by '{}'",
                        existing.owner
                    ));
                }
            } else if registrations
                .values()
                .filter(|registration| registration.owner == owner)
                .count()
                >= MAX_REGISTRATIONS_PER_MOD
            {
                return Err(format!(
                    "mod '{owner}' exceeds {MAX_REGISTRATIONS_PER_MOD} resource registrations"
                ));
            }
            registrations.insert(
                resource.clone(),
                ResourceRegistration {
                    owner: owner.into(),
                    kind,
                    resource: resource.clone(),
                    target,
                },
            )
        };

        if let Err(error) = self.resolve(&resource) {
            let mut registrations = self.registrations.borrow_mut();
            if let Some(previous) = previous {
                registrations.insert(resource, previous);
            } else {
                registrations.remove(&resource);
            }
            return Err(error);
        }
        Ok(())
    }

    fn unregister(&self, owner: &str, resource: &ResourceId) -> Result<bool, String> {
        let resource = resource.as_string();
        let mut registrations = self.registrations.borrow_mut();
        let Some(existing) = registrations.get(&resource) else {
            return Ok(false);
        };
        if existing.owner != owner {
            return Err(format!(
                "resource '{resource}' is owned by '{}', not '{owner}'",
                existing.owner
            ));
        }
        registrations.remove(&resource);
        Ok(true)
    }
}

/// Per-mod resource view. `mod_root` is retained only on the Rust side and is never exposed to Lua.
#[derive(Clone, Debug)]
pub struct ResourceApiState {
    owner: String,
    mod_root: PathBuf,
    provider: ResourceProviderSnapshot,
    registry: SharedResourceRegistry,
}

impl ResourceApiState {
    pub fn new(
        owner: impl Into<String>,
        mod_root: &Path,
        provider: ResourceProviderSnapshot,
        registry: SharedResourceRegistry,
    ) -> Result<Self, String> {
        let owner = owner.into();
        if !valid_namespace(&owner) {
            return Err("resource owner must be a valid lowercase namespace".into());
        }
        let mod_root = mod_root
            .canonicalize()
            .map_err(|error| format!("failed to resolve mod root: {error}"))?;
        if !mod_root.is_dir() {
            return Err("mod root must be a directory".into());
        }
        Ok(Self {
            owner,
            mod_root,
            provider,
            registry,
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn registry(&self) -> SharedResourceRegistry {
        self.registry.clone()
    }

    /// Must be called when the owning runtime unloads. It is intentionally explicit because
    /// cloned Lua closures make `Drop` unsuitable for owner-wide cleanup.
    pub fn clear_registrations(&self) {
        self.registry.clear_owner(&self.owner);
    }

    pub fn exists(&self, id: &str) -> Result<bool, String> {
        let id = ResourceId::parse(id)?;
        if self.own_file(&id)?.is_some() {
            return Ok(true);
        }
        let requested = id.as_string();
        let resolved = self.registry.resolve(&requested)?;
        if resolved != requested {
            let resolved_id = ResourceId::parse(&resolved)?;
            if self.own_file(&resolved_id)?.is_some() || self.provider.contains(&resolved) {
                return Ok(true);
            }
        }
        Ok(self.provider.contains(&requested))
    }

    pub fn read_bytes(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        self.read_limited(id, MAX_BINARY_RESOURCE_BYTES)
    }

    pub fn read_text(&self, id: &str) -> Result<Option<String>, String> {
        let Some(bytes) = self.read_limited(id, MAX_TEXT_RESOURCE_BYTES)? else {
            return Ok(None);
        };
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("resource '{id}' is not valid UTF-8"))
    }

    pub fn register_alias(&self, alias: &str, target: &str) -> Result<(), String> {
        let alias = ResourceId::parse(alias)?;
        let target = ResourceId::parse(target)?;
        if alias.namespace != self.owner {
            return Err(format!(
                "resource alias namespace must be the owning mod namespace '{}'",
                self.owner
            ));
        }
        if !self.exists(&target.as_string())? {
            return Err(format!(
                "resource alias target '{}' does not exist",
                target.as_string()
            ));
        }
        self.registry
            .register(&self.owner, ResourceRegistrationKind::Alias, alias, target)
    }

    pub fn register_replacement(&self, resource: &str, replacement: &str) -> Result<(), String> {
        let resource = ResourceId::parse(resource)?;
        let replacement = ResourceId::parse(replacement)?;
        if replacement.namespace != self.owner {
            return Err(format!(
                "replacement target namespace must be the owning mod namespace '{}'",
                self.owner
            ));
        }
        if !self.exists(&replacement.as_string())? {
            return Err(format!(
                "replacement target '{}' does not exist",
                replacement.as_string()
            ));
        }
        self.registry.register(
            &self.owner,
            ResourceRegistrationKind::Replacement,
            resource,
            replacement,
        )
    }

    pub fn unregister(&self, resource: &str) -> Result<bool, String> {
        self.registry
            .unregister(&self.owner, &ResourceId::parse(resource)?)
    }

    fn read_limited(&self, id: &str, max_bytes: usize) -> Result<Option<Vec<u8>>, String> {
        let id = ResourceId::parse(id)?;
        if let Some(path) = self.own_file(&id)? {
            return read_file_limited(&path, max_bytes, &id.as_string()).map(Some);
        }

        let requested = id.as_string();
        let resolved = self.registry.resolve(&requested)?;
        if resolved != requested {
            let resolved_id = ResourceId::parse(&resolved)?;
            if let Some(path) = self.own_file(&resolved_id)? {
                return read_file_limited(&path, max_bytes, &resolved).map(Some);
            }
            if let Some(bytes) = self.provider.get(&resolved) {
                return checked_provider_bytes(bytes, max_bytes, &resolved).map(Some);
            }
        }

        self.provider
            .get(&requested)
            .map(|bytes| checked_provider_bytes(bytes, max_bytes, &requested))
            .transpose()
    }

    fn own_file(&self, id: &ResourceId) -> Result<Option<PathBuf>, String> {
        if id.namespace != self.owner {
            return Ok(None);
        }
        let namespace_root = self.mod_root.join("assets").join(&self.owner);
        if !namespace_root.exists() {
            return Ok(None);
        }
        let namespace_root = namespace_root
            .canonicalize()
            .map_err(|error| format!("failed to resolve mod asset root: {error}"))?;
        if !namespace_root.starts_with(&self.mod_root) || !namespace_root.is_dir() {
            return Err("mod asset root escapes the owning mod directory".into());
        }

        let path = namespace_root.join(&id.path);
        if !path.exists() {
            return Ok(None);
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("failed to resolve resource '{}': {error}", id.as_string()))?;
        if !canonical.starts_with(&namespace_root) || !canonical.is_file() {
            return Err(format!(
                "resource '{}' escapes its namespaced asset directory",
                id.as_string()
            ));
        }
        Ok(Some(canonical))
    }
}

pub fn install(
    lua: &Lua,
    game: &Table,
    permissions: &PermissionSet,
    state: ResourceApiState,
) -> mlua::Result<()> {
    let can_read = permissions.contains(Permission::ResourcesRead);
    let can_register = permissions.contains(Permission::ResourcesRegister);
    if !can_read && !can_register {
        return Ok(());
    }

    let resources = lua.create_table()?;
    if can_read {
        let exists_state = state.clone();
        resources.set(
            "exists",
            lua.create_function(move |_, id: String| {
                exists_state.exists(&id).map_err(mlua::Error::external)
            })?,
        )?;

        let text_state = state.clone();
        resources.set(
            "read_text",
            lua.create_function(move |_, id: String| {
                text_state.read_text(&id).map_err(mlua::Error::external)
            })?,
        )?;

        let bytes_state = state.clone();
        resources.set(
            "read_bytes",
            lua.create_function(move |lua, id: String| {
                bytes_state
                    .read_bytes(&id)
                    .map_err(mlua::Error::external)?
                    .map(|bytes| lua.create_string(bytes))
                    .transpose()
            })?,
        )?;

        let resolve_registry = state.registry();
        resources.set(
            "resolve",
            lua.create_function(move |_, id: String| {
                resolve_registry.resolve(&id).map_err(mlua::Error::external)
            })?,
        )?;
        resources.set("max_binary_bytes", MAX_BINARY_RESOURCE_BYTES)?;
        resources.set("max_text_bytes", MAX_TEXT_RESOURCE_BYTES)?;
    }

    if can_register {
        let alias_state = state.clone();
        resources.set(
            "register_alias",
            lua.create_function(move |_, (alias, target): (String, String)| {
                alias_state
                    .register_alias(&alias, &target)
                    .map_err(mlua::Error::external)
            })?,
        )?;

        let replacement_state = state.clone();
        resources.set(
            "register_replacement",
            lua.create_function(move |_, (resource, replacement): (String, String)| {
                replacement_state
                    .register_replacement(&resource, &replacement)
                    .map_err(mlua::Error::external)
            })?,
        )?;

        resources.set(
            "unregister",
            lua.create_function(move |_, resource: String| {
                state.unregister(&resource).map_err(mlua::Error::external)
            })?,
        )?;
    }

    game.set("resources", resources)
}

fn read_file_limited(path: &Path, max_bytes: usize, id: &str) -> Result<Vec<u8>, String> {
    let length = path
        .metadata()
        .map_err(|error| format!("failed to inspect resource '{id}': {error}"))?
        .len();
    if length > max_bytes as u64 {
        return Err(format!(
            "resource '{id}' exceeds the {max_bytes}-byte read limit"
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read resource '{id}': {error}"))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "resource '{id}' exceeds the {max_bytes}-byte read limit"
        ));
    }
    Ok(bytes)
}

fn checked_provider_bytes(bytes: Arc<[u8]>, max_bytes: usize, id: &str) -> Result<Vec<u8>, String> {
    if bytes.len() > max_bytes {
        return Err(format!(
            "resource '{id}' exceeds the {max_bytes}-byte read limit"
        ));
    }
    Ok(bytes.as_ref().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::permissions::PermissionPolicy;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(owner: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "rustcraft_resources_{}_{}_{}",
                std::process::id(),
                owner,
                nonce
            ));
            fs::create_dir_all(root.join("assets").join(owner)).unwrap();
            Self(root)
        }

        fn write(&self, owner: &str, relative: &str, bytes: &[u8]) {
            let path = self.0.join("assets").join(owner).join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn state(
        root: &TestRoot,
        owner: &str,
        provider: ResourceProviderSnapshot,
        registry: SharedResourceRegistry,
    ) -> ResourceApiState {
        ResourceApiState::new(owner, &root.0, provider, registry).unwrap()
    }

    fn permissions(owner: &str, requested: &[Permission]) -> PermissionSet {
        let mut policy = PermissionPolicy::default();
        if requested.contains(&Permission::ResourcesRegister) {
            policy.approve_for(owner, Permission::ResourcesRegister);
        }
        PermissionSet::resolve(owner, requested, &policy)
    }

    fn install_for_test(
        lua: &Lua,
        owner: &str,
        requested: &[Permission],
        state: ResourceApiState,
    ) -> Table {
        let game = lua.create_table().unwrap();
        install(lua, &game, &permissions(owner, requested), state).unwrap();
        lua.globals().set("game", game.clone()).unwrap();
        game
    }

    #[test]
    fn own_assets_have_priority_and_reads_return_owned_lua_values() {
        let root = TestRoot::new("example");
        root.write("example", "texts/hello.txt", b"from mod");
        root.write("example", "data/raw.bin", &[0, 1, 0xff]);
        let provider = ResourceProviderSnapshot::try_from_entries(vec![
            ("example:texts/hello.txt".into(), b"from provider".to_vec()),
            ("minecraft:lang/en_us.lang".into(), b"Language".to_vec()),
        ])
        .unwrap();
        let state = state(
            &root,
            "example",
            provider,
            SharedResourceRegistry::default(),
        );
        let lua = Lua::new();
        install_for_test(&lua, "example", &[Permission::ResourcesRead], state);

        let (local, provider, binary_len, missing): (String, String, usize, bool) = lua
            .load(
                r#"
                local bytes = game.resources.read_bytes("example:data/raw.bin")
                return game.resources.read_text("example:texts/hello.txt"),
                    game.resources.read_text("minecraft:lang/en_us.lang"),
                    #bytes,
                    game.resources.read_text("example:missing.txt") == nil
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(local, "from mod");
        assert_eq!(provider, "Language");
        assert_eq!(binary_len, 3);
        assert!(missing);
    }

    #[test]
    fn rejects_path_escape_invalid_ids_and_oversized_resources() {
        let root = TestRoot::new("example");
        root.write(
            "example",
            "huge.bin",
            &vec![0; MAX_BINARY_RESOURCE_BYTES + 1],
        );
        let local_state = state(
            &root,
            "example",
            ResourceProviderSnapshot::default(),
            SharedResourceRegistry::default(),
        );
        for invalid in [
            "../secret",
            "example:../secret",
            "example:textures/../../secret",
            "example:/absolute",
            "example:textures\\secret",
            "Example:textures/icon.png",
        ] {
            assert!(
                local_state.read_bytes(invalid).is_err(),
                "accepted {invalid}"
            );
        }
        assert!(local_state.read_bytes("example:huge.bin").is_err());

        let provider = ResourceProviderSnapshot::try_from_entries(vec![(
            "minecraft:huge.bin".into(),
            vec![0; MAX_BINARY_RESOURCE_BYTES + 1],
        )])
        .unwrap();
        let provider_state = state(
            &root,
            "example",
            provider,
            SharedResourceRegistry::default(),
        );
        assert!(provider_state.read_bytes("minecraft:huge.bin").is_err());
    }

    #[test]
    fn text_reads_reject_invalid_utf8_and_use_a_smaller_limit() {
        let root = TestRoot::new("example");
        root.write("example", "invalid.txt", &[0xff, 0xfe]);
        root.write(
            "example",
            "large.txt",
            &vec![b'x'; MAX_TEXT_RESOURCE_BYTES + 1],
        );
        let state = state(
            &root,
            "example",
            ResourceProviderSnapshot::default(),
            SharedResourceRegistry::default(),
        );
        assert!(state.read_text("example:invalid.txt").is_err());
        assert!(state.read_text("example:large.txt").is_err());
        assert!(state.read_bytes("example:large.txt").is_ok());
    }

    #[test]
    fn registration_validates_owner_conflicts_cycles_and_lifecycle_cleanup() {
        let registry = SharedResourceRegistry::default();
        let first_root = TestRoot::new("first");
        first_root.write("first", "textures/a.png", b"a");
        first_root.write("first", "textures/b.png", b"b");
        let first = state(
            &first_root,
            "first",
            ResourceProviderSnapshot::default(),
            registry.clone(),
        );
        first
            .register_alias("first:alias", "first:textures/a.png")
            .unwrap();
        first
            .register_replacement("minecraft:textures/gui/icons.png", "first:textures/b.png")
            .unwrap();
        assert_eq!(
            registry.resolve("first:alias").unwrap(),
            "first:textures/a.png"
        );
        assert_eq!(registry.declarations_for_owner("first").len(), 2);
        assert!(first
            .register_alias("other:alias", "first:textures/a.png")
            .is_err());
        assert!(first
            .register_replacement("minecraft:x", "minecraft:y")
            .is_err());
        first
            .register_alias("first:alias_chain", "first:alias")
            .unwrap();
        assert!(first
            .register_alias("first:alias", "first:alias_chain")
            .is_err());
        assert_eq!(
            registry.resolve("first:alias").unwrap(),
            "first:textures/a.png"
        );

        let second_root = TestRoot::new("second");
        second_root.write("second", "textures/a.png", b"second");
        let second = state(
            &second_root,
            "second",
            ResourceProviderSnapshot::default(),
            registry.clone(),
        );
        assert!(second
            .register_replacement("minecraft:textures/gui/icons.png", "second:textures/a.png")
            .is_err());

        first.clear_registrations();
        assert!(registry.declarations_for_owner("first").is_empty());
        second
            .register_replacement("minecraft:textures/gui/icons.png", "second:textures/a.png")
            .unwrap();
    }

    #[test]
    fn lua_api_is_permission_pruned_and_registers_owned_declarations() {
        let root = TestRoot::new("example");
        root.write("example", "textures/icon.png", b"png");
        let registry = SharedResourceRegistry::default();
        let resource_state = state(
            &root,
            "example",
            ResourceProviderSnapshot::default(),
            registry.clone(),
        );
        let lua = Lua::new();
        let game = install_for_test(
            &lua,
            "example",
            &[Permission::ResourcesRead],
            resource_state.clone(),
        );
        let resources: Table = game.get("resources").unwrap();
        assert!(resources.contains_key("read_text").unwrap());
        assert!(!resources.contains_key("register_alias").unwrap());

        let lua = Lua::new();
        install_for_test(
            &lua,
            "example",
            &[Permission::ResourcesRegister],
            resource_state,
        );
        lua.load(
            r#"
            game.resources.register_alias(
                "example:current_icon",
                "example:textures/icon.png"
            )
            game.resources.register_replacement(
                "minecraft:textures/gui/icons.png",
                "example:textures/icon.png"
            )
            "#,
        )
        .exec()
        .unwrap();
        assert_eq!(registry.declarations_for_owner("example").len(), 2);
    }
}
