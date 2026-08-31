// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use atendemente_lib::{run_server, AppState};
use tokio::sync::RwLock;
use tauri_plugin_dialog::DialogExt;

/// Abre o dialogo nativo "salvar como" e grava os bytes onde a pessoa escolher.
///
/// Substitui o caminho web (`<a download>` + `showSaveFilePicker`), que na pratica
/// nao funcionava no app instalado: `showSaveFilePicker` so existe em navegadores
/// Chromium, entao no WebKitGTK do Linux — e no WKWebView do macOS — a chamada caia
/// no fallback do link, que grava sem perguntar nada e sem dizer onde. A tela
/// anunciava "baixado com sucesso" e a pessoa ficava sem saber onde o arquivo foi
/// parar. Para a chave de recuperacao isso e grave: e o unico caminho de volta a
/// conta, e ela sumia num diretorio qualquer.
///
/// Devolve o caminho gravado, para que a interface possa mostra-lo.
/// `async` de proposito, e com o dialogo numa thread de bloqueio.
///
/// `blocking_save_file` trava se rodar na thread principal: ele bloqueia esperando a
/// escolha, e quem processa os eventos do dialogo e justamente o event loop daquela
/// thread — deadlock, sem erro e sem janela. Comandos Tauri **sincronos** rodam na
/// main thread, entao a versao anterior deste comando (`save_recovery_file`) nunca
/// poderia ter funcionado; talvez por isso o frontend jamais a chamasse.
#[tauri::command]
async fn cmd_save_file(
    app: tauri::AppHandle,
    filename: String,
    content_base64: String,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    let dados = BASE64
        .decode(&content_base64)
        .map_err(|e| format!("Conteudo invalido: {}", e))?;

    let nome = filename.clone();
    let escolhido = tauri::async_runtime::spawn_blocking(move || {
        let extensao = std::path::Path::new(&nome)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let mut dialogo = app.dialog().file().set_file_name(&nome);
        if !extensao.is_empty() {
            dialogo = dialogo.add_filter(extensao.to_uppercase(), &[extensao.as_str()]);
        }
        dialogo.blocking_save_file()
    })
    .await
    .map_err(|e| format!("Erro ao abrir o seletor de arquivo: {}", e))?
    .ok_or_else(|| "cancelado".to_string())?;

    let caminho = escolhido
        .into_path()
        .map_err(|_| "Caminho invalido.".to_string())?;

    std::fs::write(&caminho, &dados).map_err(|e| format!("Erro ao salvar arquivo: {}", e))?;
    Ok(caminho.to_string_lossy().to_string())
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
        user_dbs: RwLock::new(HashMap::new()),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            cmd_save_file,
            atendemente_lib::commands::cmd_export_patient_zip,
            atendemente_lib::commands::cmd_list_files_by_appointment,
            atendemente_lib::commands::cmd_download_file,
            atendemente_lib::commands::cmd_upload_file_content,
            atendemente_lib::commands::cmd_confirm_file_upload,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let state = state.clone();
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();

            tauri::async_runtime::spawn(async move {
                if let Err(e) = run_server(state, Some(handle), Some(ready_tx)).await {
                    tracing::error!("Server error: {}", e);
                }
            });

            let _ = ready_rx.recv_timeout(Duration::from_secs(5));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
