use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

use crate::auth::auth_service;
use crate::db::models::RecordFile;
use crate::features;
use crate::AppState;

#[derive(Serialize)]
pub struct DownloadResult {
    pub id: String,
    pub original_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub uploaded_at: String,
    pub content_base64: String,
}

async fn validate_token_and_get_db(
    state: &AppState,
    token: &str,
) -> Result<(String, sqlx::SqlitePool), String> {
    let (user_id, _, _) = auth_service::validate_session(&state.auth_db, token)
        .await
        .map_err(|e| format!("Unauthorized: {}", e))?;

    let db = state
        .get_or_open_user_db(&user_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok((user_id, db))
}

#[tauri::command]
pub async fn cmd_export_patient_zip(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    patient_id: String,
) -> Result<String, String> {
    let (user_id, db) = validate_token_and_get_db(&state, &token).await?;

    let bundle = features::exports::export_patient_bundle(&db, &user_id, &patient_id)
        .await
        .map_err(|e| format!("Export error: {}", e))?;

    let file_path = app
        .dialog()
        .file()
        .add_filter("ZIP", &["zip"])
        .set_file_name(&format!("paciente-{}.zip", patient_id))
        .blocking_save_file()
        .ok_or_else(|| "Save cancelled.".to_string())?;

    let path = file_path
        .into_path()
        .map_err(|_| "Invalid path.".to_string())?;

    std::fs::write(&path, &bundle.buffer)
        .map_err(|e| format!("Error writing file: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn cmd_list_files_by_appointment(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    appointment_id: String,
) -> Result<Vec<RecordFile>, String> {
    let (user_id, db) = validate_token_and_get_db(&state, &token).await?;
    features::files::list_files_by_appointment(&db, &user_id, &appointment_id)
        .await
        .map_err(|e| format!("Error listing files: {}", e))
}

#[tauri::command]
pub async fn cmd_download_file(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    file_id: String,
) -> Result<DownloadResult, String> {
    let (user_id, db) = validate_token_and_get_db(&state, &token).await?;
    let (file, data) = features::files::download_file(&db, &user_id, &file_id)
        .await
        .map_err(|e| format!("Error downloading file: {}", e))?;

    Ok(DownloadResult {
        id: file.id,
        original_name: file.original_name,
        mime_type: file.mime_type,
        byte_size: file.byte_size,
        uploaded_at: file.uploaded_at,
        content_base64: BASE64.encode(&data),
    })
}

#[tauri::command]
pub async fn cmd_upload_file_content(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    file_id: String,
    content_base64: String,
) -> Result<(), String> {
    let (user_id, db) = validate_token_and_get_db(&state, &token).await?;
    let data = BASE64
        .decode(&content_base64)
        .map_err(|e| format!("Invalid base64: {}", e))?;
    features::files::write_upload_content(&db, &user_id, &file_id, &data)
        .await
        .map_err(|e| format!("Error writing file content: {}", e))
}

#[tauri::command]
pub async fn cmd_confirm_file_upload(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    file_id: String,
) -> Result<RecordFile, String> {
    let (user_id, db) = validate_token_and_get_db(&state, &token).await?;
    features::files::confirm_upload(&db, &state.config, &user_id, &file_id)
        .await
        .map_err(|e| format!("Error confirming upload: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    async fn setup_test_state() -> (Arc<AppState>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let pepper = [0u8; 32];

        let auth_db_path = tmp.path().join("auth.db");
        let auth_db_url = format!("sqlite:{}?mode=rwc", auth_db_path.display());
        let auth_db = db::init_auth_database(&auth_db_url).await.unwrap();

        let storage_dir = tmp.path().join("uploads");
        std::fs::create_dir_all(&storage_dir).unwrap();

        let config = AppConfig {
            database_url: String::new(),
            auth_database_url: auth_db_url,
            server_port: 3001,
            master_pepper: pepper,
            storage_dir,
            mobile_access_enabled: false,
        };

        let state = Arc::new(AppState {
            config,
            auth_db,
            user_dbs: RwLock::new(HashMap::new()),
        });

        (state, tmp)
    }

    #[tokio::test]
    async fn test_validate_token_invalid() {
        let (state, _tmp) = setup_test_state().await;
        let result = validate_token_and_get_db(&state, "invalid-token").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unauthorized"));
    }
}
