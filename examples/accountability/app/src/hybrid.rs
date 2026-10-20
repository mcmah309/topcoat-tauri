use crate::api::Api;
use crate::domain::{CheckInReply, HostInfo, NoteReply};
use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, page, route},
    runtime::{Event, procedure, record, signal},
    view::{View, view},
};
use topcoat_asset::asset;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveNote {
    text: String,
}

#[route(GET "/api/hybrid/note")]
async fn read_note(cx: &Cx) -> Result<Json<NoteReply>> {
    Ok(Json(app_context::<Api>(cx).read_note()?))
}

#[route(POST "/api/hybrid/note")]
async fn save_note(cx: &Cx, Json(input): Json<SaveNote>) -> Result<Json<NoteReply>> {
    Ok(Json(app_context::<Api>(cx).save_note(input.text)?))
}

#[route(GET "/api/hybrid/host")]
async fn inspect_host(cx: &Cx) -> Result<Json<HostInfo>> {
    Ok(Json(app_context::<Api>(cx).inspect_host()?))
}

#[route(GET "/api/hybrid/check-ins")]
async fn check_ins(cx: &Cx) -> Result<Json<CheckInReply>> {
    Ok(Json(app_context::<Api>(cx).check_ins(false).await?))
}

#[route(POST "/api/hybrid/check-ins")]
async fn check_in(cx: &Cx) -> Result<Json<CheckInReply>> {
    Ok(Json(app_context::<Api>(cx).check_ins(true).await?))
}

#[record]
#[derive(Clone)]
pub struct NoteState {
    pub text: String,
    pub status: String,
    pub execution: String,
    pub error: String,
}

#[record]
#[derive(Clone)]
pub struct CloudState {
    pub count: usize,
    pub status: String,
    pub error: String,
}

#[record]
#[derive(Clone)]
pub struct HostState {
    pub name: String,
    pub version: String,
    pub storage_path: String,
    pub execution: String,
    pub window_title: String,
    pub error: String,
}

fn note_state(reply: Result<NoteReply>, action: &str) -> NoteState {
    match reply {
        Ok(reply) => NoteState {
            text: reply.note.text,
            status: format!("{action} by {}", reply.execution),
            execution: reply.execution,
            error: String::new(),
        },
        Err(error) => NoteState {
            text: String::new(),
            status: "The note operation did not finish.".into(),
            execution: String::new(),
            error: error.to_string(),
        },
    }
}

// Explicit paths form a stable contract between the independently hosted page
// and the web or desktop Rust implementation. Topcoat generates the transport.
#[procedure("/api/hybrid/platform/read")]
pub async fn platform_read(cx: &Cx) -> Result<NoteState> {
    Ok(note_state(app_context::<Api>(cx).read_note(), "Loaded"))
}
#[procedure("/api/hybrid/platform/save")]
pub async fn platform_save(cx: &Cx, text: String) -> Result<NoteState> {
    Ok(note_state(app_context::<Api>(cx).save_note(text), "Saved"))
}
#[procedure("/api/hybrid/platform/inspect")]
pub async fn platform_inspect(cx: &Cx) -> Result<HostState> {
    Ok(match app_context::<Api>(cx).inspect_host() {
        Ok(info) => HostState {
            name: info.name,
            version: info.version,
            storage_path: info.storage_path,
            execution: info.execution,
            window_title: info
                .window_title
                .unwrap_or_else(|| "No native window in browser mode".into()),
            error: String::new(),
        },
        Err(error) => HostState {
            name: String::new(),
            version: String::new(),
            storage_path: String::new(),
            execution: String::new(),
            window_title: String::new(),
            error: error.to_string(),
        },
    })
}
async fn cloud_state(cx: &Cx, increment: bool) -> CloudState {
    match app_context::<Api>(cx).check_ins(increment).await {
        Ok(reply) => CloudState {
            count: reply.count,
            status: format!("Executed on the {}.", reply.execution),
            error: String::new(),
        },
        Err(error) => CloudState {
            count: 0,
            status: String::new(),
            error: error.to_string(),
        },
    }
}
#[procedure("/api/hybrid/cloud/read")]
pub async fn cloud_read(cx: &Cx) -> Result<CloudState> {
    Ok(cloud_state(cx, false).await)
}
#[procedure("/api/hybrid/cloud/check-in")]
pub async fn cloud_check_in(cx: &Cx) -> Result<CloudState> {
    Ok(cloud_state(cx, true).await)
}

