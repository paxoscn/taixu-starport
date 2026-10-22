//! File-backed `GameStore`: atomic whole-snapshot replacement.
//!
//! Taixu deliberately ships only a `MemoryStore`; a production host is expected
//! to provide its own store. This is the demo's reference implementation:
//! write a temp file, fsync it, then rename over the snapshot.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use taixu::store::{GameStore, StoreError};
use taixu::GameState;

#[derive(Clone, Debug)]
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl GameStore for FileStore {
    fn load(&self) -> std::result::Result<Option<GameState>, StoreError> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| StoreError::new(format!("invalid snapshot: {error}"))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::new(format!(
                "failed to read {}: {error}",
                self.path.display()
            ))),
        }
    }

    fn save(&self, state: &GameState) -> std::result::Result<(), StoreError> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            StoreError::new(format!(
                "failed to create snapshot dir {}: {error}",
                parent.display()
            ))
        })?;

        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(state)
            .map_err(|error| StoreError::new(format!("failed to encode snapshot: {error}")))?;
        {
            let mut file = fs::File::create(&tmp).map_err(|error| {
                StoreError::new(format!("failed to create {}: {error}", tmp.display()))
            })?;
            file.write_all(&bytes).map_err(|error| {
                StoreError::new(format!("failed to write {}: {error}", tmp.display()))
            })?;
            file.sync_all().map_err(|error| {
                StoreError::new(format!("failed to fsync {}: {error}", tmp.display()))
            })?;
        }
        fs::rename(&tmp, &self.path).map_err(|error| {
            StoreError::new(format!(
                "failed to replace {}: {error}",
                self.path.display()
            ))
        })?;
        Ok(())
    }
}
