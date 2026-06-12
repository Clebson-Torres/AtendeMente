pub mod auth_service;

use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::errors::{ActionResponse, AppError};
use crate::AppState;

pub fn create_auth_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
        .route("/auth/recover", post(recover_handler))
        .route("/auth/reset-password", post(reset_password_handler))
        .with_state(state)
}

#[derive(Deserialize)]
struct RegisterInput {
    email: String,
    password: String,
    full_name: String,
}

#[derive(Deserialize)]
struct LoginInput {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct RecoverInput {
    user_id: String,
    recovery_secret: String,
}

#[derive(Deserialize)]
struct ResetPasswordInput {
    reset_token: String,
    new_password: String,
}

async fn register_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RegisterInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    crate::rate_limit::enforce_rate_limit(
        &state.auth_db, "auth:register", &input.email, 3, 3600_000,
    )
    .await?;

    let result = auth_service::register(&state.auth_db, &input.email, &input.password, &input.full_name)
        .await
        .map_err(|e| AppError::bad_request(e))?;

    // Create user's app DB and run migrations
    let app_db_path = state.config.user_db_path(&result.user_id);
    let app_db = crate::db::init_database(&app_db_path)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao criar banco de dados: {}", e)))?;

    // Insert user record in their own app DB
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    sqlx::query(
        r#"INSERT INTO users (id, email, full_name, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&result.user_id)
    .bind(&result.email)
    .bind(&result.full_name)
    .bind(&now)
    .bind(&now)
    .execute(&app_db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao criar usuario no app DB: {}", e)))?;

    // Initialize crypto for the new user
    crate::crypto::init_user_crypto(&result.user_id)
        .map_err(|e| AppError::internal(format!("Erro ao iniciar criptografia: {}", e)))?;

    Ok(Json(ActionResponse::success(
        "Conta criada com sucesso!",
        serde_json::json!({
            "user_id": result.user_id,
            "email": result.email,
            "full_name": result.full_name,
            "token": result.token,
            "recovery_secret": result.recovery_secret,
        }),
    )))
}

async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LoginInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    crate::rate_limit::enforce_rate_limit(
        &state.auth_db, "auth:login", &input.email, 5, 600_000,
    )
    .await?;

    let result = auth_service::login(&state.auth_db, &input.email, &input.password)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    // Initialize crypto for this user
    crate::crypto::init_user_crypto(&result.user_id)
        .map_err(|e| AppError::internal(format!("Erro ao iniciar criptografia: {}", e)))?;

    // Open user's app DB
    state.get_or_open_user_db(&result.user_id).await?;

    Ok(Json(ActionResponse::success(
        "Login realizado com sucesso!",
        serde_json::json!({
            "user_id": result.user_id,
            "email": result.email,
            "full_name": result.full_name,
            "token": result.token,
        }),
    )))
}

async fn logout_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    auth_service::logout(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::internal(e))?;
    state.clear_user_db().await;
    Ok(Json(ActionResponse::<()>::success_empty("Sessão encerrada.")))
}

async fn me_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, email, full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    // Re-open user's app DB (useful after page refresh)
    state.get_or_open_user_db(&user_id).await?;

    Ok(Json(ActionResponse::success(
        "",
        serde_json::json!({
            "user_id": user_id,
            "email": email,
            "full_name": full_name,
        }),
    )))
}

async fn recover_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RecoverInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    crate::rate_limit::enforce_rate_limit(
        &state.auth_db, "auth:password-reset", &input.user_id, 3, 900_000,
    )
    .await?;

    let result = auth_service::recover_with_secret(
        &state.auth_db,
        &input.user_id,
        &input.recovery_secret,
    )
    .await
    .map_err(|e| AppError::unauthorized(e))?;

    Ok(Json(ActionResponse::success(
        "Chave verificada. Crie uma nova senha.",
        serde_json::json!({
            "reset_token": result.reset_token,
        }),
    )))
}

async fn reset_password_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ResetPasswordInput>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    auth_service::reset_password(&state.auth_db, &input.reset_token, &input.new_password)
        .await
        .map_err(|e| AppError::bad_request(e))?;

    Ok(Json(ActionResponse::<()>::success_empty(
        "Senha redefinida com sucesso! Faca login novamente.",
    )))
}

fn extract_bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Token nao informado."))?;

    auth_header
        .strip_prefix("Bearer ")
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::unauthorized("Formato invalido. Use: Bearer <token>."))
}
