use accountability_app::domain::NoteStore;
use accountability_app::router_with_note_store;
use http_body_util::BodyExt;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use topcoat::{
    asset::AssetBundle,
    router::{Body, Router, request::Request},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hybrid-api-{}-{}",
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

fn router(path: PathBuf) -> Router {
    let executable = std::env::current_exe().unwrap();
    let profile = executable.parent().unwrap().parent().unwrap();
    router_with_note_store(
        AssetBundle::load_dir(profile.join("assets"))
            .expect("run topcoat asset bundle --package accountability-web first"),
        NoteStore::new(path),
    )
    .build()
}

async fn request(
    router: &Router,
    method: &str,
    path: &str,
    json: &str,
) -> (u16, serde_json::Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost")
        .header("origin", "http://localhost")
        .header("content-type", "application/json")
        .body(Body::from(json.to_owned()))
        .unwrap();
    let response = router.handle(request).await;
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn web_note_api_persists_and_validates_with_an_explicit_execution_location() {
    let directory = Directory::new();
    let path = directory.0.join("server-note.json");
    let app = router(path.clone());
    let (status, initial) = request(&app, "GET", "/api/hybrid/note", "").await;
    assert_eq!(status, 200);
    assert_eq!(initial["note"]["text"], "");
    let (status, saved) = request(
        &app,
        "POST",
        "/api/hybrid/note",
        r#"{"text":"Written on the server"}"#,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(saved["execution"], "web server");
    assert_eq!(saved["note"]["text"], "Written on the server");
    let long = serde_json::json!({"text": "x".repeat(2001)}).to_string();
    assert_eq!(
        request(&app, "POST", "/api/hybrid/note", &long).await.0,
        400
    );
    drop(app);
    let restarted = router(path);
    assert_eq!(
        request(&restarted, "GET", "/api/hybrid/note", "").await.1,
        saved
    );
    let (_, info) = request(&restarted, "GET", "/api/hybrid/host", "").await;
    assert_eq!(info["execution"], "web server");
    assert!(info["window_title"].is_null());
}

#[tokio::test]
async fn remote_service_uses_one_shared_server_total() {
    let directory = Directory::new();
    let app = router(directory.0.join("note.json"));
    assert_eq!(
        request(&app, "GET", "/api/hybrid/check-ins", "").await.1["count"],
        0
    );
    for count in [1, 2] {
        let (status, result) = request(&app, "POST", "/api/hybrid/check-ins", "").await;
        assert_eq!(status, 200);
        assert_eq!(result["count"], count);
        assert_eq!(result["execution"], "remote Topcoat server");
    }
    assert_eq!(
        request(&app, "GET", "/api/hybrid/check-ins", "").await.1["count"],
        2
    );
}

#[tokio::test]
async fn rust_ui_procedures_use_the_host_services_and_shared_cloud_state() {
    let directory = Directory::new();
    let app = router(directory.0.join("note.json"));
    request(
        &app,
        "POST",
        "/api/hybrid/note",
        r#"{"text":"A persisted procedure note"}"#,
    )
    .await;
    let (status, note) = request(&app, "POST", "/api/hybrid/platform/read", "[]").await;
    assert_eq!(status, 200);
    assert!(note.to_string().contains("A persisted procedure note"));
    assert!(note.to_string().contains("Loaded by web server"));
    let (status, host) = request(&app, "POST", "/api/hybrid/platform/inspect", "[]").await;
    assert_eq!(status, 200);
    assert!(host.to_string().contains("Topcoat web host"));
    let (status, cloud) = request(&app, "POST", "/api/hybrid/cloud/check-in", "[]").await;
    assert_eq!(status, 200);
    assert!(cloud.to_string().contains("remote Topcoat server"));
    assert_eq!(
        request(&app, "GET", "/api/hybrid/check-ins", "").await.1["count"],
        1
    );
}
