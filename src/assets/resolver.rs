//! Asset resolver — unified resource lookup with resource-pack override support.
//!
//! Lookup order: enabled resource packs (highest priority first) → vanilla assets.
//! Resource packs are .zip files in `resourcepacks/` containing `pack.mcmeta`
//! and an `assets/minecraft/` tree that mirrors the vanilla layout.
//!
//! Vanilla assets are loaded directly from the filesystem (`assets/minecraft/`).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A resource pack loaded from a .zip file.
pub struct ZipResourcePack {
    pub name: String,
    pub description: String,
    pub zip: zip::ZipArchive<std::fs::File>,
}

impl ZipResourcePack {
    /// Open a resource pack zip and parse its pack.mcmeta.
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = fs::File::open(path).map_err(|e| format!("Cannot open {:?}: {}", path, e))?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|e| format!("Bad zip {:?}: {}", path, e))?;

        // Parse pack.mcmeta for description
        let description = if let Ok(mut entry) = zip.by_name("pack.mcmeta") {
            let mut text = String::new();
            if entry.read_to_string(&mut text).is_ok() {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&text) {
                    meta["pack"]["description"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .to_string()
                } else {
                    "Unknown".into()
                }
            } else {
                "Unknown".into()
            }
        } else {
            "No pack.mcmeta".into()
        };

        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".into());

        Ok(ZipResourcePack {
            name,
            description,
            zip,
        })
    }

    /// Read a file from the zip by its path within the archive.
    /// The path should be relative to the zip root, e.g. "assets/minecraft/textures/blocks/stone.png".
    fn read_file(&mut self, internal_path: &str) -> Option<Vec<u8>> {
        let mut entry = self.zip.by_name(internal_path).ok()?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).ok()?;
        Some(buf)
    }

    /// Check if a file exists in the zip.
    fn has_file(&mut self, internal_path: &str) -> bool {
        self.zip.by_name(internal_path).is_ok()
    }
}

/// A resource pack stored as a directory on disk (folder-based pack).
pub struct FolderResourcePack {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

impl FolderResourcePack {
    pub fn open(path: &Path) -> Result<Self, String> {
        let pack_mcmeta = path.join("pack.mcmeta");
        let description = if pack_mcmeta.exists() {
            if let Ok(text) = std::fs::read_to_string(&pack_mcmeta) {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&text) {
                    meta["pack"]["description"]
                        .as_str()
                        .unwrap_or("Unknown")
                        .to_string()
                } else {
                    "Unknown".into()
                }
            } else {
                "Unknown".into()
            }
        } else {
            "No pack.mcmeta".into()
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".into());
        Ok(FolderResourcePack {
            name,
            description,
            path: path.to_path_buf(),
        })
    }

    fn read_file(&self, internal_path: &str) -> Option<Vec<u8>> {
        let p = self.path.join(internal_path);
        std::fs::read(&p).ok()
    }

    fn has_file(&self, internal_path: &str) -> bool {
        self.path.join(internal_path).exists()
    }
}

/// Unified asset resolver — looks up resources in resource packs first,
/// then falls back to the vanilla asset directory.
pub struct AssetResolver {
    /// Path to the vanilla assets root (e.g. "assets/minecraft").
    pub vanilla_root: PathBuf,
    /// Enabled resource packs, highest priority first.
    packs: Vec<Pack>,
    /// Cache of texture name lists per category.
    texture_list_cache: HashMap<String, Vec<String>>,
}

enum Pack {
    Zip(ZipResourcePack),
    Folder(FolderResourcePack),
}

impl AssetResolver {
    /// Create a resolver with only vanilla assets (no resource packs).
    pub fn new(vanilla_base: &str) -> Self {
        AssetResolver {
            vanilla_root: PathBuf::from(vanilla_base),
            packs: Vec::new(),
            texture_list_cache: HashMap::new(),
        }
    }

    /// Create a resolver and load resource packs from the given directory.
    /// Create a resolver and load resource packs from the given directory.
    /// Enabled packs are listed highest priority first.
    pub fn with_resource_packs(vanilla_base: &str, rp_dir: &str, enabled: &[String]) -> Self {
        let mut resolver = Self::new(vanilla_base);
        resolver.packs = Self::load_enabled_packs(rp_dir, enabled);
        log::debug!(
            "asset resolver ready: vanilla_root={}, requested_packs={}, loaded_packs={}",
            resolver.vanilla_root.display(),
            enabled.len(),
            resolver.packs.len()
        );
        resolver
    }

    /// Rebuild resolver with new pack list (purges caches).
    pub fn reload(&mut self, rp_dir: &str, enabled: &[String]) {
        self.texture_list_cache.clear();
        self.packs = Self::load_enabled_packs(rp_dir, enabled);
        log::info!(
            "asset resolver reloaded: requested_packs={}, loaded_packs={}",
            enabled.len(),
            self.packs.len()
        );
    }

