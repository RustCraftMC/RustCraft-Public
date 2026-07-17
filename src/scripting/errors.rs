use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ScriptError {
    Io(std::io::Error),
    Manifest(serde_json::Error),
    Lua(mlua::Error),
    InvalidManifest(String),
    InvalidPath(PathBuf),
    DuplicateMod(String),
    UnsupportedApiVersion { mod_id: String, version: u32 },
    PermissionDenied(String),
    ModNotFound(String),
    ReloadDenied(String),
    ProtocolTranslation(String),
    Configuration(String),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Manifest(error) => write!(f, "invalid manifest JSON: {error}"),
            Self::Lua(error) => write!(f, "Lua error: {error}"),
            Self::InvalidManifest(message) => write!(f, "invalid manifest: {message}"),
            Self::InvalidPath(path) => {
                write!(f, "path escapes the mod directory: {}", path.display())
            }
            Self::DuplicateMod(id) => write!(f, "duplicate mod id '{id}'"),
            Self::UnsupportedApiVersion { mod_id, version } => {
                write!(
                    f,
                    "mod '{mod_id}' requests unsupported API version {version}"
                )
            }
            Self::PermissionDenied(permission) => write!(f, "permission denied: {permission}"),
            Self::ModNotFound(id) => write!(f, "mod '{id}' was not found"),
            Self::ReloadDenied(message) => write!(f, "reload denied: {message}"),
            Self::ProtocolTranslation(message) => {
                write!(f, "protocol translation failed: {message}")
            }
            Self::Configuration(message) => write!(f, "configuration error: {message}"),
        }
    }
}

impl std::error::Error for ScriptError {}

impl From<std::io::Error> for ScriptError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ScriptError {
    fn from(value: serde_json::Error) -> Self {
        Self::Manifest(value)
    }
}

impl From<mlua::Error> for ScriptError {
    fn from(value: mlua::Error) -> Self {
        Self::Lua(value)
    }
}

pub type ScriptResult<T> = Result<T, ScriptError>;
