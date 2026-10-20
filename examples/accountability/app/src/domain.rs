//! Shared data and storage logic; no dependency on Topcoat, Tauri, or a transport.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_NOTE_CHARS: usize = 2000;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Note {
    pub text: String,
    pub saved_at: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NoteReply {
    pub note: Note,
    pub execution: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HostInfo {
    pub name: String,
    pub version: String,
    pub storage_path: String,
    pub execution: String,
    pub window_title: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CheckInReply {
    pub count: usize,
    pub execution: String,
}

/// Each host supplies its own path. A single managed instance serializes writes.
#[derive(Clone)]
pub struct NoteStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl NoteStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> io::Result<Note> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| io::Error::other("Note store unavailable"))?;
        match fs::read(&self.path) {
            Ok(data) => serde_json::from_slice(&data).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Note::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, text: String) -> io::Result<Note> {
        if text.chars().count() > MAX_NOTE_CHARS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Notes must be 2000 characters or fewer.",
            ));
        }
        let _guard = self
            .lock
            .lock()
            .map_err(|_| io::Error::other("Note store unavailable"))?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let note = Note {
            text,
            saved_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
        };
        let bytes = serde_json::to_vec_pretty(&note).map_err(io::Error::other)?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, &self.path)?;
        Ok(note)
    }
}