    fn load_enabled_packs(rp_dir: &str, enabled: &[String]) -> Vec<Pack> {
        let mut packs = Vec::with_capacity(enabled.len());
        for name in enabled.iter().rev() {
            let zip_path = Path::new(rp_dir).join(format!("{}.zip", name));
            if zip_path.exists() {
                match ZipResourcePack::open(&zip_path) {
                    Ok(pack) => {
                        log::info!(
                            "loaded zip resource pack: name='{}', description='{}', path={}",
                            pack.name,
                            pack.description,
                            zip_path.display()
                        );
                        packs.push(Pack::Zip(pack));
                    }
                    Err(error) => log::warn!(
                        "failed to load zip resource pack '{}': {}",
                        zip_path.display(),
                        error
                    ),
                }
            } else {
                let dir_path = Path::new(rp_dir).join(name);
                if dir_path.is_dir() {
                    match FolderResourcePack::open(&dir_path) {
                        Ok(pack) => {
                            log::info!(
                                "loaded folder resource pack: name='{}', description='{}', path={}",
                                pack.name,
                                pack.description,
                                dir_path.display()
                            );
                            packs.push(Pack::Folder(pack));
                        }
                        Err(error) => log::warn!(
                            "failed to load folder resource pack '{}': {}",
                            dir_path.display(),
                            error
                        ),
                    }
                } else {
                    log::warn!(
                        "enabled resource pack '{}' was not found as {} or {}",
                        name,
                        zip_path.display(),
                        dir_path.display()
                    );
                }
            }
        }
        packs
    }

    /// Read raw bytes for a resource path like "minecraft/textures/blocks/stone.png".
    pub fn read_bytes(&mut self, resource_path: &str) -> Option<Vec<u8>> {
        let normalized = normalize_resource_path(resource_path);

        // Check resource packs first (highest priority first)
        for pack in &mut self.packs {
            let internal = format!("assets/{}", normalized);
            match pack {
                Pack::Zip(z) => {
                    if let Some(data) = z.read_file(&internal) {
                        return Some(data);
                    }
                }
                Pack::Folder(f) => {
                    if let Some(data) = f.read_file(&internal) {
                        return Some(data);
                    }
                }
            }
        }
        // Fall back to vanilla filesystem
        let fs_path = if normalized.starts_with("minecraft/") {
            self.vanilla_root.join(&normalized["minecraft/".len()..])
        } else {
            self.vanilla_root.join(&normalized)
        };
        match fs::read(&fs_path) {
            Ok(data) => Some(data),
            Err(_) => {
                // Also try the flat assets/textures/blocks/ directory
                let flat_path = PathBuf::from("assets/textures/blocks")
                    .join(normalized.rsplit('/').next().unwrap_or(""));
                fs::read(&flat_path).ok()
            }
        }
    }

    /// Read a text resource.
    pub fn read_string(&mut self, resource_path: &str) -> Option<String> {
        let bytes = self.read_bytes(resource_path)?;
        String::from_utf8(bytes).ok()
    }