#[page]
async fn home(cx: &Cx) -> Result<impl View> {
    let text = signal(cx, String::new);
    let note = signal(cx, || NoteState {
        text: String::new(),
        status: "Click Reload note to load your saved note, or write a new one.".into(),
        execution: "Rust platform API".into(),
        error: String::new(),
    });
    let cloud = signal(cx, || CloudState {
        count: 0,
        status: "Click Refresh total to load the shared service.".into(),
        error: String::new(),
    });
    let host = signal(cx, || HostState {
        name: String::new(),
        version: String::new(),
        storage_path: String::new(),
        execution: String::new(),
        window_title: String::new(),
        error: String::new(),
    });
    let note_busy = signal(cx, || false);
    let cloud_busy = signal(cx, || false);
    let host_busy = signal(cx, || false);
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Accountability · Hybrid APIs"</title>
                <link rel="stylesheet" href=(asset!("./hybrid.css"))>
                topcoat::runtime::script()
            </head>
            <body>
                <main>
                    <header>
                        <p class="eyebrow">"One Topcoat UI · Rust APIs in both hosts"</p>
                        <h1>"An API that feels at home."</h1>
                        <p class="intro">"The same Rust view and procedure calls run in the browser and desktop app. One API handles local notes, host details, and shared check-ins."</p>
                        <p class="mode" id="runtime-mode" role="status">$(note.get().execution)</p>
                    </header>
                    <section aria-labelledby="note-heading">
                        <div class="section-heading"><span class="step">"01"</span><div><h2 id="note-heading">"Platform storage"</h2><p>"Same Rust procedure. A different storage location in each host."</p></div></div>
                        <label for="note-text">"Your note"</label>
                        <textarea id="note-text" maxlength="2000" rows="4" placeholder="Write a small intention for today…" :value=$(text.get()) @input=$(|event: Event| text.set(event.target.value))></textarea>
                        <div class="actions">
                            <button id="save-note" type="button" :disabled=$(note_busy.get()) @click=$(async |_e| {
                                note_busy.set(true);
                                note.set(platform_save(text.get()).await);
                                note_busy.set(false);
                            })>"Save note"</button>
                            <button id="reload-note" class="secondary" type="button" :disabled=$(note_busy.get()) @click=$(async |_e| {
                                note_busy.set(true);
                                let result = platform_read().await;
                                if result.error.is_empty() { text.set(result.text.to_owned()); }
                                note.set(result);
                                note_busy.set(false);
                            })>"Reload note"</button>
                        </div>
                        <p class="status" id="note-status" role="status">$(note.get().status)</p>
                        <p class="error" id="note-error" role="alert">$(note.get().error)</p>
                        <p class="hint">"Browser: a file on the web server. Desktop: a file in this app’s local data directory."</p>
                    </section>
                    <section aria-labelledby="cloud-heading">
                        <div class="section-heading"><span class="step">"02"</span><div><h2 id="cloud-heading">"Shared remote service"</h2><p>"This API stays on the hosted server for everyone."</p></div></div>
                        <p class="total">"Check-ins: " <output id="cloud-count">$(cloud.get().count)</output></p>
                        <div class="actions">
                            <button id="cloud-check-in" type="button" :disabled=$(cloud_busy.get()) @click=$(async |_e| { cloud_busy.set(true); cloud.set(cloud_check_in().await); cloud_busy.set(false); })>"Check in remotely"</button>
                            <button id="refresh-cloud" class="secondary" type="button" :disabled=$(cloud_busy.get()) @click=$(async |_e| { cloud_busy.set(true); cloud.set(cloud_read().await); cloud_busy.set(false); })>"Refresh total"</button>
                        </div>
                        <p class="status" id="cloud-status" role="status">$(cloud.get().status)</p>
                        <p class="error" id="cloud-error" role="alert">$(cloud.get().error)</p>
                        <p class="hint">"Open this page in a browser and in Tauri to see the same server total. Refresh to see another window’s check-in."</p>
                    </section>
                    <section aria-labelledby="host-heading">
                        <div class="section-heading"><span class="step">"03"</span><div><h2 id="host-heading">"Inspect this host"</h2><p>"The desktop implementation calls Tauri APIs directly in Rust."</p></div></div>
                        <button id="inspect-host" type="button" :disabled=$(host_busy.get()) @click=$(async |_e| { host_busy.set(true); host.set(platform_inspect().await); host_busy.set(false); })>"Inspect host"</button>
                        <dl id="host-details" :hidden=$(host.get().execution.is_empty())>
                            <dt>"Executed in"</dt><dd id="host-execution">$(host.get().execution)</dd>
                            <dt>"Application"</dt><dd id="host-name">$(host.get().name)</dd>
                            <dt>"Version"</dt><dd id="host-version">$(host.get().version)</dd>
                            <dt>"Note file"</dt><dd id="host-path">$(host.get().storage_path)</dd>
                            <dt>"Window title"</dt><dd id="host-title">$(host.get().window_title)</dd>
                        </dl>
                        <p class="error" id="host-error" role="alert">$(host.get().error)</p>
                        <p class="hint">"Desktop Rust resolves the app data directory, reads package metadata, and changes the native window title. Web Rust reports the server’s details."</p>
                    </section>
                    <footer>"UI actions, platform services, and Tauri calls are authored in Rust. Topcoat generates its browser runtime."</footer>
                </main>
            </body>
        </html>
    })
}
