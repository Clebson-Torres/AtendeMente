use sqlx::SqlitePool;

use crate::db::models::CalendarEvent;
use crate::errors::AppError;

pub async fn get_dashboard_data(
    db: &SqlitePool,
    user_id: &str,
) -> Result<(i64, Vec<CalendarEvent>, Vec<CalendarEvent>), AppError> {
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

    // Monthly stats
    let (count,) = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COUNT(*) FROM appointments
        WHERE user_id = ? AND deleted_at IS NULL
        AND status IN ('scheduled', 'completed')
        AND starts_at >= ? AND starts_at < ?"#,
    )
    .bind(user_id)
    .bind(&month_start)
    .bind(&next_month)
    .fetch_one(db)
    .await
    .map_err(|e| AppError::internal(format!("Dashboard stats error: {}", e)))?;

    // Today's appointments
    let today_start = format!("{}T00:00:00", now.format("%Y-%m-%d"));
    let today_end = format!("{}T23:59:59", now.format("%Y-%m-%d"));

    let todays = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
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
    .fetch_all(db)
    .await
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

    // Upcoming appointments (next 8)
    let upcoming = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        r#"SELECT a.id, a.patient_id, p.full_name, a.starts_at, a.ends_at, a.status, a.confirmation_status
        FROM appointments a
        INNER JOIN patients p ON p.id = a.patient_id
        WHERE a.user_id = ? AND a.deleted_at IS NULL
        AND a.starts_at >= ? AND a.status = 'scheduled'
        ORDER BY a.starts_at
        LIMIT 8"#,
    )
    .bind(user_id)
    .bind(&now.format("%Y-%m-%dT%H:%M:%S").to_string())
    .fetch_all(db)
    .await
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

    Ok((count, todays, upcoming))
}
