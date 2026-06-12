// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use atendemente_lib::{run_server, AppState};
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
fn save_recovery_file(app: tauri::AppHandle, filename: String, content: String) -> Result<String, String> {
    let path = app
        .dialog()
        .file()
        .add_filter("Recovery", &["json"])
        .set_file_name(&filename)
        .blocking_save_file();

    match path {
        Some(path) => {
            let p = path.into_path()
                .map_err(|_| "Caminho inválido.".to_string())?;
            std::fs::write(&p, &content)
                .map_err(|e| format!("Erro ao salvar arquivo: {}", e))?;
            Ok(p.to_string_lossy().to_string())
        }
        None => Err("Salvamento cancelado.".to_string()),
    }
}

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

    let auth_db = atendemente_lib::db::init_auth_database(&config.auth_database_url)
        .await
        .expect("Failed to initialize auth database");

    let state = Arc::new(AppState {
        config: config.clone(),
        auth_db,
        user_db: RwLock::new(None),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![save_recovery_file])
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
