use std::fs;
use std::path::{Path, PathBuf};

use super::errors::{ScriptError, ScriptResult};
use super::manifest::ModManifest;

const MAX_ENTRYPOINT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct LoadedMod {
    pub root: PathBuf,
    pub manifest: ModManifest,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct ModLoader {
    mods_dir: PathBuf,
}

impl ModLoader {
    pub fn new(mods_dir: impl Into<PathBuf>) -> Self {
        Self {
            mods_dir: mods_dir.into(),
        }
    }

    pub fn mods_dir(&self) -> &Path {
        &self.mods_dir
    }

    pub fn discover(&self) -> ScriptResult<Vec<LoadedMod>> {
        if !self.mods_dir.exists() {
            fs::create_dir_all(&self.mods_dir)?;
            return Ok(Vec::new());
        }
        let mut roots = fs::read_dir(&self.mods_dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        roots.sort();
        roots.into_iter().map(|root| self.load_root(root)).collect()
    }

    pub fn find_by_id(&self, id: &str) -> ScriptResult<LoadedMod> {
        self.discover()?
            .into_iter()
            .find(|loaded| loaded.manifest.id.as_str() == id)
            .ok_or_else(|| ScriptError::ModNotFound(id.to_owned()))
    }

    fn load_root(&self, root: PathBuf) -> ScriptResult<LoadedMod> {
        let manifest = ModManifest::parse(&fs::read_to_string(root.join("manifest.json"))?)?;
        let entrypoint = root.join(&manifest.entrypoints.client);
        let canonical_root = root.canonicalize()?;
        let canonical_entrypoint = entrypoint.canonicalize()?;
        if !canonical_entrypoint.starts_with(&canonical_root) {
            return Err(ScriptError::InvalidPath(entrypoint));
        }
        let metadata = canonical_entrypoint.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_ENTRYPOINT_BYTES {
            return Err(ScriptError::InvalidManifest(format!(
                "entrypoint must be a file no larger than {MAX_ENTRYPOINT_BYTES} bytes"
            )));
        }
        let source = fs::read_to_string(canonical_entrypoint)?;
        Ok(LoadedMod {
            root: canonical_root,
            manifest,
            source,
        })
    }
}
