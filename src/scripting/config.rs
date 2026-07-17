//! Declarative, manager-editable mod configuration.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::errors::{ScriptError, ScriptResult};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigChoice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConfigEntryKind {
    Boolean,
    Number {
        min: f64,
        max: f64,
        #[serde(default = "default_step")]
        step: f64,
    },
    Choice {
        options: Vec<ConfigChoice>,
    },
}

fn default_step() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConfigValue {
    Boolean(bool),
    Number(f64),
    Choice(String),
}

impl ConfigValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Number(_) => "number",
            Self::Choice(_) => "choice",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigDefinition {
    pub key: String,
    pub label: String,
    pub description: String,
    pub kind: ConfigEntryKind,
    pub default_value: ConfigValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigEntrySnapshot {
    pub key: String,
    pub label: String,
    pub description: String,
    pub kind: ConfigEntryKind,
    pub value: ConfigValue,
    pub default_value: ConfigValue,
    pub is_default: bool,
}

#[derive(Clone, Debug)]
struct ConfigEntry {
    definition: ConfigDefinition,
    current_value: ConfigValue,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct JsonConfig {
    definitions: Vec<ConfigDefinition>,
    values: BTreeMap<String, ConfigValue>,
}

#[derive(Clone, Debug)]
pub struct ModConfig {
    path: PathBuf,
    entries: BTreeMap<String, ConfigEntry>,
}

pub type SharedModConfig = Rc<RefCell<ModConfig>>;

impl ModConfig {
    pub fn load(root: &Path) -> ScriptResult<Self> {
        let config_path = root.join("config.json");
        let json = if config_path.exists() {
            let text = fs::read_to_string(&config_path).map_err(|error| {
                ScriptError::Configuration(format!(
                    "failed to read config from '{}': {error}",
                    config_path.display()
                ))
            })?;
            if text.trim().is_empty() {
                JsonConfig::default()
            } else {
                serde_json::from_str(&text).map_err(|error| {
                    ScriptError::Configuration(format!(
                        "invalid config json in '{}': {error}",
                        config_path.display()
                    ))
                })?
            }
        } else {
            JsonConfig::default()
        };

        let mut entries = BTreeMap::new();
        for definition in &json.definitions {
            let current_value = json
                .values
                .get(&definition.key)
                .cloned()
                .unwrap_or_else(|| definition.default_value.clone());
            entries.insert(
                definition.key.clone(),
                ConfigEntry {
                    definition: definition.clone(),
                    current_value,
                },
            );
        }

        Ok(Self {
            path: config_path,
            entries,
        })
    }

    fn save(&self) -> ScriptResult<()> {
        let definitions: Vec<ConfigDefinition> = self
            .entries
            .values()
            .map(|entry| entry.definition.clone())
            .collect();
        let mut values = BTreeMap::new();
        for (key, entry) in &self.entries {
            values.insert(key.clone(), entry.current_value.clone());
        }
        let json = JsonConfig {
            definitions,
            values,
        };
        let text = serde_json::to_string_pretty(&json).map_err(|error| {
            ScriptError::Configuration(format!("failed to serialize config: {error}"))
        })?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ScriptError::Configuration(format!("failed to create config directory: {error}"))
            })?;
        }
        fs::write(&self.path, text).map_err(|error| {
            ScriptError::Configuration(format!(
                "failed to write config to '{}': {error}",
                self.path.display()
            ))
        })
    }

    pub fn define_all(&mut self, definitions: Vec<ConfigDefinition>) -> ScriptResult<()> {
        let mut changed = false;
        for definition in definitions {
            if self.entries.contains_key(&definition.key) {
                continue;
            }
            let current_value = definition.default_value.clone();
            self.entries.insert(
                definition.key.clone(),
                ConfigEntry {
                    definition,
                    current_value,
                },
            );
            changed = true;
        }
        if changed {
            self.save()?;
        }
        Ok(())
    }

    pub fn value(&self, key: &str) -> ScriptResult<ConfigValue> {
        self.entries
            .get(key)
            .map(|entry| entry.current_value.clone())
            .ok_or_else(|| {
                ScriptError::Configuration(format!("config entry '{key}' is not defined"))
            })
    }

    pub fn set_value(&mut self, key: &str, value: ConfigValue) -> ScriptResult<()> {
        let entry = self.entries.get_mut(key).ok_or_else(|| {
            ScriptError::Configuration(format!("config entry '{key}' is not defined"))
        })?;

        match (&value, &entry.definition.kind) {
            (ConfigValue::Boolean(_), ConfigEntryKind::Boolean) => {}
            (ConfigValue::Number(number), ConfigEntryKind::Number { min, max, .. }) => {
                if number < min || number > max || number.is_nan() || number.is_infinite() {
                    return Err(ScriptError::Configuration(format!(
                        "config value {} is out of range [{min}, {max}] for '{key}'",
                        number
                    )));
                }
            }
            (ConfigValue::Choice(choice), ConfigEntryKind::Choice { options }) => {
                if !options.iter().any(|option| option.value == *choice) {
                    return Err(ScriptError::Configuration(format!(
                        "config choice '{choice}' is not a valid option for '{key}'"
                    )));
                }
            }
            _ => {
                return Err(ScriptError::Configuration(format!(
                    "config value type mismatch for '{key}': expected {}",
                    match &entry.definition.kind {
                        ConfigEntryKind::Boolean => "boolean",
                        ConfigEntryKind::Number { .. } => "number",
                        ConfigEntryKind::Choice { .. } => "choice",
                    }
                )))
            }
        }

        entry.current_value = value;
        self.save()
    }

    pub fn snapshots(&self) -> Vec<ConfigEntrySnapshot> {
        self.entries
            .values()
            .map(|entry| ConfigEntrySnapshot {
                key: entry.definition.key.clone(),
                label: entry.definition.label.clone(),
                description: entry.definition.description.clone(),
                kind: entry.definition.kind.clone(),
                value: entry.current_value.clone(),
                default_value: entry.definition.default_value.clone(),
                is_default: entry.current_value == entry.definition.default_value,
            })
            .collect()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn mod_config_loads_saved_definitions() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustcraft-config-load-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();

        let mut config = ModConfig::load(&root).unwrap();
        config
            .define_all(vec![ConfigDefinition {
                key: "enabled".into(),
                label: "Enabled".into(),
                description: String::new(),
                kind: ConfigEntryKind::Boolean,
                default_value: ConfigValue::Boolean(true),
            }])
            .unwrap();

        let reloaded = ModConfig::load(&root).unwrap();
        assert_eq!(
            reloaded.value("enabled").unwrap(),
            ConfigValue::Boolean(true)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn set_value_validates_type_and_range() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustcraft-config-validate-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();

        let mut config = ModConfig::load(&root).unwrap();
        config
            .define_all(vec![ConfigDefinition {
                key: "alpha".into(),
                label: "Alpha".into(),
                description: String::new(),
                kind: ConfigEntryKind::Number {
                    min: 0.0,
                    max: 1.0,
                    step: 0.1,
                },
                default_value: ConfigValue::Number(0.5),
            }])
            .unwrap();

        assert!(config.set_value("alpha", ConfigValue::Number(1.5)).is_err());
        assert!(config
            .set_value("alpha", ConfigValue::Boolean(true))
            .is_err());
        config.set_value("alpha", ConfigValue::Number(0.8)).unwrap();
        assert_eq!(config.value("alpha").unwrap(), ConfigValue::Number(0.8));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshots_report_is_default() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustcraft-config-snapshot-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();

        let mut config = ModConfig::load(&root).unwrap();
        config
            .define_all(vec![ConfigDefinition {
                key: "flag".into(),
                label: "Flag".into(),
                description: String::new(),
                kind: ConfigEntryKind::Boolean,
                default_value: ConfigValue::Boolean(true),
            }])
            .unwrap();

        let snapshots = config.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].is_default);

        config
            .set_value("flag", ConfigValue::Boolean(false))
            .unwrap();
        let snapshots = config.snapshots();
        assert!(!snapshots[0].is_default);

        let _ = fs::remove_dir_all(root);
    }
}
