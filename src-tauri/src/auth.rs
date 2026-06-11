use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::crypto;
use crate::errors::AppError;
use crate::features::auth::AuthenticatedUser;
use crate::firebase;

#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken(String),
    ExpiredToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::MissingToken => {
                (StatusCode::UNAUTHORIZED, "Token de autenticacao nao informado.")
            }
            Self::InvalidToken(ref msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
            Self::ExpiredToken => {
                (StatusCode::UNAUTHORIZED, "Token expirado. Faca login novamente.")
            }
        };

        (
            status,
            Json(serde_json::json!({
                "success": false,
                "message": message,
            })),
        )
            .into_response()
    }
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| {
                AuthError::InvalidToken(
                    "Formato de token invalido. Use: Bearer <token>.".into(),
                )
            })?;

        if token.is_empty() {
            return Err(AuthError::InvalidToken("Token vazio.".into()));
        }

        // In dev mode, the token is just the user ID.
        // This will be replaced with proper Firebase JWT verification.
        Ok(AuthenticatedUser {
            id: token.to_string(),
            email: "dev@atendemente.local".into(),
            full_name: Some("Dev User".into()),
        })
    }
}

/// Verify a Firebase JWT token and return the authenticated user.
/// Falls back to dev mode if `DEV_MODE` env var is set.
pub async fn require_user(
    auth_header: Option<String>,
    db: &sqlx::SqlitePool,
    project_id: &str,
) -> Result<AuthenticatedUser, AppError> {
    let is_dev = std::env::var("DEV_MODE").is_ok();

    if is_dev {
        return dev_require_user(auth_header, db).await;
    }

    let header = auth_header.ok_or_else(|| AppError::unauthorized("Token nao informado."))?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::unauthorized("Formato invalido. Use: Bearer <token>."))?;

    let claims = firebase::verify_firebase_token(token, project_id).await?;
    let firebase_uid = &claims.sub;

    // Upsert user into local DB
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    sqlx::query(
        r#"INSERT INTO users (id, email, full_name, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            email = excluded.email,
            full_name = excluded.full_name,
            updated_at = excluded.updated_at"#,
    )
    .bind(firebase_uid)
    .bind(&claims.email)
    .bind(&claims.name)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao sincronizar usuario: {}", e)))?;

    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, email, full_name FROM users WHERE id = ?",
    )
    .bind(firebase_uid)
    .fetch_optional(db)
    .await
    .map_err(|_| AppError::internal("Erro ao buscar usuario."))?
    .ok_or_else(|| AppError::internal("Usuario nao encontrado apos criacao."))?;

    // Derive and cache crypto key automatically
    crypto::init_user_crypto(&row.0)?;

    Ok(AuthenticatedUser {
        id: row.0,
        email: row.1,
        full_name: row.2,
    })
}

/// Dev-mode authentication: token is the user ID directly.
/// Derives and caches the user's encryption key automatically.
pub async fn dev_require_user(
    auth_header: Option<String>,
    db: &sqlx::SqlitePool,
) -> Result<AuthenticatedUser, AppError> {
    let header = auth_header.ok_or_else(|| AppError::unauthorized("Token nao informado."))?;
    let user_id = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::unauthorized("Formato invalido."))?;

    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, email, full_name FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|_| AppError::internal("Erro ao buscar usuario."))?
    .ok_or_else(|| AppError::unauthorized("Usuario nao encontrado."))?;

    // Derive and cache crypto key for this user automatically
    crypto::init_user_crypto(&row.0)?;

    Ok(AuthenticatedUser {
        id: row.0,
        email: row.1,
        full_name: row.2,
    })
}
