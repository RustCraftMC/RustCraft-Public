use std::path::PathBuf;

use super::index::AssetIndex;
use super::sound::SoundRegistry;

/// A unified Minecraft resource pack combining the content-addressed asset index
/// and derived registries (sounds, etc.).
///
/// # Usage
///
/// ```ignore
/// let pack = ResourcePack::load("assets", "1.8")?;
/// let ogg_bytes = pack.index.read_bytes("minecraft:sounds/dig/stone1.ogg");
/// let event = pack.sounds.get("game.player.hurt");
/// ```
pub struct ResourcePack {
    /// The asset index mapping resource paths to object files.
    pub index: AssetIndex,
    /// The sound event registry parsed from `sounds.json`.
    pub sounds: SoundRegistry,
    /// Root path to the extracted resource pack (e.g. `"assets/minecraft"`).
    pub base_path: PathBuf,
}

impl ResourcePack {
    /// Load a resource pack from the standard Minecraft directory layout.
    ///
    /// - `assets_dir`: root assets directory (e.g. `"assets"`)
    /// - `version`: asset index version (e.g. `"1.8"`)
    pub fn load(assets_dir: &str, version: &str) -> Result<Self, String> {
        let index = AssetIndex::load(assets_dir, version)?;
        let sounds = SoundRegistry::load(&index)?;
        let base_path = PathBuf::from(format!("{}/minecraft", assets_dir));

        log::info!(
            "resource pack loaded: assets={}, sound_events={}, base={}",
            index.len(),
            sounds.len(),
            base_path.display()
        );

        Ok(ResourcePack {
            index,
            sounds,
            base_path,
        })
    }

    /// Load vanilla indexed assets with sound definitions and OGG files from
    /// the enabled resource-pack stack layered on top.
    pub fn load_with_resource_packs(
        assets_dir: &str,
        version: &str,
        resolver: &mut crate::assets::resolver::AssetResolver,
    ) -> Result<Self, String> {
        let mut index = AssetIndex::load(assets_dir, version)?;
        let mut sounds = SoundRegistry::load(&index)?;

        // A pack sounds.json extends vanilla events unless it requests
        // `replace: true`; resolver priority is already highest-pack first.
        if let Some(sound_json) = resolver.read_bytes("minecraft/sounds.json") {
            sounds.merge_json(&sound_json, false)?;
        }
        for relative in resolver.list_resource_files_recursive("sounds", "ogg") {
            let path = format!("minecraft/sounds/{}", relative);
            if let Some(bytes) = resolver.read_bytes(&path) {
                index.insert_override(path, bytes);
            }
        }

        log::info!(
            "layered resource pack loaded: assets={}, sound_events={}, base={}/minecraft",
            index.len(),
            sounds.len(),
            assets_dir
        );

        Ok(ResourcePack {
            index,
            sounds,
            base_path: PathBuf::from(format!("{}/minecraft", assets_dir)),
        })
    }

    /// Read a raw resource by its path (e.g. `"minecraft:textures/blocks/stone.png"`).
    pub fn read_bytes(&self, resource_path: &str) -> Option<Vec<u8>> {
        self.index.read_bytes(resource_path)
    }

    /// Resolve a sound event and return the first available OGG file path.
    pub fn resolve_sound(&self, event_name: &str) -> Option<(String, bool)> {
        self.sounds.pick_random(event_name)
    }
}
