pub mod api;
pub mod audit;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod errors;
pub mod features;
pub mod firebase;
pub mod middleware;
pub mod rate_limit;
pub mod utils;

use std::sync::Arc;

use axum::{Router, middleware as axum_middleware};
use sqlx::SqlitePool;
use tauri::AppHandle;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub struct AppState {
    pub config: config::AppConfig,
    pub db: SqlitePool,
}

pub async fn run_server(state: Arc<AppState>, _app: AppHandle) {
    let app = build_router(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", state.config.server_port);
    tracing::info!("Starting API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = api::routes::create_router(state);

    Router::new()
        .nest("/api", api_routes)
        .route_layer(axum_middleware::from_fn(crate::middleware::security_headers))
}
