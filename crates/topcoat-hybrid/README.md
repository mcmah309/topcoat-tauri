# topcoat-hybrid

One hosted Topcoat UI for web and Tauri. Shared procedures call your Rust API; its methods handle local work and remote delegation.

## Add it

```toml
[dependencies]
topcoat-hybrid = { path = "../topcoat-hybrid/crates/topcoat-hybrid" }
topcoat = { version = "=0.10.0", default-features = false, features = ["router", "runtime", "view"] }
tauri = "2.12.4"
```

## Desktop setup

In an existing Tauri project, define a procedure shared with your hosted UI:

```rust
use tauri::{Manager, WebviewWindowBuilder};
use topcoat::{
    context::Cx,
    router::Router,
    runtime::{procedure, RouterBuilderRuntimeExt},
};
use topcoat_hybrid::HostedApp;

#[procedure("/api/hello")]
pub async fn hello(_cx: &Cx) -> topcoat::Result<String> {
    Ok("Hello from Rust".into())
}

pub fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let hosted = HostedApp::new("https://example.com/app")?;
    let procedures = Router::builder().route(hello).runtime();

    app.handle().plugin(hosted.plugin(procedures)?)?;
    WebviewWindowBuilder::new(app, "main", hosted.webview_url())
        .title("My app")
        .build()?;
    Ok(())
}
```

Pass `install` to `tauri::Builder::setup`. The hosted UI calls `hello().await` through Topcoat.

Tauri API calls live in Rust. Always-remote APIs use `hosted.client()`.

## Run the example

From the repository root, in separate terminals:

```sh
cargo install topcoat-cli --version 0.10.0 --locked
PORT=3002 topcoat dev --package accountability-web
```

```sh
cargo run -p accountability-desktop
```

Browser: <http://127.0.0.1:3002/hybrid>.

## Build and test

```sh
topcoat asset bundle --package accountability-web
cargo test --workspace --all-features
cargo build -p accountability-desktop
```

[Example overview](../../examples/accountability/README.md) · [Example UI](../../examples/accountability/app/src/hybrid.rs) · [Example API](../../examples/accountability/app/src/api.rs) · [Architecture](../../docs/architecture.md)

HTTPS required outside loopback development. Desktop responses are buffered; streaming and WebSockets are unsupported. Keep hosted UI and installed procedure schemas compatible.
