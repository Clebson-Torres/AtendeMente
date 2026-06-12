use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

const SESSION_TTL_DAYS: i64 = 7;
const RESET_TOKEN_TTL_MINUTES: i64 = 5;

pub struct AuthResult {
    pub user_id: String,
    pub email: String,
    pub full_name: String,
    pub token: String,
    pub recovery_secret: String,
}

pub struct RecoveryResult {
    pub user_id: String,
    pub reset_token: String,
}

/// Hash a password using Argon2id
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Erro ao hash da senha: {}", e))
}

/// Verify a password against an Argon2 hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| format!("Hash invalido: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Generate a recovery secret (32 random bytes, base64 encoded)
pub fn generate_recovery_secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
}

/// Hash the recovery secret for storage
pub fn hash_recovery_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Generate a session token (UUID v4 plaintext) and return both the token and its hash
pub fn generate_session_token() -> (String, String) {
    let token = Uuid::new_v4().to_string();
    let hash = hash_token(&token);
    (token, hash)
}

/// Hash a token for storage
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Register a new user
pub async fn register(
    db: &SqlitePool,
    email: &str,
    password: &str,
    full_name: &str,
) -> Result<AuthResult, String> {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Err("Email é obrigatório.".into());
    }
    if password.len() < 6 {
        return Err("A senha deve ter no mínimo 6 caracteres.".into());
    }
    let full_name = full_name.trim().to_string();
    if full_name.is_empty() {
        return Err("Nome é obrigatório.".into());
    }

    // Check if email already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM auth_users WHERE email = ?",
    )
    .bind(&email)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Erro ao verificar email: {}", e))?;

    if existing > 0 {
        return Err("Este email já está cadastrado.".into());
    }

    let user_id = Uuid::new_v4().to_string();
    let password_hash = hash_password(password)?;
    let recovery_secret = generate_recovery_secret();
    let recovery_hash = hash_recovery_secret(&recovery_secret);

    sqlx::query(
        r#"INSERT INTO auth_users (id, email, password_hash, full_name, recovery_secret_hash)
        VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&user_id)
    .bind(&email)
    .bind(&password_hash)
    .bind(&full_name)
    .bind(&recovery_hash)
    .execute(db)
    .await
    .map_err(|e| format!("Erro ao criar usuario: {}", e))?;

    let (token, token_hash) = generate_session_token();
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(SESSION_TTL_DAYS))
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    sqlx::query(
        r#"INSERT INTO sessions (id, user_id, token_hash, expires_at)
        VALUES (?, ?, ?, ?)"#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user_id)
    .bind(&token_hash)
    .bind(&expires_at)
    .execute(db)
    .await
    .map_err(|e| format!("Erro ao criar sessao: {}", e))?;

    Ok(AuthResult {
        user_id,
        email,
        full_name,
        token,
        recovery_secret,
    })
}

/// Login with email and password
pub async fn login(
    db: &SqlitePool,
    email: &str,
    password: &str,
) -> Result<AuthResult, String> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || password.is_empty() {
        return Err("Email e senha são obrigatórios.".into());
    }

    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, password_hash, full_name FROM auth_users WHERE email = ?",
    )
    .bind(&email)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao buscar usuario: {}", e))?
    .ok_or_else(|| "Email ou senha inválidos.".to_string())?;

    let (user_id, password_hash, full_name) = row;

    if !verify_password(password, &password_hash)? {
        return Err("Email ou senha inválidos.".into());
    }

    // Invalidate existing sessions for this user
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&user_id)
        .execute(db)
        .await
        .map_err(|e| format!("Erro ao limpar sessoes: {}", e))?;

    let (token, token_hash) = generate_session_token();
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(SESSION_TTL_DAYS))
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    sqlx::query(
        r#"INSERT INTO sessions (id, user_id, token_hash, expires_at)
        VALUES (?, ?, ?, ?)"#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user_id)
    .bind(&token_hash)
    .bind(&expires_at)
    .execute(db)
    .await
    .map_err(|e| format!("Erro ao criar sessao: {}", e))?;

    Ok(AuthResult {
        user_id,
        email,
        full_name,
        token,
        recovery_secret: String::new(),
    })
}

