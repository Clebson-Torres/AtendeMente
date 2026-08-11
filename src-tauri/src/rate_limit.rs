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

    let mut tx = db.begin()
        .await
        .map_err(|_| AppError::internal("Rate limit transaction failed."))?;

    let existing = sqlx::query_as::<_, (String, i64, String)>(
        r#"SELECT id, hits, window_starts_at FROM request_limits
        WHERE scope = ? AND identifier = ?"#,
    )
    .bind(scope)
    .bind(identifier)
    .fetch_optional(&mut *tx)
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
            .execute(&mut *tx)
            .await
            .map_err(|_| AppError::internal("Rate limit insert failed."))?;
            tx.commit().await.map_err(|_| AppError::internal("Rate limit commit failed."))?;
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
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("Rate limit update failed."))?;
        tx.commit().await.map_err(|_| AppError::internal("Rate limit commit failed."))?;
        return Ok(());
    }

    if hits >= limit {
        tx.rollback().await.ok();
        return Err(AppError::rate_limited(
            "Muitas tentativas em pouco tempo. Tente novamente em alguns minutos.",
        ));
    }

    sqlx::query(
        r#"UPDATE request_limits SET hits = hits + 1, updated_at = ? WHERE id = ?"#,
    )
    .bind(&now_iso)
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("Rate limit update failed."))?;

    tx.commit().await.map_err(|_| AppError::internal("Rate limit commit failed."))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::enforce_rate_limit;
    use crate::db;
    use crate::errors::AppError;
    use sqlx::SqlitePool;

    async fn app_db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("rl.db").to_string_lossy()
        );
        (dir, db::init_database(&url).await.unwrap())
    }

    /// Backdates the window so an elapsed window can be tested without sleeping.
    async fn backdate_window(db: &SqlitePool, scope: &str, identifier: &str, ms_ago: i64) {
        let when = (chrono::Utc::now() - chrono::Duration::milliseconds(ms_ago))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        sqlx::query("UPDATE request_limits SET window_starts_at = ? WHERE scope = ? AND identifier = ?")
            .bind(&when)
            .bind(scope)
            .bind(identifier)
            .execute(db)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn allows_up_to_the_limit_then_blocks() {
        let (_d, db) = app_db().await;

        for i in 1..=3 {
            enforce_rate_limit(&db, "auth:login", "a@b.com", 3, 600_000)
                .await
                .unwrap_or_else(|e| panic!("tentativa {i} deveria passar: {e}"));
        }

        let err = enforce_rate_limit(&db, "auth:login", "a@b.com", 3, 600_000)
            .await
            .expect_err("a 4a tentativa deve ser bloqueada");
        assert!(
            matches!(err, AppError::RateLimited { .. }),
            "esperava RateLimited, obtido: {err:?}"
        );
    }

    /// Brute-forcing one account must not lock out another.
    #[tokio::test]
    async fn identifiers_are_independent() {
        let (_d, db) = app_db().await;

        for _ in 0..3 {
            enforce_rate_limit(&db, "auth:login", "vitima@b.com", 3, 600_000)
                .await
                .unwrap();
        }
        assert!(enforce_rate_limit(&db, "auth:login", "vitima@b.com", 3, 600_000)
            .await
            .is_err());

        // Outro e-mail comeca do zero.
        enforce_rate_limit(&db, "auth:login", "outro@b.com", 3, 600_000)
            .await
            .expect("identificador diferente nao deve herdar o bloqueio");
    }

    /// Hitting the export limit must not block logging in.
    #[tokio::test]
    async fn scopes_are_independent() {
        let (_d, db) = app_db().await;
        let who = "550e8400-e29b-41d4-a716-446655440001";

        for _ in 0..2 {
            enforce_rate_limit(&db, "export", who, 2, 600_000).await.unwrap();
        }
        assert!(enforce_rate_limit(&db, "export", who, 2, 600_000).await.is_err());

        enforce_rate_limit(&db, "auth:login", who, 2, 600_000)
            .await
            .expect("escopo diferente nao deve herdar o bloqueio");
    }

    #[tokio::test]
    async fn window_expiry_resets_the_counter() {
        let (_d, db) = app_db().await;

        for _ in 0..2 {
            enforce_rate_limit(&db, "auth:login", "a@b.com", 2, 600_000).await.unwrap();
        }
        assert!(enforce_rate_limit(&db, "auth:login", "a@b.com", 2, 600_000)
            .await
            .is_err());

        // Janela de 10 min ja passou.
        backdate_window(&db, "auth:login", "a@b.com", 600_001).await;

        enforce_rate_limit(&db, "auth:login", "a@b.com", 2, 600_000)
            .await
            .expect("apos a janela expirar deve liberar");

        // E o contador reiniciou de fato: cabe mais uma antes de bloquear.
        enforce_rate_limit(&db, "auth:login", "a@b.com", 2, 600_000)
            .await
            .expect("contador deve ter reiniciado em 1");
        assert!(enforce_rate_limit(&db, "auth:login", "a@b.com", 2, 600_000)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_still_open_window_keeps_blocking() {
        let (_d, db) = app_db().await;
        enforce_rate_limit(&db, "auth:login", "a@b.com", 1, 600_000).await.unwrap();
        // Quase expirando, mas ainda dentro.
        backdate_window(&db, "auth:login", "a@b.com", 599_000).await;
        assert!(
            enforce_rate_limit(&db, "auth:login", "a@b.com", 1, 600_000).await.is_err(),
            "janela ainda aberta deve continuar bloqueando"
        );
    }

    /// The auth database must carry `request_limits` too: login and register are
    /// rate limited against it, and a missing table there would silently disable
    /// the only brute-force protection the app has.
    #[tokio::test]
    async fn auth_database_supports_rate_limiting() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("auth.db").to_string_lossy()
        );
        let auth_db = db::init_auth_database(&url).await.unwrap();

        for _ in 0..5 {
            enforce_rate_limit(&auth_db, "auth:login", "a@b.com", 5, 600_000)
                .await
                .expect("request_limits deve existir no banco de auth");
        }
        assert!(enforce_rate_limit(&auth_db, "auth:login", "a@b.com", 5, 600_000)
            .await
            .is_err());
    }
}
