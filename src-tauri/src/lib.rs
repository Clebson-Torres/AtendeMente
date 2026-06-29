pub mod api;
pub mod audit;
#[cfg(test)]
mod audit_tests;
pub mod auth;
pub mod commands;
pub mod config;
pub mod crypto;
pub mod db;
pub mod errors;
pub mod features;
pub mod middleware;
pub mod rate_limit;
pub mod utils;

use std::collections::HashMap;
use std::sync::Arc;

use axum::{body::Body, extract::DefaultBodyLimit, http::StatusCode, middleware as axum_middleware, response::Response, Router};
use std::path::PathBuf;
use sqlx::SqlitePool;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::errors::AppError;

pub struct AppState {
    pub config: config::AppConfig,
    pub auth_db: SqlitePool,
    pub user_dbs: RwLock<HashMap<String, SqlitePool>>,
}

impl AppState {
    pub async fn get_or_open_user_db(&self, user_id: &str) -> Result<SqlitePool, AppError> {
        {
            let guard = self.user_dbs.read().await;
            if let Some(pool) = guard.get(user_id) {
                if sqlx::query("SELECT 1").execute(pool).await.is_ok() {
                    return Ok(pool.clone());
                }
            }
        }

        let db_path = self.config.user_db_path(user_id);
        let pool = crate::db::init_database(&db_path)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao abrir banco de dados: {}", e)))?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        sqlx::query(
            r#"INSERT OR IGNORE INTO users (id, email, full_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(user_id)
        .bind("")
        .bind("")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao sincronizar usuario: {}", e)))?;

        let mut guard = self.user_dbs.write().await;
        guard.insert(user_id.to_string(), pool.clone());
        Ok(pool)
    }

    pub async fn clear_user_db(&self) {
        let mut guard = self.user_dbs.write().await;
        guard.clear();
    }

    pub async fn clear_user_db_for_user(&self, user_id: &str) {
        let mut guard = self.user_dbs.write().await;
        guard.remove(user_id);
    }
}

fn resolve_frontend_dist() -> PathBuf {
    if let Ok(path) = std::env::var("FRONTEND_DIST") {
        return PathBuf::from(path);
    }

    // Resolve relativo ao diretório do executável
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // server está em target/debug/server → sobe 2 níveis para raiz do projeto
            let candidate = exe_dir.join("../../dist");
            if candidate.join("index.html").exists() {
                return candidate;
            }
            // server está em target/release/server → mesmo esquema
            let candidate = exe_dir.join("../../../dist");
            if candidate.join("index.html").exists() {
                return candidate;
            }
        }
    }

    // Fallback: CWD relativo (para compatibilidade)
    PathBuf::from("../dist")
}

fn start_backup_scheduler(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let users: Vec<String> = match sqlx::query_scalar(
                "SELECT id FROM auth_users",
            )
            .fetch_all(&state.auth_db)
            .await
            {
                Ok(u) => u,
                Err(_) => continue,
            };
            for user_id in &users {
                let db = match state.get_or_open_user_db(user_id).await {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let config = match crate::features::backup::get_backup_config(&db, user_id).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let should_backup = match config.frequency.as_str() {
                    "never" => false,
                    "daily" => {
                        let last = config.last_backup_at.as_deref().unwrap_or("");
                        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                        !last.starts_with(&today)
                    }
                    "weekly" => {
                        let last = config.last_backup_at.as_deref().unwrap_or("");
                        let now = chrono::Utc::now();
                        let week_start = (now - chrono::Duration::days(7))
                            .format("%Y-%m-%d")
                            .to_string();
                        last < week_start.as_str()
                    }
                    _ => false,
                };
                if should_backup {
                    match crate::features::backup::create_backup(&db, &state.config, user_id).await {
                        Ok(bundle) => {
                            let _ = crate::features::backup::touch_backup_timestamp(&db, user_id).await;
                            tracing::info!("Backup automatico criado: {}", bundle.file_name);
                        }
                        Err(e) => {
                            tracing::warn!("Falha no backup automatico para {}: {}", user_id, e);
                        }
                    }
                }
            }
        }
    });
}

