use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::db::models::{
    AppointmentDetail, CalendarEvent, CreateAppointmentInput,
    RecordFile,
};
use crate::errors::AppError;
use crate::features::appointments_recurrence::build_recurring_appointments;

#[derive(Debug, sqlx::FromRow)]
struct AppointmentDetailRow {
    id: String,
    patient_id: String,
    patient_name: String,
    starts_at: String,
    ends_at: String,
    series_id: Option<String>,
    status: String,
    confirmation_status: String,
    session_price_cents: i64,
    quick_notes: Option<String>,
    cancel_reason: Option<String>,
    payment_id: Option<String>,
    payment_status: Option<String>,
    payment_method: Option<String>,
    amount_received_cents: Option<i64>,
    paid_at: Option<String>,
    payment_notes: Option<String>,
    record_id: Option<String>,
    encrypted_payload: Option<String>,
    iv: Option<String>,
    auth_tag: Option<String>,
    key_version: Option<i32>,
    #[allow(dead_code)]
    frequency: Option<String>,
}

pub async fn list_calendar_events(
    db: &SqlitePool,
    user_id: &str,
    start: &str,
    end: &str,
) -> Result<Vec<CalendarEvent>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        r#"SELECT a.id, a.patient_id, p.full_name, a.starts_at, a.ends_at, a.status, a.confirmation_status
        FROM appointments a
        INNER JOIN patients p ON p.id = a.patient_id
        WHERE a.user_id = ? AND a.deleted_at IS NULL
        AND a.starts_at >= ? AND a.starts_at <= ?
        ORDER BY a.starts_at"#,
    )
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to list events: {}", e)))?;

    Ok(rows
        .into_iter()
        .map(|r| CalendarEvent {
            id: r.0,
            patient_id: r.1,
            title: r.2,
            start: r.3,
            end: r.4,
            status: r.5,
            confirmation_status: r.6,
        })
        .collect())
}

pub async fn get_appointment_detail(
    db: &SqlitePool,
    user_id: &str,
    appointment_id: &str,
) -> Result<AppointmentDetail, AppError> {
    let row = sqlx::query_as::<_, AppointmentDetailRow>(
        r#"
        SELECT
            a.id, a.patient_id, p.full_name AS patient_name, a.starts_at, a.ends_at,
            a.series_id, a.status, a.confirmation_status,
            a.session_price_cents, a.quick_notes, a.cancel_reason,
            pay.id AS payment_id, pay.status AS payment_status, pay.method AS payment_method,
            pay.amount_received_cents, pay.paid_at, pay.notes AS payment_notes,
            sr.id AS record_id, sr.encrypted_payload, sr.iv, sr.auth_tag, sr.key_version,
            rs.frequency
        FROM appointments a
        INNER JOIN patients p ON p.id = a.patient_id
        LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL
        LEFT JOIN session_records sr ON sr.appointment_id = a.id AND sr.deleted_at IS NULL
        LEFT JOIN recurring_series rs ON rs.id = a.series_id
        WHERE a.id = ? AND a.user_id = ? AND a.deleted_at IS NULL
        "#,
    )
    .bind(appointment_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get appointment: {}", e)))?
    .ok_or_else(|| AppError::not_found("Atendimento nao encontrado."))?;

    // Get files
    let files = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files
        WHERE user_id = ? AND appointment_id = ? AND deleted_at IS NULL
        ORDER BY uploaded_at DESC"#,
    )
    .bind(user_id)
    .bind(appointment_id)
    .fetch_all(db)
    .await
    .map_err(|_| AppError::internal("Failed to load files."))?;

    let _session_attachments: Vec<RecordFile> = files
        .iter()
        .filter(|f| f.kind == "session_attachment")
        .cloned()
        .collect();
    let _payment_receipts: Vec<RecordFile> = files
        .iter()
        .filter(|f| f.kind == "payment_receipt")
        .cloned()
        .collect();

    Ok(AppointmentDetail {
        id: row.id,
        patient_id: row.patient_id,
        patient_name: row.patient_name,
        starts_at: row.starts_at,
        ends_at: row.ends_at,
        series_id: row.series_id,
        status: row.status,
        confirmation_status: row.confirmation_status,
        session_price_cents: row.session_price_cents,
        quick_notes: row.quick_notes,
        cancel_reason: row.cancel_reason,
        payment_id: row.payment_id,
        payment_status: row.payment_status,
        payment_method: row.payment_method,
        amount_received_cents: row.amount_received_cents,
        paid_at: row.paid_at,
        payment_notes: row.payment_notes,
        record_id: row.record_id,
        encrypted_payload: row.encrypted_payload,
        iv: row.iv,
        auth_tag: row.auth_tag,
        key_version: row.key_version,
    })
}

