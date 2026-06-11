use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::db::models::{Payment, PaymentWithAppointment, UpsertPaymentInput};
use crate::errors::AppError;

pub async fn upsert_payment(
    db: &SqlitePool,
    user_id: &str,
    input: &UpsertPaymentInput,
) -> Result<Payment, AppError> {
    // Verify appointment exists and belongs to user
    let _appt = sqlx::query_as::<_, (String,)>(
        r#"SELECT id FROM appointments WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.appointment_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Atendimento nao encontrado."))?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // Upsert: insert or update
    let existing = sqlx::query_as::<_, (String,)>(
        r#"SELECT id FROM payments WHERE appointment_id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.appointment_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|_| AppError::internal("DB error."))?;

    let payment_id = if let Some((pid,)) = existing {
        // Update
        sqlx::query(
            r#"UPDATE payments SET status = ?, method = ?, paid_at = ?,
                amount_received_cents = ?, notes = ?, updated_at = ?
            WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
        )
        .bind(&input.status)
        .bind(&input.method)
        .bind(&input.paid_at)
        .bind(input.amount_received_cents)
        .bind(&input.notes)
        .bind(&now)
        .bind(&pid)
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to update payment: {}", e)))?;
        pid
    } else {
        // Insert
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO payments (id, user_id, appointment_id, status, method, paid_at,
                amount_received_cents, notes, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&input.appointment_id)
        .bind(&input.status)
        .bind(&input.method)
        .bind(&input.paid_at)
        .bind(input.amount_received_cents)
        .bind(&input.notes)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to create payment: {}", e)))?;
        id
    };

    audit::write_audit_log(
        db,
        user_id,
        "update",
        "payment",
        Some(&payment_id),
        None,
        None,
        None,
    )
    .await?;

    // Return the payment
    sqlx::query_as::<_, Payment>(
        r#"SELECT * FROM payments WHERE id = ? AND user_id = ?"#,
    )
    .bind(&payment_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch payment: {}", e)))
}

pub async fn get_financial_summary(
    db: &SqlitePool,
    user_id: &str,
) -> Result<(i64, i64), AppError> {
    let now = chrono::Utc::now();
    let year = now.format("%Y").to_string();
    let month = now.format("%m").to_string();
    let month_num: u32 = month.parse().unwrap_or(1);

    let month_start = format!("{}-{:02}-01T00:00:00", year, month_num);

    let next_month = if month_num == 12 {
        let next_year: u32 = year.parse::<u32>().unwrap_or(2024) + 1;
        format!("{}-01-01T00:00:00", next_year)
    } else {
        format!("{}-{:02}-01T00:00:00", year, month_num + 1)
    };

    let summary = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN pay.status = 'paid' THEN pay.amount_received_cents ELSE 0 END), 0) as paid_cents,
            COALESCE(SUM(CASE WHEN a.status = 'completed' AND (pay.id IS NULL OR pay.status = 'pending')
                THEN a.session_price_cents ELSE 0 END), 0) as pending_cents
        FROM appointments a
        LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL
        WHERE a.user_id = ? AND a.deleted_at IS NULL
        AND a.status != 'cancelled'
        AND a.starts_at >= ? AND a.starts_at < ?
        "#,
    )
    .bind(user_id)
    .bind(&month_start)
    .bind(&next_month)
    .fetch_one(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get financial summary: {}", e)))?;

    Ok(summary)
}

pub async fn list_payments(
    db: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PaymentWithAppointment>, AppError> {
    let rows = sqlx::query_as::<_, (Option<String>, String, String, Option<String>, Option<String>, Option<i64>, Option<String>, String, String, i64)>(
        r#"
        SELECT
            pay.id, a.id, a.status, pay.status, pay.method,
            pay.amount_received_cents, pay.paid_at,
            p.full_name, a.starts_at, a.session_price_cents
        FROM appointments a
        LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL
        INNER JOIN patients p ON p.id = a.patient_id
        WHERE a.user_id = ? AND a.deleted_at IS NULL AND a.status != 'cancelled'
        ORDER BY a.starts_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to list payments: {}", e)))?;

    Ok(rows
        .into_iter()
        .map(|r| PaymentWithAppointment {
            payment_id: r.0,
            appointment_id: r.1,
            appointment_status: r.2,
            status: r.3.unwrap_or_else(|| "pending".into()),
            method: r.4.unwrap_or_else(|| "other".into()),
            amount_received_cents: r.5.unwrap_or(0),
            paid_at: r.6,
            patient_name: r.7,
            starts_at: r.8,
            session_price_cents: r.9,
        })
        .collect())
}

pub async fn list_pending_payments(
    db: &SqlitePool,
    user_id: &str,
) -> Result<Vec<PaymentWithAppointment>, AppError> {
    let rows = sqlx::query_as::<_, (Option<String>, String, String, Option<String>, Option<String>, Option<i64>, Option<String>, String, String, i64)>(
        r#"
        SELECT
            pay.id, a.id, a.status, pay.status, pay.method,
            pay.amount_received_cents, pay.paid_at,
            p.full_name, a.starts_at, a.session_price_cents
        FROM appointments a
        LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL
        INNER JOIN patients p ON p.id = a.patient_id
        WHERE a.user_id = ? AND a.deleted_at IS NULL
        AND a.status = 'completed'
        AND (pay.id IS NULL OR pay.status = 'pending')
        ORDER BY a.starts_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to list pending payments: {}", e)))?;

    Ok(rows
        .into_iter()
        .map(|r| PaymentWithAppointment {
            payment_id: r.0,
            appointment_id: r.1,
            appointment_status: r.2,
            status: r.3.unwrap_or_else(|| "pending".into()),
            method: r.4.unwrap_or_else(|| "other".into()),
            amount_received_cents: r.5.unwrap_or(0),
            paid_at: r.6,
            patient_name: r.7,
            starts_at: r.8,
            session_price_cents: r.9,
        })
        .collect())
}
