use crate::api::Api;
use std::{env, error::Error};
use tauri::WebviewWindowBuilder;
use topcoat_hybrid::HostedApp;

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let hosted = HostedApp::new(
        &env::var("TOPCOAT_APP_URL").unwrap_or_else(|_| "http://127.0.0.1:3002/hybrid".into()),
    )?;
    let api = Api::desktop(app.handle().clone(), &hosted)?;
    let procedures = crate::hybrid_procedures(api);
    app.handle().plugin(hosted.plugin(procedures)?)?;
    WebviewWindowBuilder::new(app, "main", hosted.webview_url())
        .title("Accountability · Hybrid desktop")
        .inner_size(920.0, 980.0)
        .build()?;
    println!(
        "Hosted UI fetched from {}; platform procedures execute in desktop Rust.",
        hosted.page_url()
    );
    Ok(())
}