async fn find_overlapping(
    db: &SqlitePool,
    user_id: &str,
    starts_at: &str,
    ends_at: &str,
    exclude_id: Option<&str>,
) -> Result<bool, AppError> {
    let mut query = String::from(
        r#"SELECT id FROM appointments
        WHERE user_id = ? AND deleted_at IS NULL AND status != 'cancelled'
        AND starts_at < ? AND ends_at > ?"#,
    );

    if let Some(eid) = exclude_id {
        use std::fmt::Write;
        write!(query, " AND id != '{}'", eid).unwrap();
    }

    query.push_str(" LIMIT 1");

    let q = sqlx::query_as::<_, (String,)>(&query)
        .bind(user_id)
        .bind(ends_at)
        .bind(starts_at);

    let result = q.fetch_optional(db).await
        .map_err(|e| AppError::internal(format!("Overlap check failed: {}", e)))?;

    Ok(result.is_some())
}

pub async fn create_appointment(
    db: &SqlitePool,
    user_id: &str,
    input: &CreateAppointmentInput,
) -> Result<AppointmentDetail, AppError> {
    // Validate dates
    if input.starts_at >= input.ends_at {
        return Err(AppError::bad_request("O fim precisa ser depois do inicio."));
    }

    // Check overlap
    if find_overlapping(db, user_id, &input.starts_at, &input.ends_at, None).await? {
        return Err(AppError::conflict("Ja existe um agendamento para este horario."));
    }

    let is_recurring = input.recurrence_frequency.is_some();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    if !is_recurring {
        let id = Uuid::new_v4().to_string();
        let status = input.status.as_deref().unwrap_or("scheduled");
        let confirmation = input.confirmation_status.as_deref().unwrap_or("unconfirmed");
        let price = input.session_price_cents.unwrap_or(0);

        sqlx::query(
            r#"INSERT INTO appointments (id, user_id, patient_id, starts_at, ends_at, status,
                confirmation_status, session_price_cents, quick_notes, cancel_reason, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&input.patient_id)
        .bind(&input.starts_at)
        .bind(&input.ends_at)
        .bind(status)
        .bind(confirmation)
        .bind(price)
        .bind(&input.quick_notes)
        .bind(&input.cancel_reason)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to create appointment: {}", e)))?;

        audit::write_audit_log(
            db, user_id, "update", "appointment", Some(&id),
            Some(&serde_json::json!({"action": "create"})), None, None,
        ).await?;

        return get_appointment_detail(db, user_id, &id).await;
    }

    // Recurring series
    let frequency = input.recurrence_frequency.as_deref().unwrap_or("weekly");
    let occurrences = input.recurrence_occurrences;

    let results = build_recurring_appointments(
        &input.starts_at,
        &input.ends_at,
        frequency,
        input.recurrence_until_date.as_deref(),
        occurrences,
    );

    if results.len() < 2 {
        return Err(AppError::bad_request("A recorrencia precisa gerar pelo menos duas sessoes."));
    }

    // Create series
    let series_id = Uuid::new_v4().to_string();
    let start_date = &input.starts_at[..10];
    let start_time = &input.starts_at[11..16];
    let end_time = &input.ends_at[11..16];

    sqlx::query(
        r#"INSERT INTO recurring_series (id, user_id, patient_id, frequency, starts_on,
            ends_on, occurrences_count, start_time, end_time, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&series_id)
    .bind(user_id)
    .bind(&input.patient_id)
    .bind(frequency)
    .bind(start_date)
    .bind(&input.recurrence_until_date)
    .bind(occurrences)
    .bind(start_time)
    .bind(end_time)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create series: {}", e)))?;

    let status = input.status.as_deref().unwrap_or("scheduled");
    let confirmation = input.confirmation_status.as_deref().unwrap_or("unconfirmed");
    let price = input.session_price_cents.unwrap_or(0);

    for appt in &results {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO appointments (id, user_id, patient_id, series_id, starts_at, ends_at,
                status, confirmation_status, session_price_cents, quick_notes, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&input.patient_id)
        .bind(&series_id)
        .bind(&appt.starts_at)
        .bind(&appt.ends_at)
        .bind(status)
        .bind(confirmation)
        .bind(price)
        .bind(&input.quick_notes)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to create recurring appointment: {}", e)))?;
    }

    audit::write_audit_log(
        db, user_id, "update", "recurring_series", Some(&series_id),
        Some(&serde_json::json!({"action": "create", "created_count": results.len()})), None, None,
    ).await?;

    get_appointment_detail(db, user_id, &results[0].starts_at).await
}

