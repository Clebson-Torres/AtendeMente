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
                encrypted_payload = ?, iv = ?, auth_tag = ?, key_version = ?, updated_at = ?
            WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
        )
        .bind(&encrypted.encrypted_payload)
        .bind(&encrypted.iv)
        .bind(&encrypted.auth_tag)
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("records-test.db");
        let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = crate::db::init_database(&url).await.unwrap();
        (dir, pool)
    }

    async fn seed_appointment(db: &SqlitePool, user_id: &str, patient_id: &str, appointment_id: &str) {
        let now = "2026-06-12T10:00:00";
        sqlx::query(
            "INSERT INTO users (id, email, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind("test@example.com")
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(patient_id)
        .bind(user_id)
        .bind("Paciente Teste")
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"INSERT INTO appointments
            (id, user_id, patient_id, starts_at, ends_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(appointment_id)
        .bind(user_id)
        .bind(patient_id)
        .bind("2026-06-12T11:00:00")
        .bind("2026-06-12T12:00:00")
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn updating_session_record_keeps_it_decryptable() {
        let (_dir, db) = test_db().await;
        let user_id = "user-record-update";
        let patient_id = "patient-record-update";
        let appointment_id = "appointment-record-update";
        crate::crypto::set_pepper(&[9u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_appointment(&db, user_id, patient_id, appointment_id).await;

        let input = SaveRecordInput {
            appointment_id: appointment_id.into(),
            patient_id: patient_id.into(),
            content: "Primeira versao".into(),
        };
        save_session_record(&db, user_id, &input).await.unwrap();

        let updated = SaveRecordInput {
            content: "Segunda versao".into(),
            ..input
        };
        save_session_record(&db, user_id, &updated).await.unwrap();

        let content = get_session_record(&db, user_id, appointment_id).await.unwrap();
        assert_eq!(content, "Segunda versao");
    }
}
