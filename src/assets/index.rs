use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

/// A resolved reference to a content-addressed object file.
#[derive(Clone, Debug)]
pub struct ObjectFile {
    /// SHA-1 hash of the asset content.
    pub hash: String,
    /// Uncompressed size in bytes.
    pub size: u64,
    /// Resolved filesystem path: `{objects_dir}/{hash[:2]}/{hash}`.
    pub path: PathBuf,
}

/// Parses a Minecraft asset index JSON and maps resource paths to object files.
///
/// The index file (e.g. `assets/indexes/1.8.json`) has the form:
/// ```json
/// { "objects": {
///     "minecraft/sounds/ambient/cave/cave1.ogg": {
///       "hash": "abc123...", "size": 1234
///     }, ...
/// }}
/// ```
#[derive(Clone, Debug)]
pub struct AssetIndex {
    /// Maps resource path (e.g. `"minecraft:sounds/ambient/cave/cave1.ogg"`) to its object file.
    objects: HashMap<String, ObjectFile>,
    /// Resource-pack bytes override indexed vanilla objects by logical path.
    overrides: HashMap<String, Vec<u8>>,
}

#[derive(Deserialize)]
struct IndexJson {
    objects: HashMap<String, ObjectEntry>,
}

#[derive(Deserialize)]
struct ObjectEntry {
    hash: String,
    size: u64,
}

impl AssetIndex {
    /// Load an asset index from `{assets_dir}/indexes/{version}.json`.
    ///
    /// `assets_dir` is the root assets directory (e.g. `"assets"`).
    /// `version` is the index version string (e.g. `"1.8"`).
    pub fn load(assets_dir: &str, version: &str) -> Result<Self, String> {
        let index_path = format!("{}/indexes/{}.json", assets_dir, version);
        let objects_dir = format!("{}/objects", assets_dir);

        let data = fs::read_to_string(&index_path)
            .map_err(|e| format!("Failed to read asset index {}: {}", index_path, e))?;

        let index: IndexJson = serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse asset index: {}", e))?;

        let mut objects = HashMap::with_capacity(index.objects.len());

        for (key, entry) in &index.objects {
            // Keep the original key format from the JSON (e.g. "minecraft/sounds/foo.ogg")
            // The Minecraft standard uses "namespace/path" (slash) not "namespace:path" (colon)

            let prefix = if entry.hash.len() >= 2 {
                &entry.hash[..2]
            } else {
                "00"
            };
            let obj_path = PathBuf::from(format!("{}/{}/{}", objects_dir, prefix, entry.hash));

            objects.insert(
                key.clone(),
                ObjectFile {
                    hash: entry.hash.clone(),
                    size: entry.size,
                    path: obj_path,
                },
            );
        }

        log::info!(
            "asset index loaded: assets={}, source={index_path}",
            objects.len()
        );

        Ok(AssetIndex {
            objects,
            overrides: HashMap::new(),
        })
    }

    /// Look up a resource by path (e.g. `"minecraft/sounds/ambient/cave/cave1.ogg"`).
    pub fn resolve(&self, resource_path: &str) -> Option<&ObjectFile> {
        self.objects.get(resource_path)
    }

    /// Read the raw bytes of a resource.
    pub fn read_bytes(&self, resource_path: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.overrides.get(resource_path) {
            return Some(bytes.clone());
        }
        let obj = self.objects.get(resource_path)?;
        fs::read(&obj.path).ok()
    }

    /// Install bytes resolved from an enabled resource pack.
    pub fn insert_override(&mut self, resource_path: String, bytes: Vec<u8>) {
        self.overrides.insert(resource_path, bytes);
    }

    /// Read the raw bytes of an ObjectFile directly.
    pub fn read_object(&self, obj: &ObjectFile) -> Option<Vec<u8>> {
        fs::read(&obj.path).ok()
    }

    /// Check if a resource exists in the index.
    pub fn contains(&self, resource_path: &str) -> bool {
        self.objects.contains_key(resource_path)
    }

    /// Get the total number of indexed assets.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Get all resource paths matching a prefix (e.g. `"minecraft:sounds/"`).
    pub fn prefix_matches(&self, prefix: &str) -> Vec<&str> {
        self.objects
            .keys()
            .filter(|k| k.starts_with(prefix))
            .map(|k| k.as_str())
            .collect()
    }
}
