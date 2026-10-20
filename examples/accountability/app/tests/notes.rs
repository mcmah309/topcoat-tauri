use accountability_app::domain::{MAX_NOTE_CHARS, NoteStore};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hybrid-note-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn notes_survive_new_store_instances_and_host_paths_are_independent() {
    let directory = Directory::new();
    let web = NoteStore::new(directory.0.join("server/note.json"));
    let desktop = NoteStore::new(directory.0.join("desktop/note.json"));
    assert_eq!(web.read().unwrap().text, "");
    let note = web.save("A server note".into()).unwrap();
    assert!(note.saved_at.is_some());
    desktop.save("A local note".into()).unwrap();
    assert_eq!(NoteStore::new(web.path()).read().unwrap(), note);
    assert_eq!(desktop.read().unwrap().text, "A local note");
    assert_eq!(web.read().unwrap().text, "A server note");
}

#[test]
fn invalid_note_does_not_replace_saved_data() {
    let directory = Directory::new();
    let store = NoteStore::new(directory.0.join("note.json"));
    let saved = store.save("Keep this note".into()).unwrap();
    assert!(store.save("x".repeat(MAX_NOTE_CHARS + 1)).is_err());
    assert_eq!(store.read().unwrap(), saved);
    assert_eq!(
        store
            .save("🌱".repeat(MAX_NOTE_CHARS))
            .unwrap()
            .text
            .chars()
            .count(),
        MAX_NOTE_CHARS
    );
}

#[test]
fn unreadable_and_unwritable_stores_report_errors() {
    let directory = Directory::new();
    let path = directory.0.join("note.json");
    fs::write(&path, "invalid json").unwrap();
    assert!(NoteStore::new(&path).read().is_err());
    let blocked = NoteStore::new(path.join("note.json"));
    assert!(blocked.read().is_err());
    assert!(blocked.save("Cannot write here".into()).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "invalid json");
}
