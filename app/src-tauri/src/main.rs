//! Settings Migrator desktop app: the screens are React (in `app/src`),
//! and every action goes through the commands in `commands.rs`.

// No console window behind the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    let state = match commands::AppState::from_env() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::scan_apps,
            commands::plan_capture,
            commands::save_bundle,
            commands::preview_restore,
            commands::restore,
            commands::preview_undo,
            commands::undo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
