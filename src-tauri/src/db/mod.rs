use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub mod models;

pub async fn init_database(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Ensure parent directory exists
    if let Some(path) = database_url.strip_prefix("sqlite:") {
        let path = path.split('?').next().unwrap_or(path);
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Enable WAL mode and foreign keys
    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await?;

    // Run migrations
    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await?;

    tracing::info!("Database initialized successfully");
    Ok(pool)
}
