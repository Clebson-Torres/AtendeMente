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

/// Argon2 hash of a throwaway password, used to spend the same work on a
/// non-existent account as on a real one. Without this, `login` returns before
/// hashing when the address is unknown, which times account existence.
static DUMMY_PASSWORD_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn dummy_password_hash() -> &'static str {
    DUMMY_PASSWORD_HASH.get_or_init(|| {
        hash_password("senha-inexistente-para-equalizar-tempo")
            .unwrap_or_else(|_| String::new())
    })
}

/// Length-independent comparison for hex digests, so a wrong recovery code
/// cannot be narrowed down by response time.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub struct AuthResult {
    pub user_id: String,
    pub email: String,
    pub full_name: String,
    pub token: String,
    pub recovery_secret: String,
    pub onboarding_completed: bool,
}

#[derive(Debug)]
pub struct RecoveryResult {
    pub user_id: String,
    pub reset_token: String,
}

/// Outcome of a password reset. `recovery_secret` replaces the code that was
/// just consumed, and `user_id` lets the client write a recovery file in the
/// same shape as the one produced at registration.
#[derive(Debug)]
pub struct ResetResult {
    pub user_id: String,
    pub recovery_secret: String,
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

/// Gera um codigo de recuperacao: 16 bytes aleatorios em hex, agrupados de 4 em
/// 4 (`XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`).
///
/// Eram 8 bytes — 64 bits. Isso bastava enquanto o codigo so servia para
/// redefinir a senha, com rate limit de 3 tentativas a cada 15 minutos pela
/// rede. Passa a nao bastar quando ele protege uma copia da chave de dados: o
/// material embrulhado viaja dentro de todo backup, entao o ataque deixa de ser
/// online e passa a ser offline, com o arquivo em maos. Contra isso, 2^64 e
/// alcancavel para quem tem GPU. Com 128 bits, nao e.
pub fn generate_recovery_secret() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let hex_str: String = bytes.iter().map(|b| format!("{:02X}", b)).collect();
    hex_str
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("-")
}

/// Normaliza o codigo antes de hashear ou derivar: maiusculas e sem separadores.
///
/// Assim o usuario pode digitar com ou sem hifens, em qualquer caixa, e o
/// resultado e o mesmo — o que importa quando o codigo e a unica coisa entre
/// ele e o prontuario.
pub fn normalize_recovery_secret(secret: &str) -> String {
    secret
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Hash do codigo de recuperacao, com Argon2id.
///
/// Era SHA-256 puro, sem sal e sem alongamento — para um segredo de 64 bits,
/// isso e uma tabela de busca para quem tiver o banco. Agora usa o mesmo
/// esquema PHC da senha.
pub fn hash_recovery_secret(secret: &str) -> String {
    let normalizado = normalize_recovery_secret(secret);
    match hash_password(&normalizado) {
        Ok(h) => h,
        // hash_password so falha por erro interno do Argon2; cair no SHA-256
        // antigo aqui manteria o cadastro funcionando em vez de derrubar o
        // registro inteiro, e a verificacao aceita os dois formatos.
        Err(_) => legacy_sha256_recovery_hash(&normalizado),
    }
}

fn legacy_sha256_recovery_hash(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verifica o codigo contra o hash guardado, aceitando os dois formatos.
///
/// Os valores antigos sao 64 caracteres hex; os novos comecam com `$argon2id$`.
/// Ramificar pelo prefixo evita uma migration e mantem funcionando o codigo que
/// o usuario ja tem anotado.
///
/// O formato antigo era hasheado sem normalizacao, entao a comparacao legada
/// tenta tambem o valor cru — senao um codigo valido de antes deixaria de ser
/// aceito por causa de um hifen.
pub fn verify_recovery_secret(secret: &str, stored_hash: &str) -> bool {
    if stored_hash.starts_with("$argon2") {
        let normalizado = normalize_recovery_secret(secret);
        return verify_password(&normalizado, stored_hash).unwrap_or(false);
    }
    let normalizado = normalize_recovery_secret(secret);
    constant_time_eq(&legacy_sha256_recovery_hash(&normalizado), stored_hash)
        || constant_time_eq(&legacy_sha256_recovery_hash(secret), stored_hash)
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
    if password.len() < 8 {
        return Err("A senha deve ter no mínimo 8 caracteres.".into());
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
        .ok_or_else(|| "Erro interno: overflow ao calcular expiracao da sessao.")?
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
        onboarding_completed: false,
    })
}

/// Get onboarding status for a user
pub async fn get_onboarding_status(
    db: &SqlitePool,
    user_id: &str,
) -> Result<bool, String> {
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT onboarding_completed FROM auth_users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao buscar status de onboarding: {}", e))?
    .ok_or_else(|| "Usuário não encontrado.".to_string())?;

    Ok(result == 1)
}

/// Mark onboarding as completed for a user
pub async fn set_onboarding_completed(
    db: &SqlitePool,
    user_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE auth_users SET onboarding_completed = 1, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| format!("Erro ao atualizar onboarding: {}", e))?;

    Ok(())
}

