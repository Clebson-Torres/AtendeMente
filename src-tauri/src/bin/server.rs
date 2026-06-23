use std::collections::HashMap;
use std::sync::Arc;

use atendemente_lib::config::AppConfig;
use atendemente_lib::{run_server, AppState};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut config = AppConfig::from_env();

    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--port") {
        if let Some(port_str) = args.get(pos + 1) {
            if let Ok(port) = port_str.parse() {
                config.server_port = port;
            }
        }
    }

    atendemente_lib::crypto::set_pepper(&config.master_pepper);

    let auth_db = atendemente_lib::db::init_auth_database(&config.auth_database_url)
        .await
        .expect("Failed to initialize auth database");

    let state = Arc::new(AppState {
        config: config.clone(),
        auth_db,
        user_dbs: RwLock::new(HashMap::new()),
    });

    if let Err(e) = run_server(state, None).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
