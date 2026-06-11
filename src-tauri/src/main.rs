// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use atendemente_lib::{run_server, AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = atendemente_lib::config::AppConfig::from_env();
    atendemente_lib::crypto::set_pepper(&config.master_pepper);

    let db = atendemente_lib::db::init_database(&config.database_url)
        .await
        .expect("Failed to initialize database");

    let state = Arc::new(AppState {
        config: config.clone(),
        db,
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            let state = state.clone();

            tokio::spawn(async move {
                run_server(state, handle).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
