use sqlx::SqlitePool;

use crate::db::models::CalendarEvent;
use crate::errors::AppError;

pub async fn get_dashboard_data(
    db: &SqlitePool,
    user_id: &str,
) -> Result<(i64, Vec<CalendarEvent>, Vec<CalendarEvent>, Vec<serde_json::Value>, Vec<serde_json::Value>), AppError> {
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

    let today_start = format!("{}T00:00:00", now.format("%Y-%m-%d"));
    let today_end = format!("{}T23:59:59", now.format("%Y-%m-%d"));
    let twelve_months_ago = (now - chrono::Duration::days(365)).format("%Y-%m-%dT00:00:00").to_string();
    let current_time = now.format("%Y-%m-%dT%H:%M:%S").to_string();

    let (
        count_result,
        todays_result,
        upcoming_result,
        monthly_appointments_result,
        monthly_financial_result,
    ) = tokio::join!(
        sqlx::query_as::<_, (i64,)>(
            r#"SELECT COUNT(*) FROM appointments
            WHERE user_id = ? AND deleted_at IS NULL
            AND status IN ('scheduled', 'completed')
            AND starts_at >= ? AND starts_at < ?"#,
        )
        .bind(user_id)
        .bind(&month_start)
        .bind(&next_month)
        .fetch_one(db),
        sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
            r#"SELECT a.id, a.patient_id, p.full_name, a.starts_at, a.ends_at, a.status, a.confirmation_status
            FROM appointments a
            INNER JOIN patients p ON p.id = a.patient_id
            WHERE a.user_id = ? AND a.deleted_at IS NULL
            AND a.starts_at >= ? AND a.starts_at <= ?
            ORDER BY a.starts_at"#,
        )
        .bind(user_id)
        .bind(&today_start)
        .bind(&today_end)
        .fetch_all(db),
        sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
            r#"SELECT a.id, a.patient_id, p.full_name, a.starts_at, a.ends_at, a.status, a.confirmation_status
            FROM appointments a
            INNER JOIN patients p ON p.id = a.patient_id
            WHERE a.user_id = ? AND a.deleted_at IS NULL
            AND a.starts_at >= ? AND a.status = 'scheduled'
            ORDER BY a.starts_at
            LIMIT 8"#,
        )
        .bind(user_id)
        .bind(&current_time)
        .fetch_all(db),
        sqlx::query_as::<_, (String, i64)>(
            r#"SELECT strftime('%Y-%m', starts_at) as month, COUNT(*) as count
            FROM appointments
            WHERE user_id = ? AND deleted_at IS NULL AND status != 'cancelled'
            AND starts_at >= ?
            GROUP BY strftime('%Y-%m', starts_at)
            ORDER BY month"#,
        )
        .bind(user_id)
        .bind(&twelve_months_ago)
        .fetch_all(db),
        sqlx::query_as::<_, (String, i64)>(
            r#"SELECT strftime('%Y-%m', a.starts_at) as month, COALESCE(SUM(pay.amount_received_cents), 0) as total
            FROM appointments a
            LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL AND pay.status = 'paid'
            WHERE a.user_id = ? AND a.deleted_at IS NULL AND a.status != 'cancelled'
            AND a.starts_at >= ?
            GROUP BY strftime('%Y-%m', a.starts_at)
            ORDER BY month"#,
        )
        .bind(user_id)
        .bind(&twelve_months_ago)
        .fetch_all(db),
    );

    let (count,) = count_result
        .map_err(|e| AppError::internal(format!("Dashboard stats error: {}", e)))?;

    let todays = todays_result
        .map_err(|e| AppError::internal(format!("Dashboard today error: {}", e)))?
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
        .collect();

    let upcoming = upcoming_result
        .map_err(|e| AppError::internal(format!("Dashboard upcoming error: {}", e)))?
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
        .collect();

    let monthly_appointments = monthly_appointments_result
        .map_err(|e| AppError::internal(format!("Dashboard monthly error: {}", e)))?
        .into_iter()
        .map(|(m, c)| serde_json::json!({"month": m, "count": c}))
        .collect();

    let monthly_financial = monthly_financial_result
        .map_err(|e| AppError::internal(format!("Dashboard financial error: {}", e)))?
        .into_iter()
        .map(|(m, t)| serde_json::json!({"month": m, "total_cents": t}))
        .collect();

    Ok((count, todays, upcoming, monthly_appointments, monthly_financial))
}

#[cfg(test)]
mod tests {
    use super::get_dashboard_data;
    use sqlx::SqlitePool;

