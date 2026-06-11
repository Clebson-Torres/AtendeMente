use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::config::AppConfig;
use crate::db::models::{FileUploadRequest, RecordFile};
use crate::errors::AppError;

const ALLOWED_EXTENSIONS: &[&str] = &[".pdf", ".doc", ".docx", ".png", ".jpg", ".jpeg"];
pub async fn create_upload_session(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    input: &FileUploadRequest,
) -> Result<(String, String), AppError> {
    // Validate extension
    let normalized_name = input.file_name.to_lowercase();
    let has_extension = ALLOWED_EXTENSIONS
        .iter()
        .any(|ext| normalized_name.ends_with(ext));
    if !has_extension {
        return Err(AppError::bad_request("Extensao de arquivo nao permitida."));
    }

    // Verify appointment
    let _appt = sqlx::query_as::<_, (String,)>(
        r#"SELECT id FROM appointments
        WHERE id = ? AND user_id = ? AND patient_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.appointment_id)
    .bind(user_id)
    .bind(&input.patient_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Atendimento nao encontrado."))?;

    // Verify payment if receipt
    let payment_id = if input.kind == "payment_receipt" {
        let pid = input.payment_id.as_ref().ok_or_else(|| {
            AppError::bad_request("Informe o pagamento vinculado ao recibo.")
        })?;

        sqlx::query_as::<_, (String,)>(
            r#"SELECT id FROM payments
            WHERE id = ? AND user_id = ? AND appointment_id = ? AND deleted_at IS NULL"#,
        )
        .bind(pid)
        .bind(user_id)
        .bind(&input.appointment_id)
        .fetch_optional(db)
        .await
        .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Pagamento nao encontrado para vincular o recibo."))?;

        Some(pid.clone())
    } else {
        None
    };

    // Create storage path
    let storage_path = config.storage_path_for(
        user_id,
        &input.patient_id,
        &input.appointment_id,
        &input.file_name,
    );

    // Ensure storage dir exists
    if let Some(parent) = storage_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("Failed to create storage dir: {}", e)))?;
    }

    let storage_path_str = storage_path.to_string_lossy().to_string();

    // Create record
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"INSERT INTO record_files (id, user_id, patient_id, appointment_id, payment_id,
            kind, storage_path, original_name, mime_type, byte_size, uploaded_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.patient_id)
    .bind(&input.appointment_id)
    .bind(&payment_id)
    .bind(&input.kind)
    .bind(&storage_path_str)
    .bind(&input.file_name)
    .bind(&input.mime_type)
    .bind(input.file_size)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create file record: {}", e)))?;

    Ok((id, storage_path_str))
}

pub async fn confirm_upload(
    db: &SqlitePool,
    _config: &AppConfig,
    user_id: &str,
    file_id: &str,
) -> Result<RecordFile, AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Arquivo nao encontrado."))?;

    // Verify file exists on disk
    let path = std::path::Path::new(&file.storage_path);
    let metadata = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => {
            return Err(AppError::bad_request(
                "Arquivo nao encontrado no armazenamento. Faca o upload novamente.",
            ));
        }
    };

    // Verify size matches
    if metadata.len() != file.byte_size as u64 {
        // File size mismatch - reject
        let _ = tokio::fs::remove_file(path).await;
        sqlx::query("UPDATE record_files SET deleted_at = datetime('now') WHERE id = ? AND user_id = ?")
            .bind(file_id)
            .bind(user_id)
            .execute(db)
            .await
            .ok();
        return Err(AppError::bad_request(
            "Arquivo rejeitado pela validacao de seguranca (tamanho divergente).",
        ));
    }

    // Audit log
    audit::write_audit_log(
        db,
        user_id,
        "file_upload",
        "record_file",
        Some(file_id),
        Some(&serde_json::json!({
            "kind": file.kind,
            "original_name": file.original_name,
            "appointment_id": file.appointment_id,
            "patient_id": file.patient_id,
        })),
        None,
        None,
    )
    .await?;

    Ok(file)
}

pub async fn download_file(
    db: &SqlitePool,
    user_id: &str,
    file_id: &str,
) -> Result<(RecordFile, Vec<u8>), AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Arquivo nao encontrado."))?;

    let path = std::path::Path::new(&file.storage_path);
    let data = tokio::fs::read(path)
        .await
        .map_err(|_| AppError::not_found("Arquivo nao encontrado no disco."))?;

    audit::write_audit_log(
        db,
        user_id,
        "file_download",
        "record_file",
        Some(file_id),
        Some(&serde_json::json!({
            "kind": file.kind,
            "original_name": file.original_name,
        })),
        None,
        None,
    )
    .await?;

    Ok((file, data))
}
