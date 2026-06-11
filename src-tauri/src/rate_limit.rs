use sqlx::SqlitePool;
use uuid::Uuid;

use crate::errors::AppError;

pub struct RateLimitConfig {
    pub scope: &'static str,
    pub limit: i64,
    pub window_ms: i64,
}

pub const RATE_LIMITS: &[RateLimitConfig] = &[
    RateLimitConfig {
        scope: "auth:login",
        limit: 5,
        window_ms: 10 * 60 * 1000,
    },
    RateLimitConfig {
        scope: "auth:password-reset",
        limit: 3,
        window_ms: 15 * 60 * 1000,
    },
    RateLimitConfig {
        scope: "auth:invite",
        limit: 3,
        window_ms: 30 * 60 * 1000,
    },
    RateLimitConfig {
        scope: "upload",
        limit: 20,
        window_ms: 60 * 60 * 1000,
    },
    RateLimitConfig {
        scope: "import",
        limit: 5,
        window_ms: 60 * 60 * 1000,
    },
    RateLimitConfig {
        scope: "export",
        limit: 10,
        window_ms: 60 * 60 * 1000,
    },
];

pub async fn enforce_rate_limit(
    db: &SqlitePool,
    scope: &str,
    identifier: &str,
    limit: i64,
    window_ms: i64,
) -> Result<(), AppError> {
    let now = chrono::Utc::now();
    let now_iso = now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let existing = sqlx::query_as::<_, (String, i64, String)>(
        r#"SELECT id, hits, window_starts_at FROM request_limits
        WHERE scope = ? AND identifier = ?"#,
    )
    .bind(scope)
    .bind(identifier)
    .fetch_optional(db)
    .await
    .map_err(|_| AppError::internal("Rate limit check failed."))?;

    let (id, hits, window_starts_at) = match existing {
        Some(row) => row,
        None => {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"INSERT INTO request_limits (id, scope, identifier, hits, window_starts_at)
                VALUES (?, ?, ?, 1, ?)"#,
            )
            .bind(&id)
            .bind(scope)
            .bind(identifier)
            .bind(&now_iso)
            .execute(db)
            .await
            .map_err(|_| AppError::internal("Rate limit insert failed."))?;
            return Ok(());
        }
    };

    let window_start = chrono::DateTime::parse_from_rfc3339(&window_starts_at)
        .map_err(|_| AppError::internal("Invalid rate limit timestamp."))?;
    let elapsed = (now - window_start.to_utc()).num_milliseconds();

    if elapsed >= window_ms {
        sqlx::query(
            r#"UPDATE request_limits SET hits = 1, window_starts_at = ?, updated_at = ?
            WHERE id = ?"#,
        )
        .bind(&now_iso)
        .bind(&now_iso)
        .bind(&id)
        .execute(db)
        .await
        .map_err(|_| AppError::internal("Rate limit update failed."))?;
        return Ok(());
    }

    if hits >= limit {
        return Err(AppError::rate_limited(
            "Muitas tentativas em pouco tempo. Tente novamente em alguns minutos.",
        ));
    }

    sqlx::query(
        r#"UPDATE request_limits SET hits = hits + 1, updated_at = ? WHERE id = ?"#,
    )
    .bind(&now_iso)
    .bind(&id)
    .execute(db)
    .await
    .map_err(|_| AppError::internal("Rate limit update failed."))?;

    Ok(())
}