pub async fn run_server(state: Arc<AppState>, _app: Option<AppHandle>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    start_backup_scheduler(state.clone());
    let auth_router = crate::auth::create_auth_router(state.clone());
    let api_routes = api::routes::create_router(state.clone());

    let frontend_dist: PathBuf = resolve_frontend_dist();
    if !frontend_dist.join("index.html").exists() {
        tracing::warn!(
            "FRONTEND_DIST={} não encontrada. Execute 'npm run build' antes ou use --dist <caminho>.",
            frontend_dist.display()
        );
    }

    let dist = frontend_dist.clone();
    let app = Router::new()
        .nest("/api", auth_router)
        .nest("/api", api_routes)
        .fallback_service(
            tower::service_fn(move |_req: axum::http::Request<Body>| {
                let dist = dist.clone();
                async move {
                    let path = _req.uri().path().trim_start_matches('/');
                    let file_path = dist.join(path);
                    if file_path.exists() && file_path.is_file() {
                        match tokio::fs::read(&file_path).await {
                            Ok(data) => {
                                let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
                                Ok(Response::builder()
                                    .header("content-type", mime.as_ref())
                                    .body(Body::from(data))
                                    .unwrap_or_else(|_| Response::new(Body::from("Internal error"))))
                            }
                            Err(_) => Ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::from("Not found"))
                                .unwrap_or_else(|_| Response::new(Body::from("Not found")))),
                        }
                    } else {
                        let index_path = dist.join("index.html");
                        match tokio::fs::read_to_string(&index_path).await {
                            Ok(html) => Ok(Response::new(Body::from(html))),
                            Err(_) => Ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::from("Not found"))
                                .unwrap_or_else(|_| Response::new(Body::from("Not found")))),
                        }
                    }
                }
            }),
        )
        .route_layer(axum_middleware::from_fn(crate::middleware::security_headers))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::PATCH,
                ])
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(false),
        );

    let bind_ip = if state.config.mobile_access_enabled { "0.0.0.0" } else { "127.0.0.1" };
    let addr = format!("{}:{}", bind_ip, state.config.server_port);
    tracing::info!("Starting API server on {} (mobile_access={})", addr, state.config.mobile_access_enabled);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| {
            tracing::error!("Failed to bind address {}: {}", addr, e);
            e
        })?;

    axum::serve(listener, app)
        .await
        .map_err(|e| {
            tracing::error!("Server failed: {}", e);
            e
        })?;
    Ok(())
}

pub fn add_firewall_rule() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Nao foi possivel obter o caminho do executavel: {}", e))?
        .to_string_lossy()
        .to_string();

    let output = std::process::Command::new("netsh")
        .args([
            "advfirewall", "firewall", "add", "rule",
            "name=AtendeMente",
            "dir=in",
            "action=allow",
            &format!("program={}", exe_path),
            "enable=yes",
        ])
        .output()
        .map_err(|e| format!("Falha ao executar netsh: {}", e))?;

    if output.status.success() {
        tracing::info!("[Mobile] Regra de firewall adicionada para: {}", exe_path);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("[Mobile] Falha ao adicionar regra de firewall: {}", stderr);
        Err(format!("Falha ao adicionar regra de firewall: {}", stderr))
    }
}

pub fn remove_firewall_rule() -> Result<(), String> {
    let output = std::process::Command::new("netsh")
        .args([
            "advfirewall", "firewall", "delete", "rule",
            "name=AtendeMente",
        ])
        .output()
        .map_err(|e| format!("Falha ao executar netsh: {}", e))?;

    if output.status.success() {
        tracing::info!("[Mobile] Regra de firewall removida");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("[Mobile] Falha ao remover regra de firewall: {}", stderr);
        Err(format!("Falha ao remover regra de firewall: {}", stderr))
    }
}