/// Validate a session token, return user info if valid
pub async fn validate_session(
    db: &SqlitePool,
    token: &str,
) -> Result<(String, String, String), String> {
    let token_hash = hash_token(token);

    let row = sqlx::query_as::<_, (String, String, String, String)>(
        r#"SELECT s.user_id, a.email, a.full_name, s.expires_at
        FROM sessions s
        JOIN auth_users a ON a.id = s.user_id
        WHERE s.token_hash = ?"#,
    )
    .bind(&token_hash)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao validar sessao: {}", e))?
    .ok_or_else(|| "Sessão inválida ou expirada.".to_string())?;

    let (user_id, email, full_name, expires_at_str) = row;

    let expires_at = chrono::NaiveDateTime::parse_from_str(&expires_at_str, "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| "Formato de data invalido.".to_string())?;
    let now = chrono::Utc::now().naive_utc();

    if now > expires_at {
        // Clean up expired session
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(db)
            .await;
        return Err("Sessão expirada. Faça login novamente.".into());
    }

    Ok((user_id, email, full_name))
}

/// Logout: invalidate session
pub async fn logout(db: &SqlitePool, token: &str) -> Result<(), String> {
    let token_hash = hash_token(token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(&token_hash)
        .execute(db)
        .await
        .map_err(|e| format!("Erro ao encerrar sessao: {}", e))?;
    Ok(())
}

/// Start password recovery using recovery file secret
pub async fn recover_with_secret(
    db: &SqlitePool,
    user_id: &str,
    recovery_secret: &str,
) -> Result<RecoveryResult, String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT recovery_secret_hash FROM auth_users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao buscar usuario: {}", e))?
    .ok_or_else(|| "Usuário não encontrado.".to_string())?;

    let stored_hash = row.0;
    let computed_hash = hash_recovery_secret(recovery_secret);

    if stored_hash != computed_hash {
        return Err("Chave de recuperação inválida.".into());
    }

    // Generate a one-time reset token (valid 5 min)
    let reset_token = Uuid::new_v4().to_string();
    let reset_token_hash = hash_token(&reset_token);
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::minutes(RESET_TOKEN_TTL_MINUTES))
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    // Store reset token as a session-like entry with special prefix
    sqlx::query(
        r#"INSERT INTO sessions (id, user_id, token_hash, expires_at)
        VALUES (?, ?, ?, ?)"#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(format!("reset:{}", &reset_token_hash))
    .bind(&expires_at)
    .execute(db)
    .await
    .map_err(|e| format!("Erro ao criar token de reset: {}", e))?;

    Ok(RecoveryResult {
        user_id: user_id.to_string(),
        reset_token,
    })
}

/// Reset password using a valid reset token
pub async fn reset_password(
    db: &SqlitePool,
    reset_token: &str,
    new_password: &str,
) -> Result<(), String> {
    if new_password.len() < 6 {
        return Err("A senha deve ter no mínimo 6 caracteres.".into());
    }

    let reset_token_hash = format!("reset:{}", hash_token(reset_token));

    // Find the reset session
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT user_id, expires_at FROM sessions WHERE token_hash = ?",
    )
    .bind(&reset_token_hash)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao validar token: {}", e))?
    .ok_or_else(|| "Token de reset inválido ou já utilizado.".to_string())?;

    let (user_id, expires_at_str) = row;

    let expires_at = chrono::NaiveDateTime::parse_from_str(&expires_at_str, "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| "Formato de data invalido.".to_string())?;
    let now = chrono::Utc::now().naive_utc();

    if now > expires_at {
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&reset_token_hash)
            .execute(db)
            .await;
        return Err("Token de reset expirado. Solicite novamente.".into());
    }

    let password_hash = hash_password(new_password)?;

    sqlx::query(
        "UPDATE auth_users SET password_hash = ?, recovery_secret_hash = NULL, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&password_hash)
    .bind(&user_id)
    .execute(db)
    .await
    .map_err(|e| format!("Erro ao redefinir senha: {}", e))?;

    // Delete the reset token and all sessions
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&user_id)
        .execute(db)
        .await
        .map_err(|e| format!("Erro ao limpar sessoes: {}", e))?;

    Ok(())
}