    /// List all texture files in a category (e.g. "blocks", "items").
    /// Returns filenames like "stone.png", "dirt.png", etc.
    pub fn list_textures(&mut self, category: &str) -> Vec<String> {
        if let Some(cached) = self.texture_list_cache.get(category) {
            return cached.clone();
        }

        let mut names = std::collections::BTreeSet::new();

        // Scan vanilla directory
        let vanilla_dir = self.vanilla_root.join("textures").join(category);
        if let Ok(entries) = fs::read_dir(&vanilla_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".png") {
                        names.insert(name.to_string());
                    }
                }
            }
        }

        // Also scan flat directory
        let flat_dir = format!("assets/textures/{}", category);
        if let Ok(entries) = fs::read_dir(&flat_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".png") {
                        names.insert(name.to_string());
                    }
                }
            }
        }

        // Scan resource packs
        for pack in &mut self.packs {
            let tex_dir = format!("assets/minecraft/textures/{}", category);
            match pack {
                Pack::Zip(z) => {
                    for i in 0..z.zip.len() {
                        if let Ok(entry) = z.zip.by_index(i) {
                            let name = entry.name().to_string();
                            if name.starts_with(&format!("{}/", tex_dir)) && name.ends_with(".png")
                            {
                                let basename = name.rsplit('/').next().unwrap_or("");
                                names.insert(basename.to_string());
                            }
                        }
                    }
                }
                Pack::Folder(f) => {
                    let dir = f.path.join(&tex_dir);
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.ends_with(".png") {
                                    names.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        let result: Vec<String> = names.into_iter().collect();
        self.texture_list_cache
            .insert(category.to_string(), result.clone());
        result
    }

    /// List files from the vanilla namespace plus every enabled resource pack.
    /// Returned paths are relative to `assets/minecraft/<directory>`.
    pub fn list_resource_files(&mut self, directory: &str, extension: &str) -> Vec<String> {
        let mut names = std::collections::BTreeSet::new();
        let wanted_suffix = format!(".{}", extension.trim_start_matches('.'));

        let vanilla_dir = self.vanilla_root.join(directory);
        if let Ok(entries) = fs::read_dir(vanilla_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(&wanted_suffix) {
                        names.insert(name.to_string());
                    }
                }
            }
        }

        let pack_dir = format!("assets/minecraft/{}", directory.trim_matches('/'));
        let prefix = format!("{}/", pack_dir);
        for pack in &mut self.packs {
            match pack {
                Pack::Zip(zip) => {
                    for index in 0..zip.zip.len() {
                        if let Ok(entry) = zip.zip.by_index(index) {
                            let path = entry.name();
                            if let Some(relative) = path.strip_prefix(&prefix) {
                                if !relative.contains('/') && relative.ends_with(&wanted_suffix) {
                                    names.insert(relative.to_string());
                                }
                            }
                        }
                    }
                }
                Pack::Folder(folder) => {
                    let path = folder.path.join(&pack_dir);
                    if let Ok(entries) = fs::read_dir(path) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                if name.ends_with(&wanted_suffix) {
                                    names.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        names.into_iter().collect()
    }

    /// Recursively list files under an asset directory.  Sound assets are
    /// nested (`sounds/dig/stone1.ogg`), unlike the flat texture directories.
    pub fn list_resource_files_recursive(
        &mut self,
        directory: &str,
        extension: &str,
    ) -> Vec<String> {
        let mut names = std::collections::BTreeSet::new();
        let wanted_suffix = format!(".{}", extension.trim_start_matches('.'));
        let prefix = format!("assets/minecraft/{}/", directory.trim_matches('/'));

        for pack in &mut self.packs {
            match pack {
                Pack::Zip(zip) => {
                    for index in 0..zip.zip.len() {
                        if let Ok(entry) = zip.zip.by_index(index) {
                            if let Some(relative) = entry.name().strip_prefix(&prefix) {
                                if relative.ends_with(&wanted_suffix) {
                                    names.insert(relative.to_string());
                                }
                            }
                        }
                    }
                }
                Pack::Folder(folder) => {
                    let root = folder.path.join(&prefix);
                    collect_files_recursive(&root, &root, &wanted_suffix, &mut names);
                }
            }
        }
        names.into_iter().collect()
    }

    /// Number of enabled resource packs.
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Get info about all loaded resource packs.
    pub fn pack_info(&self) -> Vec<(String, String)> {
        self.packs
            .iter()
            .map(|p| match p {
                Pack::Zip(z) => (z.name.clone(), z.description.clone()),
                Pack::Folder(f) => (f.name.clone(), f.description.clone()),
            })
            .collect()
    }

    /// Read a raw file from resource pack root (not the assets/ prefix).
    /// The path is relative to the pack root (e.g. "mcpatcher/sky/world0/sky0.png").
    pub fn read_raw(&mut self, internal_path: &str) -> Option<Vec<u8>> {
        let internal_path = normalize_resource_path(internal_path);
        for pack in &mut self.packs {
            match pack {
                Pack::Zip(z) => {
                    if let Some(data) = z.read_file(&internal_path) {
                        return Some(data);
                    }
                }
                Pack::Folder(f) => {
                    if let Some(data) = f.read_file(&internal_path) {
                        return Some(data);
                    }
                }
            }
        }
        None
    }

    /// List files in a directory within resource packs.
    /// Returns relative file names (not full paths).
    pub fn list_pack_dir(&mut self, dir_path: &str) -> Vec<String> {
        let mut files = std::collections::BTreeSet::new();
        for pack in &mut self.packs {
            match pack {
                Pack::Zip(z) => {
                    let prefix = format!("{}/", dir_path);
                    for i in 0..z.zip.len() {
                        if let Ok(entry) = z.zip.by_index(i) {
                            let name = entry.name().to_string();
                            if name.starts_with(&prefix) {
                                let rel = &name[prefix.len()..];
                                files.insert(rel.to_string());
                            }
                        }
                    }
                }
                Pack::Folder(f) => {
                    let dir = f.path.join(dir_path);
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.flatten() {
                            files.insert(entry.file_name().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        files.into_iter().collect()
    }
}

/// Normalise a resource path by resolving `..` and `.` components.
/// ZIP archives perform exact string matching on entry names and cannot
/// handle path traversal, so callers must feed resolver a canonical path.
fn normalize_resource_path(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    stack.join("/")
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    suffix: &str,
    names: &mut std::collections::BTreeSet<String>,
) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, suffix, names);
        } else if path.to_string_lossy().ends_with(suffix) {
            if let Ok(relative) = path.strip_prefix(root) {
                names.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}
