pub mod api;
pub mod audit;
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

use axum::{extract::DefaultBodyLimit, middleware as axum_middleware, Router};
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

pub async fn run_server(state: Arc<AppState>, _app: Option<AppHandle>) {
    let auth_router = crate::auth::create_auth_router(state.clone());
    let api_routes = api::routes::create_router(state.clone());

    let app = Router::new()
        .nest("/api", auth_router)
        .nest("/api", api_routes)
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
                ])
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(false),
        );

    let addr = format!("127.0.0.1:{}", state.config.server_port);
    tracing::info!("Starting API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