pub async fn update_appointment(
    db: &SqlitePool,
    user_id: &str,
    appointment_id: &str,
    input: &CreateAppointmentInput,
) -> Result<AppointmentDetail, AppError> {
    // Verify exists
    let _existing = get_appointment_detail(db, user_id, appointment_id).await?;

    if input.starts_at >= input.ends_at {
        return Err(AppError::bad_request("O fim precisa ser depois do inicio."));
    }

    if find_overlapping(db, user_id, &input.starts_at, &input.ends_at, Some(appointment_id)).await? {
        return Err(AppError::conflict("Ja existe um agendamento para este horario."));
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let status = input.status.as_deref().unwrap_or("scheduled");
    let confirmation = input.confirmation_status.as_deref().unwrap_or("unconfirmed");
    let price = input.session_price_cents.unwrap_or(0);

    sqlx::query(
        r#"UPDATE appointments SET patient_id = ?, starts_at = ?, ends_at = ?, status = ?,
            confirmation_status = ?, session_price_cents = ?, quick_notes = ?, cancel_reason = ?,
            updated_at = ?
        WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.patient_id)
    .bind(&input.starts_at)
    .bind(&input.ends_at)
    .bind(status)
    .bind(confirmation)
    .bind(price)
    .bind(&input.quick_notes)
    .bind(&input.cancel_reason)
    .bind(&now)
    .bind(appointment_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to update appointment: {}", e)))?;

    audit::write_audit_log(
        db, user_id, "update", "appointment", Some(appointment_id),
        None, None, None,
    ).await?;

    get_appointment_detail(db, user_id, appointment_id).await
}

pub async fn cancel_appointment(
    db: &SqlitePool,
    user_id: &str,
    appointment_id: &str,
    cancel_reason: &str,
) -> Result<AppointmentDetail, AppError> {
    let reason = if cancel_reason.is_empty() {
        "Cancelado manualmente."
    } else {
        cancel_reason
    };
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    sqlx::query(
        r#"UPDATE appointments SET status = 'cancelled', confirmation_status = 'cancelled',
            cancel_reason = ?, updated_at = ?
        WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(reason)
    .bind(&now)
    .bind(appointment_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to cancel appointment: {}", e)))?;

    audit::write_audit_log(
        db, user_id, "update", "appointment", Some(appointment_id),
        Some(&serde_json::json!({"action": "cancel"})), None, None,
    ).await?;

    get_appointment_detail(db, user_id, appointment_id).await
}

pub async fn cancel_recurring_series(
    db: &SqlitePool,
    user_id: &str,
    series_id: &str,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // Cancel series
    let result = sqlx::query(
        r#"UPDATE recurring_series SET cancelled_at = ?, updated_at = ?
        WHERE id = ? AND user_id = ? AND cancelled_at IS NULL"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(series_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to cancel series: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Serie recorrente nao encontrada."));
    }

    // Cancel future appointments
    sqlx::query(
        r#"UPDATE appointments SET status = 'cancelled', confirmation_status = 'cancelled',
            cancel_reason = 'Serie recorrente encerrada.', updated_at = ?
        WHERE user_id = ? AND series_id = ? AND starts_at >= ? AND deleted_at IS NULL AND status != 'cancelled'"#,
    )
    .bind(&now)
    .bind(user_id)
    .bind(series_id)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to cancel series appointments: {}", e)))?;

    audit::write_audit_log(
        db, user_id, "update", "recurring_series", Some(series_id),
        Some(&serde_json::json!({"action": "cancel"})), None, None,
    ).await?;

    Ok(())
}
