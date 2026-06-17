use std::path::Path;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::config::AppConfig;
use crate::db::models::{FileUploadRequest, RecordFile};
use crate::errors::AppError;

const MAX_FILE_SIZE_BYTES: u64 = 20 * 1024 * 1024;

fn validate_file_content(bytes: &[u8], declared_name: &str) -> Result<(), AppError> {
    let kind = infer::get(bytes);
    let ext = Path::new(declared_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let allowed_pairs: &[(&str, &[&str])] = &[
        ("application/pdf",  &["pdf"]),
        ("image/jpeg",       &["jpg", "jpeg"]),
        ("image/png",        &["png"]),
        ("application/msword", &["doc"]),
        ("application/vnd.openxmlformats-officedocument.wordprocessingml.document", &["docx"]),
    ];

    match kind {
        Some(k) => {
            let mime = k.mime_type();
            let valid = allowed_pairs.iter().any(|(m, exts)| {
                *m == mime && exts.contains(&ext.as_str())
            });
            if !valid {
                return Err(AppError::bad_request(
                    "Tipo de arquivo não permitido ou extensão não corresponde ao conteúdo.",
                ));
            }
        }
        None => {
            if !["doc", "docx"].contains(&ext.as_str()) {
                return Err(AppError::bad_request("Tipo de arquivo não reconhecido."));
            }
        }
    }
    Ok(())
}

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
    )?;

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

pub async fn write_upload_content(
    db: &SqlitePool,
    user_id: &str,
    file_id: &str,
    data: &[u8],
) -> Result<(), AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Upload session nao encontrada."))?;

    if let Some(parent) = std::path::Path::new(&file.storage_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("Failed to create dir: {}", e)))?;
    }

    tokio::fs::write(&file.storage_path, data)
        .await
        .map_err(|e| AppError::internal(format!("Failed to write file: {}", e)))?;

    Ok(())
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

    // Verify size matches and max size
    if metadata.len() != file.byte_size as u64 {
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

    if metadata.len() > MAX_FILE_SIZE_BYTES {
        let _ = tokio::fs::remove_file(path).await;
        sqlx::query("UPDATE record_files SET deleted_at = datetime('now') WHERE id = ? AND user_id = ?")
            .bind(file_id)
            .bind(user_id)
            .execute(db)
            .await
            .ok();
        return Err(AppError::bad_request(
            "Arquivo excede o tamanho máximo de 20 MB.",
        ));
    }

    // Validate magic bytes
    let data = tokio::fs::read(path).await
        .map_err(|_| AppError::internal("Erro ao ler arquivo para validacao."))?;
    validate_file_content(&data, &file.original_name)?;

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

pub async fn list_files_by_appointment(
    db: &SqlitePool,
    user_id: &str,
    appointment_id: &str,
) -> Result<Vec<RecordFile>, AppError> {
    let files = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files
        WHERE user_id = ? AND appointment_id = ? AND deleted_at IS NULL
        ORDER BY uploaded_at DESC"#,
    )
    .bind(user_id)
    .bind(appointment_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?;

    Ok(files)
}

pub async fn delete_file(
    db: &SqlitePool,
    user_id: &str,
    file_id: &str,
) -> Result<(), AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Arquivo nao encontrado."))?;

    let _ = tokio::fs::remove_file(&file.storage_path).await;

    sqlx::query("UPDATE record_files SET deleted_at = datetime('now') WHERE id = ? AND user_id = ?")
        .bind(file_id)
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("DB error: {}", e)))?;

    audit::write_audit_log(
        db,
        user_id,
        "file_delete",
        "record_file",
        Some(file_id),
        Some(&serde_json::json!({
            "original_name": file.original_name,
        })),
        None,
        None,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db;

    fn valid_pdf_bytes() -> Vec<u8> {
        b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF".to_vec()
    }

    async fn test_db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("files-test.db");
        let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = db::init_database(&url).await.unwrap();
        (dir, pool)
    }

    fn test_config(tmp: &tempfile::TempDir) -> AppConfig {
        let storage_dir = tmp.path().join("uploads");
        std::fs::create_dir_all(&storage_dir).unwrap();
        AppConfig {
            database_url: String::new(),
            auth_database_url: String::new(),
            server_port: 3001,
            master_pepper: [0u8; 32],
            storage_dir,
        }
    }

    async fn seed_data(db: &SqlitePool, user_id: &str, patient_id: &str, appointment_id: &str) {
        let now = "2026-06-17T10:00:00";
        sqlx::query("INSERT INTO users (id, email, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(user_id)
            .bind("test@test.com")
            .bind(now)
            .bind(now)
            .execute(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO patients (id, user_id, full_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(patient_id)
            .bind(user_id)
            .bind("Test Patient")
            .bind(now)
            .bind(now)
            .execute(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO appointments (id, user_id, patient_id, starts_at, ends_at, status, session_price_cents, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(appointment_id)
            .bind(user_id)
            .bind(patient_id)
            .bind("2026-06-17T14:00:00")
            .bind("2026-06-17T15:00:00")
            .bind("scheduled")
            .bind(10000i64)
            .bind(now)
            .bind(now)
            .execute(db)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_create_upload_session() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: 200,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _storage_path) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session should be created");

        assert!(!file_id.is_empty());
    }

    #[tokio::test]
    async fn test_write_and_confirm_upload() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");

        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");

        let file = confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        assert_eq!(file.original_name, "test.pdf");
        assert_eq!(file.kind, "session_attachment");
        assert_eq!(file.byte_size, pdf_data.len() as i64);
    }

    #[tokio::test]
    async fn test_list_files_by_appointment() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");
        confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        let files = list_files_by_appointment(&db, user_id, appt_id)
            .await
            .expect("list files");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].original_name, "test.pdf");
    }

    #[tokio::test]
    async fn test_download_file() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");
        confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        let (file, data) = download_file(&db, user_id, &file_id)
            .await
            .expect("download file");

        assert_eq!(file.original_name, "test.pdf");
        assert_eq!(data, pdf_data);
    }

    #[tokio::test]
    async fn test_validate_file_content_pdf() {
        let pdf_data = valid_pdf_bytes();
        assert!(validate_file_content(&pdf_data, "test.pdf").is_ok());
    }

    #[tokio::test]
    async fn test_validate_file_content_rejects_mismatch() {
        let pdf_data = valid_pdf_bytes();
        // PDF content with .png extension should fail
        assert!(validate_file_content(&pdf_data, "test.png").is_err());
    }

    #[tokio::test]
    async fn test_upload_wrong_user_cannot_access() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let wrong_user = "550e8400-e29b-41d4-a716-446655449999";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();
        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");
        confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        // Wrong user should not be able to list
        let result = list_files_by_appointment(&db, wrong_user, appt_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        // Wrong user should not be able to download
        let result = download_file(&db, wrong_user, &file_id).await;
        assert!(result.is_err());
    }
}
