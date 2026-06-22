pub mod auth_service;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;

use crate::audit::{self, AuditAction};
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
        .route("/auth/lock", post(lock_handler))
        .route("/auth/unlock", post(unlock_handler))
        .route("/auth/onboarding", patch(onboarding_handler))
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
    user_id: Option<String>,
    email: Option<String>,
    recovery_secret: String,
}

#[derive(Deserialize)]
struct ResetPasswordInput {
    reset_token: String,
    new_password: String,
}

#[derive(Deserialize)]
struct UnlockInput {
    password: String,
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
            "onboarding_completed": false,
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

    let result = match auth_service::login(&state.auth_db, &input.email, &input.password).await {
        Ok(r) => {
            let _ = audit::write_audit_event(
                &state.auth_db,
                &r.user_id,
                AuditAction::LoginSucceeded,
                "session",
                Some(&r.user_id),
                serde_json::json!({}),
                None,
            )
            .await;
            r
        }
        Err(e) => {
            let user_id = format!("unknown:{}", input.email);
            let _ = audit::write_audit_event(
                &state.auth_db,
                &user_id,
                AuditAction::LoginFailed,
                "session",
                None,
                serde_json::json!({"email": input.email}),
                None,
            )
            .await;
            return Err(AppError::unauthorized(e));
        }
    };

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
            "onboarding_completed": result.onboarding_completed,
        }),
    )))
}

async fn logout_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let user_id = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map(|(uid, _, _)| uid)
        .unwrap_or_default();
    auth_service::logout(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::internal(e))?;
    if !user_id.is_empty() {
        let _ = audit::write_audit_event(
            &state.auth_db,
            &user_id,
            AuditAction::Logout,
            "session",
            Some(&user_id),
            serde_json::json!({}),
            None,
        )
        .await;
    }
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

    let onboarding_completed = auth_service::get_onboarding_status(&state.auth_db, &user_id)
        .await
        .unwrap_or(false);

    // Re-open user's app DB (useful after page refresh)
    state.get_or_open_user_db(&user_id).await?;

    Ok(Json(ActionResponse::success(
        "",
        serde_json::json!({
            "user_id": user_id,
            "email": email,
            "full_name": full_name,
            "onboarding_completed": onboarding_completed,
        }),
    )))
}

async fn recover_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RecoverInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user_id = match (&input.user_id, &input.email) {
        (Some(uid), _) if !uid.is_empty() => uid.clone(),
        (_, Some(email)) if !email.is_empty() => {
            auth_service::find_user_id_by_email(&state.auth_db, email)
                .await
                .map_err(|e| AppError::not_found(e))?
        }
        _ => return Err(AppError::bad_request("Informe user_id ou email.")),
    };

    crate::rate_limit::enforce_rate_limit(
        &state.auth_db, "auth:password-reset", &user_id, 3, 900_000,
    )
    .await?;

    let result = auth_service::recover_with_secret(
        &state.auth_db,
        &user_id,
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

async fn lock_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    crate::crypto::clear_user_crypto(&user_id);
    state.clear_user_db_for_user(&user_id).await;

    let _ = audit::write_audit_event(
        &state.auth_db,
        &user_id,
        AuditAction::Locked,
        "session",
        Some(&user_id),
        serde_json::json!({}),
        None,
    )
    .await;

    Ok(Json(ActionResponse::<()>::success_empty("Tela bloqueada.")))
}

async fn unlock_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<UnlockInput>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    let password_valid = auth_service::verify_user_password(&state.auth_db, &user_id, &input.password)
        .await
        .map_err(|e| AppError::internal(e))?;

    if !password_valid {
        let _ = audit::write_audit_event(
            &state.auth_db,
            &user_id,
            AuditAction::LoginFailed,
            "session",
            Some(&user_id),
            serde_json::json!({"reason": "unlock_wrong_password"}),
            None,
        )
        .await;
        return Err(AppError::unauthorized("Senha incorreta."));
    }

    crate::crypto::init_user_crypto(&user_id)
        .map_err(|e| AppError::internal(format!("Erro ao reiniciar criptografia: {}", e)))?;

    state.get_or_open_user_db(&user_id).await?;

    let _ = audit::write_audit_event(
        &state.auth_db,
        &user_id,
        AuditAction::Unlocked,
        "session",
        Some(&user_id),
        serde_json::json!({}),
        None,
    )
    .await;

    Ok(Json(ActionResponse::<()>::success_empty("Tela desbloqueada.")))
}

async fn onboarding_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    auth_service::set_onboarding_completed(&state.auth_db, &user_id)
        .await
        .map_err(|e| AppError::internal(e))?;

    Ok(Json(ActionResponse::<()>::success_empty(
        "Onboarding concluido.",
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
