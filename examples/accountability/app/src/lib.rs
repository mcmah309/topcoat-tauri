pub mod api;
pub mod desktop;
pub mod domain;
mod hybrid;

use api::Api;
use domain::NoteStore;

use topcoat::{
    router::{RouterBuilder, RouterBuilderDiscoverExt, module_router},
    runtime::RouterBuilderRuntimeExt,
};
use topcoat_asset::{AssetBundle, RouterBuilderAssetExt};

/// Native host registers the same stable procedures with native Rust services.
pub fn hybrid_procedures(api: Api) -> RouterBuilder {
    topcoat::router::Router::builder()
        .route(hybrid::platform_read)
        .route(hybrid::platform_save)
        .route(hybrid::platform_inspect)
        .route(hybrid::cloud_read)
        .route(hybrid::cloud_check_in)
        .app_context(api)
        .runtime()
}

/// Hosted UI, assets, platform procedures, and shared remote APIs.
pub fn router(assets: AssetBundle) -> RouterBuilder {
    let path =
        std::env::var("WEB_NOTE_PATH").unwrap_or_else(|_| "data/hybrid-web-note.json".into());
    router_with_note_store(assets, NoteStore::new(path))
}

/// Inject a storage location for the hosted platform API.
pub fn router_with_note_store(assets: AssetBundle, notes: NoteStore) -> RouterBuilder {
    module_router!()
        .discover()
        .app_context(Api::web(notes))
        .assets(assets)
        .runtime()
}
