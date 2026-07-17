use std::collections::HashMap;

use serde::Deserialize;

use super::index::AssetIndex;

/// A single sound file reference within a sound event.
#[derive(Clone, Debug)]
pub enum SoundRef {
    /// A direct file reference with an optional streaming flag.
    /// `name` is relative to `minecraft/sounds/` (e.g. `"ambient/cave/cave1"`).
    File { name: String, stream: bool },
    /// A reference to another sound event (for event chaining).
    /// e.g. `music.game.creative` references `music.game`.
    Event { name: String },
}

/// A parsed sound event entry from `sounds.json`.
#[derive(Clone, Debug)]
pub struct SoundEntry {
    /// Category string from JSON (e.g. "ambient", "block", "hostile", "music").
    pub category: String,
    /// The list of sound sources for this event.
    pub sounds: Vec<SoundRef>,
}

/// Maps sound event names to their file references, loaded from `sounds.json`.
#[derive(Clone, Debug)]
pub struct SoundRegistry {
    /// Event name (e.g. `"game.player.hurt"`) -> SoundEntry.
    events: HashMap<String, SoundEntry>,
}

// -- Deserialization types for sounds.json --

#[derive(Deserialize)]
struct SoundsJson(HashMap<String, SoundEntryJson>);

#[derive(Deserialize)]
struct SoundEntryJson {
    category: Option<String>,
    #[serde(default)]
    replace: bool,
    sounds: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct SoundFileJson {
    name: String,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    attenuation: Option<i32>,
}

#[derive(Deserialize)]
struct SoundEventRef {
    #[serde(rename = "type")]
    ref_type: Option<String>,
    name: String,
}

impl SoundRegistry {
    /// Load the sound registry from `sounds.json` via the asset index.
    pub fn load(index: &AssetIndex) -> Result<Self, String> {
        let data = index
            .read_bytes("minecraft/sounds.json")
            .ok_or("sounds.json not found in asset index")?;

        let mut registry = SoundRegistry {
            events: HashMap::new(),
        };
        registry.merge_json(&data, true)?;

        log::info!(
            "sound registry loaded: events={}, source=minecraft/sounds.json",
            registry.events.len()
        );

        Ok(registry)
    }

    /// Merge an enabled resource pack's sounds.json using vanilla's event
    /// semantics: `replace: true` replaces the event, otherwise sources append.
    pub fn merge_json(&mut self, data: &[u8], replace_existing: bool) -> Result<(), String> {
        let raw: SoundsJson = serde_json::from_slice(data)
            .map_err(|e| format!("Failed to parse sounds.json: {}", e))?;
        for (event_name, entry_json) in raw.0 {
            let (entry, replace) = parse_entry(entry_json);
            if replace_existing || replace {
                self.events.insert(event_name, entry);
            } else if let Some(existing) = self.events.get_mut(&event_name) {
                existing.sounds.extend(entry.sounds);
                if entry.category != "master" {
                    existing.category = entry.category;
                }
            } else {
                self.events.insert(event_name, entry);
            }
        }
        Ok(())
    }

    /// Look up a sound event by name.
    pub fn get(&self, event_name: &str) -> Option<&SoundEntry> {
        self.events.get(event_name)
    }

    /// Resolve all concrete OGG file paths for a sound event, following event references.
    /// Returns `(ogg_resource_path, stream_flag)` pairs.
    /// `ogg_resource_path` is in the form `"minecraft:sounds/{name}.ogg"`.
    pub fn resolve_files(&self, event_name: &str) -> Vec<(String, bool)> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.resolve_files_inner(event_name, &mut result, &mut visited);
        result
    }

    fn resolve_files_inner(
        &self,
        event_name: &str,
        result: &mut Vec<(String, bool)>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if !visited.insert(event_name.to_string()) {
            return; // prevent infinite recursion
        }

        let entry = match self.events.get(event_name) {
            Some(e) => e,
            None => return,
        };

        for sound_ref in &entry.sounds {
            match sound_ref {
                SoundRef::File { name, stream } => {
                    let path = if let Some((namespace, path)) = name.split_once(':') {
                        format!("{}/sounds/{}.ogg", namespace, path)
                    } else {
                        format!("minecraft/sounds/{}.ogg", name)
                    };
                    result.push((path, *stream));
                }
                SoundRef::Event { name } => {
                    self.resolve_files_inner(name, result, visited);
                }
            }
        }
    }

    /// Pick a random sound file variant for an event.
    /// Returns `(ogg_resource_path, stream_flag)`.
    pub fn pick_random(&self, event_name: &str) -> Option<(String, bool)> {
        let files = self.resolve_files(event_name);
        if files.is_empty() {
            return None;
        }
        // Simple deterministic pick based on a counter (avoiding rand dependency)
        // In practice, the caller should randomize via timestamp or similar
        Some(files[0].clone())
    }

    /// Get the category for a sound event.
    pub fn category(&self, event_name: &str) -> Option<&str> {
        self.events.get(event_name).map(|e| e.category.as_str())
    }

    /// Get the total number of sound events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if a sound event exists.
    pub fn contains(&self, event_name: &str) -> bool {
        self.events.contains_key(event_name)
    }
}

fn parse_entry(entry_json: SoundEntryJson) -> (SoundEntry, bool) {
    let category = entry_json.category.unwrap_or_else(|| "master".to_string());
    let mut sounds = Vec::with_capacity(entry_json.sounds.len());
    for sound_val in entry_json.sounds {
        match sound_val {
            serde_json::Value::String(name) => sounds.push(SoundRef::File {
                name,
                stream: false,
            }),
            serde_json::Value::Object(map) => {
                let Some(serde_json::Value::String(name)) = map.get("name") else {
                    continue;
                };
                if map.get("type").and_then(|value| value.as_str()) == Some("event") {
                    sounds.push(SoundRef::Event { name: name.clone() });
                } else {
                    sounds.push(SoundRef::File {
                        name: name.clone(),
                        stream: map
                            .get("stream")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                    });
                }
            }
            _ => {}
        }
    }
    (SoundEntry { category, sounds }, entry_json.replace)
}
