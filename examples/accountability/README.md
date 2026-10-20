# Accountability example

One Rust Topcoat UI and one application API, running in a browser and Tauri.

## Crates

| Directory | Responsibility |
| --- | --- |
| [app](app/src/lib.rs) | All application code: UI, procedures, data types, validation, persistence, and web/native behavior. |
| [desktop](desktop/src/main.rs) | Desktop launcher. Starts Tauri and calls `accountability_app::desktop::setup`. |
| [web](web/src/main.rs) | Web launcher. Starts the HTTP server with `accountability_app::router`. |

## How they fit together

`web` serves the page from `app`. The browser calls procedures on that server.

`app` contains [shared storage and types](app/src/domain.rs), an [Api enum](app/src/api.rs) with `Web` and `Desktop` variants, and [desktop window setup](app/src/desktop.rs). Each variant owns its `NoteStore`. Each API method decides whether to execute locally or delegate. Both variants are always compiled into the app crate.

The desktop setup uses [`HostedApp`](../../crates/topcoat-hybrid/README.md) to fetch the same page and assets, then handles its procedure calls locally. Both hosts register the same procedures with an `Api` configured for that host.

| Action | Browser | Desktop |
| --- | --- | --- |
| Save/reload note | File on the server | File in the app's local data directory |
| Inspect host | Server information | Tauri app metadata and native window title |
| Check in / refresh total | Server counter | Rust HTTP request to that same server counter |

The check-in counter resets when the server restarts. Notes persist in their respective files.

## Run

From the repository root, start the web host:

```sh
PORT=3002 topcoat dev --package accountability-web
```

Open <http://127.0.0.1:3002/hybrid>. In another terminal:

```sh
cargo run -p accountability-desktop
```

Try saving different notes in each window, inspecting the host, and refreshing the shared counter.
