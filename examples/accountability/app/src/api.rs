use crate::domain::{CheckInReply, HostInfo, NoteReply, NoteStore};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use topcoat::{
    Error, Result,
    router::error::{bad_request, internal_server_error},
};

#[declare::common_accessors]
#[derive(Clone)]
pub enum Api {
    Web {
        notes: NoteStore,
        check_ins: Arc<AtomicUsize>,
    },
    Desktop {
        notes: NoteStore,
        app: tauri::AppHandle,
        client: reqwest::Client,
        check_ins_url: reqwest::Url,
    },
}

impl Api {
    pub fn web(notes: NoteStore) -> Self {
        Self::Web {
            notes,
            check_ins: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn desktop(
        app: tauri::AppHandle,
        hosted: &topcoat_hybrid::HostedApp,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use tauri::Manager;
        let notes = NoteStore::new(app.path().app_data_dir()?.join("platform-note.json"));
        Ok(Self::Desktop {
            notes,
            app,
            client: hosted.client().clone(),
            check_ins_url: hosted.page_url().join("/api/hybrid/check-ins")?,
        })
    }

    pub fn read_note(&self) -> Result<NoteReply> {
        Ok(NoteReply {
            note: self.notes_ref().read().map_err(storage_error)?,
            execution: self.execution().into(),
        })
    }

    pub fn save_note(&self, text: String) -> Result<NoteReply> {
        Ok(NoteReply {
            note: self.notes_ref().save(text).map_err(storage_error)?,
            execution: self.execution().into(),
        })
    }

    pub fn inspect_host(&self) -> Result<HostInfo> {
        match self {
            Self::Web { notes, .. } => Ok(HostInfo {
                name: "Topcoat web host".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                storage_path: notes.path().display().to_string(),
                execution: self.execution().into(),
                window_title: None,
            }),
            Self::Desktop { app, notes, .. } => {
                use tauri::Manager;
                let package = app.package_info();
                let window = app.get_webview_window("main").ok_or_else(|| {
                    internal_server_error(io::Error::other("Main window is unavailable"))
                })?;
                window
                    .set_title("Accountability · Desktop Rust API")
                    .map_err(internal_server_error)?;
                Ok(HostInfo {
                    name: package.name.clone(),
                    version: package.version.to_string(),
                    storage_path: notes.path().display().to_string(),
                    execution: self.execution().into(),
                    window_title: Some(window.title().map_err(internal_server_error)?),
                })
            }
        }
    }

    pub async fn check_ins(&self, increment: bool) -> Result<CheckInReply> {
        match self {
            Self::Web { check_ins, .. } => Ok(CheckInReply {
                count: if increment {
                    check_ins.fetch_add(1, Ordering::Relaxed) + 1
                } else {
                    check_ins.load(Ordering::Relaxed)
                },
                execution: "remote Topcoat server".into(),
            }),
            Self::Desktop {
                client,
                check_ins_url,
                ..
            } => {
                let request = if increment {
                    client.post(check_ins_url.clone())
                } else {
                    client.get(check_ins_url.clone())
                };
                Ok(request
                    .send()
                    .await
                    .map_err(internal_server_error)?
                    .error_for_status()
                    .map_err(internal_server_error)?
                    .json::<CheckInReply>()
                    .await
                    .map_err(internal_server_error)?)
            }
        }
    }

    fn execution(&self) -> &'static str {
        match self {
            Self::Web { .. } => "web server",
            Self::Desktop { .. } => "desktop Rust",
        }
    }
}

fn storage_error(error: io::Error) -> Error {
    if error.kind() == io::ErrorKind::InvalidInput {
        bad_request(error.to_string()).into()
    } else {
        internal_server_error(error).into()
    }
}
