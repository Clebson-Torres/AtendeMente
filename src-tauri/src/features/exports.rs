use sqlx::SqlitePool;
use zip::write::FileOptions;

use crate::audit;
use crate::crypto;
use crate::db::models::RecordFile;
use crate::errors::AppError;

pub struct ExportBundle {
    pub buffer: Vec<u8>,
    pub appointment_count: usize,
    pub file_count: usize,
}

pub async fn export_patient_bundle(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
) -> Result<ExportBundle, AppError> {
    // Get patient
    let patient = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, String)>(
        r#"SELECT id, user_id, full_name, chart_number, phone, email, birth_date, status, created_at
        FROM patients WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(patient_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Paciente nao encontrado."))?;

    // Get appointments with records and payments
    let appointments = sqlx::query_as::<_, (String, String, String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>)>(
        r#"
        SELECT
            a.id, a.starts_at, a.ends_at, a.status, a.patient_id,
            a.session_price_cents, a.quick_notes,
            pay.status, pay.method, pay.amount_received_cents, pay.paid_at,
            sr.encrypted_payload, sr.iv, sr.auth_tag, sr.key_version
        FROM appointments a
        LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL
        LEFT JOIN session_records sr ON sr.appointment_id = a.id AND sr.deleted_at IS NULL
        WHERE a.user_id = ? AND a.patient_id = ? AND a.deleted_at IS NULL
        ORDER BY a.starts_at DESC
        "#,
    )
    .bind(user_id)
    .bind(patient_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get appointments: {}", e)))?;

    // Get all files for this patient
    let files = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files
        WHERE user_id = ? AND patient_id = ? AND deleted_at IS NULL
        ORDER BY uploaded_at"#,
    )
    .bind(user_id)
    .bind(patient_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get files: {}", e)))?;

    let file_count = files.len();

    // Build manifest
    let manifest = build_manifest(user_id, &patient, &appointments, &files);

    // Create ZIP
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));

    let options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // Add manifest.json
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| AppError::internal(format!("Failed to serialize manifest: {}", e)))?;
    zip.start_file("manifest.json", options)
        .map_err(|e| AppError::internal(format!("ZIP error: {}", e)))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| AppError::internal(format!("ZIP write error: {}", e)))?;

    use std::io::Write;

    // Add files directory
    for (i, appt) in appointments.iter().enumerate() {
        let appt_files: Vec<&RecordFile> = files
            .iter()
            .filter(|f| f.appointment_id == appt.0)
            .collect();

        for file in &appt_files {
            let path = std::path::Path::new(&file.storage_path);
            let data = match tokio::fs::read(path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            let zip_path = format!(
                "files/session_{}/{}/{}",
                i + 1,
                file.kind,
                &file.original_name
            );
            zip.start_file(&zip_path, options)
                .map_err(|e| AppError::internal(format!("ZIP error: {}", e)))?;
            zip.write_all(&data)
                .map_err(|e| AppError::internal(format!("ZIP write error: {}", e)))?;
        }
    }

    let buffer = zip
        .finish()
        .map_err(|e| AppError::internal(format!("ZIP finish error: {}", e)))?
        .into_inner();

    // Audit
    audit::write_audit_log(
        db,
        user_id,
        "patient_export",
        "patient",
        Some(patient_id),
        Some(&serde_json::json!({
            "appointment_count": appointments.len(),
            "file_count": file_count,
        })),
        None,
        None,
    )
    .await?;

    Ok(ExportBundle {
        buffer,
        appointment_count: appointments.len(),
        file_count,
    })
}

fn build_manifest(
    user_id: &str,
    patient: &(String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, String),
    appointments: &Vec<(String, String, String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>)>,
    files: &[RecordFile],
) -> serde_json::Value {
    let appts: Vec<serde_json::Value> = appointments
        .iter()
        .enumerate()
        .map(|(_i, a)| {
            let summary = if let (Some(ep), Some(iv), Some(at), Some(kv)) =
                (&a.11, &a.12, &a.13, a.14)
            {
                let payload = crypto::EncryptedPayload {
                    encrypted_payload: ep.clone(),
                    iv: iv.clone(),
                    auth_tag: at.clone(),
                    key_version: kv,
                };
                crypto::decrypt_content(&payload, user_id).ok()
            } else {
                None
            };

            let appt_files: Vec<serde_json::Value> = files
                .iter()
                .filter(|f| f.appointment_id == a.0)
                .map(|f| {
                    serde_json::json!({
                        "id": f.id,
                        "kind": f.kind,
                        "original_name": f.original_name,
                        "mime_type": f.mime_type,
                        "byte_size": f.byte_size,
                        "uploaded_at": f.uploaded_at,
                    })
                })
                .collect();

            serde_json::json!({
                "appointmentId": a.0,
                "startsAt": a.1,
                "endsAt": a.2,
                "status": a.3,
                "sessionPriceCents": a.5,
                "quickNotes": a.6,
                "payment": {
                    "status": a.7,
                    "method": a.8,
                    "amountReceivedCents": a.9,
                    "paidAt": a.10,
                },
                "summary": summary,
                "files": appt_files,
            })
        })
        .collect();

    serde_json::json!({
        "exportedAt": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        "patient": {
            "id": patient.0,
            "fullName": patient.2,
            "chartNumber": patient.3,
            "phone": patient.4,
            "email": patient.5,
            "birthDate": patient.6,
            "status": patient.7,
        },
        "appointments": appts,
    })
}