pub async fn login(
    db: &SqlitePool,
    email: &str,
    password: &str,
) -> Result<AuthResult, String> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || password.is_empty() {
        return Err("Email e senha são obrigatórios.".into());
    }

    let row = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT id, password_hash, full_name, onboarding_completed FROM auth_users WHERE email = ?",
    )
    .bind(&email)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao buscar usuario: {}", e))?;

    let (user_id, password_hash, full_name, onboarding_completed) = match row {
        Some(row) => row,
        None => {
            // Spend the same Argon2 work as a real verification before failing,
            // so response time does not reveal whether the account exists.
            let _ = verify_password(password, dummy_password_hash());
            return Err("Email ou senha inválidos.".into());
        }
    };

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
        .ok_or_else(|| "Erro interno: overflow ao calcular expiracao da sessao.")?
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
        onboarding_completed: onboarding_completed == 1,
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

/// Verify a user's password by looking up their stored hash
pub async fn verify_user_password(
    db: &SqlitePool,
    user_id: &str,
    password: &str,
) -> Result<bool, String> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT password_hash FROM auth_users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao buscar usuario: {}", e))?
    .ok_or_else(|| "Usuário não encontrado.".to_string())?;

    verify_password(password, &row)
}

/// Look up a user's ID by their email address
pub async fn find_user_id_by_email(db: &SqlitePool, email: &str) -> Result<String, String> {
    let email = email.trim().to_lowercase();
    sqlx::query_scalar::<_, String>("SELECT id FROM auth_users WHERE email = ?")
        .bind(&email)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("Erro ao buscar email: {}", e))?
        .ok_or_else(|| "Nenhuma conta encontrada com este email.".to_string())
}

/// Start password recovery using recovery file secret
pub async fn recover_with_secret(
    db: &SqlitePool,
    user_id: &str,
    recovery_secret: &str,
) -> Result<RecoveryResult, String> {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT recovery_secret_hash FROM auth_users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Erro ao buscar usuario: {}", e))?
    .ok_or_else(|| "Usuário não encontrado.".to_string())?;

    let stored_hash = row.0.ok_or_else(|| {
        "Este código de recuperação já foi utilizado. Cada código só pode ser usado uma vez.".to_string()
    })?;

    // Precisa passar por `verify_recovery_secret`, e nao por comparacao de
    // hashes: com Argon2 o sal e aleatorio, entao hashear o mesmo codigo duas
    // vezes produz strings diferentes e a igualdade nunca casaria. A funcao
    // ramifica pelo prefixo e aceita tambem os hashes SHA-256 antigos, para o
    // codigo que o usuario ja tem anotado continuar valendo.
    if !verify_recovery_secret(recovery_secret, &stored_hash) {
        return Err("Chave de recuperação inválida.".into());
    }

    // Generate a one-time reset token (valid 5 min)
    let reset_token = Uuid::new_v4().to_string();
    let reset_token_hash = hash_token(&reset_token);
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::minutes(RESET_TOKEN_TTL_MINUTES))
        .ok_or_else(|| "Erro interno: overflow ao calcular expiracao do token.".to_string())?
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

/// Reset password using a valid reset token.
///
/// Consumes the recovery code that authorised this reset and issues a fresh one,
/// which is returned so the caller can show it to the user. The old code stops
/// working the moment the password changes — previously it stayed valid forever,
/// contradicting the "single use" promise shown during onboarding.
pub async fn reset_password(
    db: &SqlitePool,
    reset_token: &str,
    new_password: &str,
) -> Result<ResetResult, String> {
    if new_password.len() < 8 {
        return Err("A senha deve ter no mínimo 8 caracteres.".into());
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

    // Rotate the recovery code in the same statement as the password: the code
    // that authorised this reset is now spent, and the user gets a replacement
    // so they are not left without a recovery path.
    let new_recovery_secret = generate_recovery_secret();
    let new_recovery_hash = hash_recovery_secret(&new_recovery_secret);

    sqlx::query(
        "UPDATE auth_users SET password_hash = ?, recovery_secret_hash = ?, \
         updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&password_hash)
    .bind(&new_recovery_hash)
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

    Ok(ResetResult {
        user_id,
        recovery_secret: new_recovery_secret,
    })
}
