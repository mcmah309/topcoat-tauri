use http::{Request, StatusCode};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tauri::WebviewUrl;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::Router,
    runtime::{RouterBuilderRuntimeExt, procedure},
};
use topcoat_hybrid::{HostedApp, Platform, TransportBuilder};

// Deliberately unrelated to the accountability example.
#[procedure("/api/inventory/count")]
async fn count(cx: &Cx) -> Result<usize> {
    Ok(app_context::<AtomicUsize>(cx).fetch_add(1, Ordering::Relaxed) + 1)
}

struct Host {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Host {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut bytes = Vec::new();
                let mut buffer = [0; 1024];
                while !bytes.windows(4).any(|value| value == b"\r\n\r\n") {
                    let size = socket.read(&mut buffer).await.unwrap();
                    if size == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..size]);
                }
                let request = String::from_utf8(bytes).unwrap();
                let line = request.lines().next().unwrap().to_owned();
                seen.lock().unwrap().push(line.clone());
                let path = line.split_whitespace().nth(1).unwrap();
                let (status, content_type, body, extra) = match path {
                    "/workspace?edition=2" => ("200 OK", "text/html", "<h1>Inventory</h1>", ""),
                    "/_topcoat/assets/app.js?v=3" => {
                        ("200 OK", "text/javascript", "window.inventory = true;", "")
                    }
                    "/_topcoat/assets/redirect.js" => (
                        "302 Found",
                        "text/plain",
                        "",
                        "Location: /workspace?edition=2\r\n",
                    ),
                    _ => ("404 Not Found", "text/plain", "Missing", ""),
                };
                let body = if line.starts_with("HEAD ") { "" } else { body };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[test]
fn accepts_trusted_hosts_and_maps_any_entry_path() {
    for url in [
        "https://ui.example/workspace",
        "http://localhost:3002/workspace",
        "http://127.0.0.1/workspace",
        "http://[::1]/workspace",
    ] {
        assert!(HostedApp::new(url).is_ok(), "{url}");
    }
    for url in [
        "not a URL",
        "http://ui.example/workspace",
        "file:///tmp/page.html",
        "https://user:password@ui.example/workspace",
        "https://ui.example/workspace#section",
    ] {
        assert!(HostedApp::new(url).is_err(), "{url}");
    }
    let app = HostedApp::new("https://ui.example/workspace?edition=2").unwrap();
    let WebviewUrl::CustomProtocol(url) = app.webview_url() else {
        panic!("expected custom protocol");
    };
    assert_eq!(url.as_str(), "topcoat://localhost/workspace?edition=2");
}

#[tokio::test]
async fn fetches_hosted_content_but_executes_procedures_locally_on_all_origins() {
    let host = Host::start().await;
    let app = HostedApp::new(&format!("{}/workspace?edition=2", host.url)).unwrap();
    for (platform, origin) in [
        (Platform::Scheme, "topcoat://localhost"),
        (Platform::HttpSubdomain, "http://topcoat.localhost"),
        (Platform::HttpsSubdomain, "https://topcoat.localhost"),
    ] {
        let local = Router::builder()
            .route(count)
            .app_context(AtomicUsize::new(0))
            .runtime();
        let session = TransportBuilder::new(app.router(local))
            .session(platform)
            .unwrap();
        let page = session
            .serve(
                Request::get(format!("{origin}/workspace?edition=2"))
                    .body(vec![])
                    .unwrap(),
            )
            .await;
        assert_eq!(page.status(), StatusCode::OK);
        assert_eq!(page.headers()["content-type"], "text/html");
        assert_eq!(page.headers()["cache-control"], "no-store");
        assert_eq!(page.body().as_ref(), b"<h1>Inventory</h1>");
        let asset = session
            .serve(
                Request::get(format!("{origin}/_topcoat/assets/app.js?v=3"))
                    .body(vec![])
                    .unwrap(),
            )
            .await;
        assert_eq!(asset.status(), StatusCode::OK);
        assert_eq!(asset.headers()["content-type"], "text/javascript");
        assert_eq!(asset.body().as_ref(), b"window.inventory = true;");
        for total in [1, 2] {
            let result = session
                .serve(
                    Request::post(format!("{origin}/api/inventory/count"))
                        .header("origin", origin)
                        .header("content-type", "application/json")
                        .body(b"[]".to_vec())
                        .unwrap(),
                )
                .await;
            assert_eq!(result.status(), StatusCode::OK);
            assert!(String::from_utf8_lossy(result.body()).contains(&format!(r#""v":"{total}""#)));
        }
        let forbidden = session
            .serve(
                Request::post(format!("{origin}/api/inventory/count"))
                    .header("origin", "https://foreign.example")
                    .header("content-type", "application/json")
                    .body(b"[]".to_vec())
                    .unwrap(),
            )
            .await;
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        let missing = session
            .serve(
                Request::get(format!("{origin}/unregistered-page"))
                    .body(vec![])
                    .unwrap(),
            )
            .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
    let requests = host.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        6,
        "only hosted pages and assets should reach the server"
    );
    assert!(
        requests
            .iter()
            .all(|line| line == "GET /workspace?edition=2 HTTP/1.1"
                || line == "GET /_topcoat/assets/app.js?v=3 HTTP/1.1")
    );
}

#[tokio::test]
async fn upstream_failures_are_visible_while_local_procedures_work_offline() {
    let host = Host::start().await;
    let app = HostedApp::new(&format!("{}/workspace?edition=2", host.url)).unwrap();
    let local = Router::builder()
        .route(count)
        .app_context(AtomicUsize::new(0))
        .runtime();
    let session = TransportBuilder::new(app.router(local))
        .session(Platform::Scheme)
        .unwrap();
    for path in ["missing.js", "redirect.js"] {
        let result = session
            .serve(
                Request::get(format!("topcoat://localhost/_topcoat/assets/{path}"))
                    .body(vec![])
                    .unwrap(),
            )
            .await;
        assert_eq!(result.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    assert_eq!(
        host.requests.lock().unwrap().len(),
        2,
        "redirects must not be followed"
    );
    drop(host);
    // Let cancellation release the listener before testing connection failure.
    tokio::task::yield_now().await;
    let page = session
        .serve(
            Request::get("topcoat://localhost/workspace")
                .body(vec![])
                .unwrap(),
        )
        .await;
    assert_eq!(page.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let local = session
        .serve(
            Request::post("topcoat://localhost/api/inventory/count")
                .header("origin", "topcoat://localhost")
                .header("content-type", "application/json")
                .body(b"[]".to_vec())
                .unwrap(),
        )
        .await;
    assert_eq!(local.status(), StatusCode::OK);
}
