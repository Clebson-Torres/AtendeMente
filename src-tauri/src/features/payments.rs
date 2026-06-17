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
            COALESCE(SUM(CASE WHEN (pay.id IS NULL OR pay.status != 'paid') THEN a.session_price_cents ELSE 0 END), 0) as pending_cents
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
        AND a.starts_at >= ? AND a.starts_at < ?
        ORDER BY a.starts_at DESC
        "#,
    )
    .bind(user_id)
    .bind(&month_start)
    .bind(&next_month)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{AppointmentDetail, CreateAppointmentInput};

    async fn test_db() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("payments-test.db");
        let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = crate::db::init_database(&url).await.unwrap();
        (dir, pool)
    }

    async fn seed_user_and_patient(db: &SqlitePool, user_id: &str, patient_id: &str) {
        let now = "2026-06-12T10:00:00";
        sqlx::query("INSERT INTO users (id, email, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(user_id).bind("test@example.com").bind(now).bind(now)
            .execute(db).await.unwrap();
        sqlx::query("INSERT INTO patients (id, user_id, full_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(patient_id).bind(user_id).bind("Paciente Teste").bind(now).bind(now)
            .execute(db).await.unwrap();
    }

    async fn create_test_appointment(db: &SqlitePool, user_id: &str, patient_id: &str, starts_at: &str, ends_at: &str) -> AppointmentDetail {
        let input = CreateAppointmentInput {
            patient_id: patient_id.into(),
            starts_at: starts_at.into(),
            ends_at: ends_at.into(),
            status: None,
            confirmation_status: None,
            session_price_cents: Some(5000),
            quick_notes: None,
            cancel_reason: None,
            recurrence_frequency: None,
            recurrence_end_mode: None,
            recurrence_until_date: None,
            recurrence_occurrences: None,
        };
        crate::features::appointments::create_appointment(db, user_id, &input).await.unwrap()
    }

    async fn create_payment(db: &SqlitePool, user_id: &str, appointment_id: &str, status: &str, amount: i64) {
        let input = UpsertPaymentInput {
            appointment_id: appointment_id.into(),
            status: status.into(),
            method: "pix".into(),
            paid_at: Some("2026-06-15T12:00:00".into()),
            amount_received_cents: amount,
            notes: None,
        };
        upsert_payment(db, user_id, &input).await.unwrap();
    }

    #[tokio::test]
    async fn financial_summary_returns_zeros_for_empty_data() {
        let (_dir, db) = test_db().await;
        let user_id = "user-summary-zeros";
        let patient_id = "patient-summary-zeros";
        seed_user_and_patient(&db, user_id, patient_id).await;

        let (paid, pending) = get_financial_summary(&db, user_id).await.unwrap();
        assert_eq!(paid, 0);
        assert_eq!(pending, 0);
    }

    #[tokio::test]
    async fn financial_summary_counts_paid_and_pending() {
        let (_dir, db) = test_db().await;
        let user_id = "user-summary-counts";
        let patient_id = "patient-summary-counts";
        seed_user_and_patient(&db, user_id, patient_id).await;

        let appt1 = create_test_appointment(&db, user_id, patient_id,
            "2026-06-15T09:00:00", "2026-06-15T10:00:00").await;
        let appt2 = create_test_appointment(&db, user_id, patient_id,
            "2026-06-16T09:00:00", "2026-06-16T10:00:00").await;

        let update1 = crate::db::models::UpdateAppointmentInput {
            patient_id: None, starts_at: None, ends_at: None,
            status: Some("completed".into()), confirmation_status: None,
            session_price_cents: None, quick_notes: None, cancel_reason: None,
        };
        crate::features::appointments::update_appointment(&db, user_id, &appt1.id, &update1).await.unwrap();
        create_payment(&db, user_id, &appt1.id, "paid", 5000).await;

        let update2 = crate::db::models::UpdateAppointmentInput {
            patient_id: None, starts_at: None, ends_at: None,
            status: Some("completed".into()), confirmation_status: None,
            session_price_cents: None, quick_notes: None, cancel_reason: None,
        };
        crate::features::appointments::update_appointment(&db, user_id, &appt2.id, &update2).await.unwrap();

        let (paid, pending) = get_financial_summary(&db, user_id).await.unwrap();
        assert_eq!(paid, 5000);
        assert_eq!(pending, 5000);
    }

    #[tokio::test]
    async fn financial_summary_filters_by_current_month() {
        let (_dir, db) = test_db().await;
        let user_id = "user-summary-month";
        let patient_id = "patient-summary-month";
        seed_user_and_patient(&db, user_id, patient_id).await;

        let appt_current = create_test_appointment(&db, user_id, patient_id,
            "2026-06-10T09:00:00", "2026-06-10T10:00:00").await;
        let appt_last_month = create_test_appointment(&db, user_id, patient_id,
            "2026-05-15T09:00:00", "2026-05-15T10:00:00").await;

        for appt in [&appt_current, &appt_last_month] {
            let update = crate::db::models::UpdateAppointmentInput {
                patient_id: None, starts_at: None, ends_at: None,
                status: Some("completed".into()), confirmation_status: None,
                session_price_cents: None, quick_notes: None, cancel_reason: None,
            };
            crate::features::appointments::update_appointment(&db, user_id, &appt.id, &update).await.unwrap();
            create_payment(&db, user_id, &appt.id, "paid", 5000).await;
        }

        let (paid, pending) = get_financial_summary(&db, user_id).await.unwrap();
        assert_eq!(paid, 5000);
        assert_eq!(pending, 0);
    }

    #[tokio::test]
    async fn list_payments_filters_by_current_month() {
        let (_dir, db) = test_db().await;
        let user_id = "user-list-month";
        let patient_id = "patient-list-month";
        seed_user_and_patient(&db, user_id, patient_id).await;

        create_test_appointment(&db, user_id, patient_id,
            "2026-06-10T09:00:00", "2026-06-10T10:00:00").await;
        create_test_appointment(&db, user_id, patient_id,
            "2026-05-15T09:00:00", "2026-05-15T10:00:00").await;

        let payments = list_payments(&db, user_id).await.unwrap();
        assert_eq!(payments.len(), 1);
    }
}
