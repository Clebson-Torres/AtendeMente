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
