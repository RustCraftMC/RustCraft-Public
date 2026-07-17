use serde::Deserialize;
use std::fmt;
use std::path::{Component, Path};

use super::errors::{ScriptError, ScriptResult};
use super::permissions::Permission;
use super::API_VERSION;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModId(String);

impl ModId {
    pub fn parse(value: impl Into<String>) -> ScriptResult<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            });
        if !valid {
            return Err(ScriptError::InvalidManifest(format!(
                "mod id '{value}' must contain only lowercase ASCII letters, digits, '_' or '-'"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ModId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModEntrypoints {
    pub client: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModManifest {
    pub id: ModId,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub entrypoints: ModEntrypoints,
    #[serde(default)]
    pub permissions: Vec<Permission>,
}

impl ModManifest {
    pub fn parse(json: &str) -> ScriptResult<Self> {
        let manifest: Self = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> ScriptResult<()> {
        if self.name.trim().is_empty() || self.name.len() > 128 {
            return Err(ScriptError::InvalidManifest(
                "name must contain between 1 and 128 characters".into(),
            ));
        }
        if self.version.trim().is_empty() || self.version.len() > 64 {
            return Err(ScriptError::InvalidManifest(
                "version must contain between 1 and 64 characters".into(),
            ));
        }
        if self.api_version != API_VERSION {
            return Err(ScriptError::UnsupportedApiVersion {
                mod_id: self.id.to_string(),
                version: self.api_version,
            });
        }
        validate_relative_path(Path::new(&self.entrypoints.client))?;
        Ok(())
    }
}

pub fn validate_relative_path(path: &Path) -> ScriptResult<()> {
    let valid = !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        return Err(ScriptError::InvalidPath(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_manifest() {
        let manifest = ModManifest::parse(
            r#"{
                "id":"old_animations","name":"Old Animations","version":"1.0.0",
                "api_version":1,"entrypoints":{"client":"scripts/client.lua"},
                "permissions":["animation.modify"]
            }"#,
        )
        .unwrap();
        assert_eq!(manifest.id.as_str(), "old_animations");
    }

    #[test]
    fn rejects_entrypoint_escape() {
        let error = ModManifest::parse(
            r#"{
                "id":"bad","name":"Bad","version":"1","api_version":1,
                "entrypoints":{"client":"../escape.lua"}
            }"#,
        )
        .unwrap_err();
        assert!(matches!(error, ScriptError::InvalidPath(_)));
    }
}