    async fn test_db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("dash.db").to_string_lossy()
        );
        (dir, crate::db::init_database(&url).await.unwrap())
    }

    async fn seed(db: &SqlitePool, user_id: &str) {
        sqlx::query("INSERT INTO users (id, email, created_at, updated_at) VALUES (?, 'd@x.com', '2026-01-01T00:00:00', '2026-01-01T00:00:00')")
            .bind(user_id).execute(db).await.unwrap();
        sqlx::query("INSERT INTO patients (id, user_id, full_name, created_at, updated_at) VALUES ('pac', ?, 'Paciente Dash', '2026-01-01T00:00:00', '2026-01-01T00:00:00')")
            .bind(user_id).execute(db).await.unwrap();
    }

    /// `starts_at`/`ends_at` relative to now, since the dashboard windows are all
    /// computed from the current date.
    async fn appointment(
        db: &SqlitePool,
        user_id: &str,
        id: &str,
        offset: chrono::Duration,
        status: &str,
        price: i64,
    ) {
        let fmt = "%Y-%m-%dT%H:%M:%S";
        let start = chrono::Utc::now() + offset;
        sqlx::query(
            "INSERT INTO appointments (id, user_id, patient_id, starts_at, ends_at, status, \
             session_price_cents, created_at, updated_at) \
             VALUES (?, ?, 'pac', ?, ?, ?, ?, '2026-01-01T00:00:00', '2026-01-01T00:00:00')",
        )
        .bind(id)
        .bind(user_id)
        .bind(start.format(fmt).to_string())
        .bind((start + chrono::Duration::hours(1)).format(fmt).to_string())
        .bind(status)
        .bind(price)
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn returns_zeros_for_an_empty_account() {
        let (_d, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400d1";
        seed(&db, user_id).await;

        let (count, todays, upcoming, meses, financeiro) =
            get_dashboard_data(&db, user_id).await.unwrap();
        assert_eq!(count, 0);
        assert!(todays.is_empty());
        assert!(upcoming.is_empty());
        assert!(meses.is_empty());
        assert!(financeiro.is_empty());
    }

    #[tokio::test]
    async fn counts_today_and_upcoming_separately() {
        let (_d, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400d2";
        seed(&db, user_id).await;

        appointment(&db, user_id, "hoje", chrono::Duration::minutes(90), "scheduled", 10000).await;
        appointment(&db, user_id, "amanha", chrono::Duration::days(1), "scheduled", 10000).await;
        appointment(&db, user_id, "passado", chrono::Duration::days(-2), "completed", 10000).await;

        let (_c, todays, upcoming, _m, _f) = get_dashboard_data(&db, user_id).await.unwrap();

        assert_eq!(todays.len(), 1, "so o de hoje entra em todays");
        assert_eq!(todays[0].id, "hoje");
        assert_eq!(todays[0].title, "Paciente Dash", "deve trazer o nome do paciente");

        let ids: Vec<&str> = upcoming.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"amanha"), "futuros agendados entram em upcoming");
        assert!(!ids.contains(&"passado"), "passado nao e upcoming");
    }

    #[tokio::test]
    async fn cancelled_appointments_are_excluded_from_counters() {
        let (_d, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400d3";
        seed(&db, user_id).await;

        appointment(&db, user_id, "vale", chrono::Duration::hours(2), "scheduled", 10000).await;
        appointment(&db, user_id, "cancelado", chrono::Duration::hours(3), "cancelled", 10000).await;

        let (count, _t, upcoming, meses, _f) = get_dashboard_data(&db, user_id).await.unwrap();
        assert_eq!(count, 1, "cancelado nao conta no total do mes");
        assert!(
            !upcoming.iter().any(|e| e.id == "cancelado"),
            "cancelado nao aparece em upcoming"
        );
        let total_mes: i64 = meses.iter().map(|m| m["count"].as_i64().unwrap()).sum();
        assert_eq!(total_mes, 1, "cancelado nao entra no grafico mensal");
    }

    #[tokio::test]
    async fn monthly_financial_only_counts_paid_payments() {
        let (_d, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400d4";
        seed(&db, user_id).await;

        appointment(&db, user_id, "a1", chrono::Duration::hours(1), "completed", 20000).await;
        appointment(&db, user_id, "a2", chrono::Duration::hours(2), "completed", 30000).await;

        // Um pago, um pendente.
        sqlx::query("INSERT INTO payments (id, user_id, appointment_id, status, method, amount_received_cents, created_at, updated_at) VALUES ('pg1', ?, 'a1', 'paid', 'pix', 20000, '2026-01-01T00:00:00', '2026-01-01T00:00:00')")
            .bind(user_id).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO payments (id, user_id, appointment_id, status, method, amount_received_cents, created_at, updated_at) VALUES ('pg2', ?, 'a2', 'pending', 'pix', 30000, '2026-01-01T00:00:00', '2026-01-01T00:00:00')")
            .bind(user_id).execute(&db).await.unwrap();

        let (_c, _t, _u, _m, financeiro) = get_dashboard_data(&db, user_id).await.unwrap();
        let total: i64 = financeiro.iter().map(|m| m["total_cents"].as_i64().unwrap()).sum();
        assert_eq!(total, 20000, "so pagamento com status 'paid' entra na receita");
    }

    #[tokio::test]
    async fn dashboard_is_scoped_to_its_owner() {
        let (_d, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400d5";
        seed(&db, user_id).await;
        appointment(&db, user_id, "meu", chrono::Duration::hours(2), "scheduled", 10000).await;

        let (count, todays, upcoming, meses, financeiro) =
            get_dashboard_data(&db, "550e8400-e29b-41d4-a716-4466554400ff")
                .await
                .unwrap();
        assert_eq!(count, 0);
        assert!(todays.is_empty() && upcoming.is_empty());
        assert!(meses.is_empty() && financeiro.is_empty());
    }
}
