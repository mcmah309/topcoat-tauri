#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .setup(accountability_app::desktop::setup)
        .run(tauri::generate_context!())?;
    Ok(())
}
