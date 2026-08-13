use sqlx::SqlitePool;
use uuid::Uuid;

use crate::errors::AppError;

pub struct RateLimitConfig {
    pub scope: &'static str,
    pub limit: i64,
    pub window_ms: i64,
}

/// Os escopos com limite, como enum e nao como string.
///
/// Antes havia uma constante `RATE_LIMITS` que **ninguem lia**: cada chamada
/// repetia escopo, limite e janela como literais. Duas consequencias reais:
///
/// - Os numeros divergiam da tabela sem que nada acusasse, e escopos novos que
///   eu mesmo adicionei (`auth:register`, `auth:unlock`, ...) nunca entraram nela.
/// - **Um erro de digitacao no escopo cria um contador separado em silencio** —
///   o limite simplesmente deixa de valer, e nada falha para avisar.
///
/// Com enum, o compilador garante que o escopo existe e os numeros vivem num
/// unico lugar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Login,
    Register,
    PasswordReset,
    /// Desbloqueio de tela: mesmo peso do login, porque e o que devolve a chave
    /// de dados.
    Unlock,
    RecoveryRotate,
    /// Rotacao da chave de dados: cada tentativa gera um backup completo e pode
    /// re-cifrar todo o acervo.
    RotateKey,
    Upload,
    Import,
    Export,
    /// Restaurar substitui banco e anexos.
    BackupRestore,
}

impl Scope {
    pub fn key(&self) -> &'static str {
        match self {
            Scope::Login => "auth:login",
            Scope::Register => "auth:register",
            Scope::PasswordReset => "auth:password-reset",
            Scope::Unlock => "auth:unlock",
            Scope::RecoveryRotate => "auth:recovery-rotate",
            Scope::RotateKey => "auth:rotate-key",
            Scope::Upload => "upload",
            Scope::Import => "import",
            Scope::Export => "export",
            Scope::BackupRestore => "backup:restore",
        }
    }

    /// `(limite, janela em ms)`
    pub fn budget(&self) -> (i64, i64) {
        const MIN: i64 = 60 * 1000;
        match self {
            Scope::Login => (5, 10 * MIN),
            Scope::Register => (3, 60 * MIN),
            Scope::PasswordReset => (5, 15 * MIN),
            Scope::Unlock => (5, 10 * MIN),
            Scope::RecoveryRotate => (5, 15 * MIN),
            Scope::RotateKey => (3, 60 * MIN),
            Scope::Upload => (20, 60 * MIN),
            Scope::Import => (5, 60 * MIN),
            Scope::Export => (10, 60 * MIN),
            Scope::BackupRestore => (5, 60 * MIN),
        }
    }
}

/// Aplica o limite do escopo. Prefira esta forma a `enforce_rate_limit`, que
/// recebe os numeros soltos e permite que eles divirjam da tabela.
pub async fn enforce(db: &SqlitePool, scope: Scope, identifier: &str) -> Result<(), AppError> {
    let (limit, window_ms) = scope.budget();
    enforce_rate_limit(db, scope.key(), identifier, limit, window_ms).await
}

pub async fn enforce_rate_limit(
    db: &SqlitePool,
    scope: &str,
    identifier: &str,
    limit: i64,
    window_ms: i64,
) -> Result<(), AppError> {
    let now = chrono::Utc::now();
    let now_iso = now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    // Limpeza oportunista: `request_limits` nunca era podada, e cada
    // (escopo, identificador) deixava uma linha para sempre. Como o identificador
    // do login e o e-mail digitado, uma varredura de credenciais fazia a tabela
    // crescer sem limite — e cada e-mail tentado ficava registrado ali.
    //
    // Uma janela encerrada ha mais de 24h nao influencia nenhuma decisao, entao
    // pode sair. E um DELETE indexado por chamada, irrelevante nesta escala.
    let corte = (now - chrono::Duration::hours(24))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let _ = sqlx::query("DELETE FROM request_limits WHERE window_starts_at < ?")
        .bind(&corte)
        .execute(db)
        .await;

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
    use super::{enforce, enforce_rate_limit, Scope};
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

    /// Cada escopo tem chave e orcamento proprios, e nenhum repete a chave de
    /// outro — duas variantes com a mesma string compartilhariam o contador em
    /// silencio, e o limite de uma valeria pela outra.
    #[test]
    fn cada_escopo_tem_chave_unica_e_orcamento_valido() {
        let todos = [
            Scope::Login, Scope::Register, Scope::PasswordReset, Scope::Unlock,
            Scope::RecoveryRotate, Scope::RotateKey, Scope::Upload, Scope::Import,
            Scope::Export, Scope::BackupRestore,
        ];
        let mut chaves: Vec<&str> = todos.iter().map(|s| s.key()).collect();
        let antes = chaves.len();
        chaves.sort_unstable();
        chaves.dedup();
        assert_eq!(chaves.len(), antes, "ha escopos com a mesma chave");

        for s in todos {
            let (limite, janela) = s.budget();
            assert!(limite > 0, "{:?} sem limite", s);
            assert!(janela > 0, "{:?} sem janela", s);
        }
    }

    /// O desbloqueio de tela precisa ser tao restrito quanto o login: e ele que
    /// devolve a chave de dados, e a protecao no cliente zera a cada refresh.
    #[test]
    fn unlock_e_tao_restrito_quanto_o_login() {
        assert_eq!(Scope::Unlock.budget(), Scope::Login.budget());
    }

    #[tokio::test]
    async fn enforce_por_escopo_aplica_o_orcamento_da_tabela() {
        let (_d, db) = app_db().await;
        let (limite, _) = Scope::Login.budget();

        for i in 1..=limite {
            enforce(&db, Scope::Login, "x@y.com")
                .await
                .unwrap_or_else(|e| panic!("tentativa {i} deveria passar: {e}"));
        }
        assert!(
            enforce(&db, Scope::Login, "x@y.com").await.is_err(),
            "passar do limite da tabela tem de bloquear"
        );
        // Escopo diferente nao compartilha contador.
        assert!(enforce(&db, Scope::Unlock, "x@y.com").await.is_ok());
    }

    /// `request_limits` nunca era podada. Como o identificador do login e o
    /// e-mail digitado, uma varredura de credenciais fazia a tabela crescer sem
    /// limite — e deixava cada e-mail tentado registrado.
    #[tokio::test]
    async fn janelas_antigas_sao_removidas() {
        let (_d, db) = app_db().await;
        enforce(&db, Scope::Login, "atual@x.com").await.unwrap();

        // Uma linha com janela de 48h atras, como sobra de uma varredura antiga.
        sqlx::query(
            "INSERT INTO request_limits (id, scope, identifier, hits, window_starts_at) \
             VALUES ('velha', 'auth:login', 'antigo@x.com', 99, ?)",
        )
        .bind(
            (chrono::Utc::now() - chrono::Duration::hours(48))
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
        )
        .execute(&db)
        .await
        .unwrap();

        enforce(&db, Scope::Login, "outro@x.com").await.unwrap();

        let sobrou: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM request_limits WHERE id = 'velha'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(sobrou, 0, "a janela de 48h atras deveria ter sido removida");

        // E a janela em uso continua lá.
        let atual: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM request_limits WHERE identifier = 'atual@x.com'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(atual, 1, "a janela recente nao pode ser removida");
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
