use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::crypto;
use crate::db::models::SaveRecordInput;
use crate::errors::AppError;

pub async fn save_session_record(
    db: &SqlitePool,
    user_id: &str,
    input: &SaveRecordInput,
) -> Result<(), AppError> {
    // Verify appointment exists and belongs to user
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

    // Encrypt the content
    let encrypted = crypto::encrypt_content(&input.content, user_id)?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // Upsert
    let existing = sqlx::query_as::<_, (String,)>(
        r#"SELECT id FROM session_records WHERE appointment_id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.appointment_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|_| AppError::internal("DB error."))?;

    if let Some((record_id,)) = existing {
        sqlx::query(
            r#"UPDATE session_records SET
                encrypted_payload = ?, iv = ?, key_version = ?, updated_at = ?
            WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
        )
        .bind(&encrypted.encrypted_payload)
        .bind(&encrypted.iv)
        .bind(encrypted.key_version)
        .bind(&now)
        .bind(&record_id)
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to update record: {}", e)))?;
    } else {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO session_records (id, user_id, patient_id, appointment_id,
                encrypted_payload, iv, auth_tag, key_version, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&input.patient_id)
        .bind(&input.appointment_id)
        .bind(&encrypted.encrypted_payload)
        .bind(&encrypted.iv)
        .bind(&encrypted.auth_tag)
        .bind(encrypted.key_version)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to create record: {}", e)))?;
    }

    audit::write_audit_log(
        db,
        user_id,
        "update",
        "session_record",
        Some(&input.appointment_id),
        None,
        None,
        None,
    )
    .await?;

    Ok(())
}

pub async fn get_session_record(
    db: &SqlitePool,
    user_id: &str,
    appointment_id: &str,
) -> Result<String, AppError> {
    let record = sqlx::query_as::<_, (String, String, String, i32)>(
        r#"SELECT encrypted_payload, iv, auth_tag, key_version
        FROM session_records
        WHERE appointment_id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(appointment_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get record: {}", e)))?
    .ok_or_else(|| AppError::not_found("Registro nao encontrado."))?;

    let payload = crypto::EncryptedPayload {
        encrypted_payload: record.0,
        iv: record.1,
        auth_tag: record.2,
        key_version: record.3,
    };

    crypto::decrypt_content(&payload, user_id)
}
