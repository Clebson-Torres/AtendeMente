use sqlx::SqlitePool;
use uuid::Uuid;

use crate::errors::AppError;

pub async fn write_audit_log(
    db: &SqlitePool,
    user_id: &str,
    action: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    metadata: Option<&serde_json::Value>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let metadata_str = metadata
        .map(|m| m.to_string())
        .unwrap_or_else(|| "{}".to_string());

    sqlx::query(
        r#"INSERT INTO audit_logs (id, user_id, action, entity_type, entity_id, ip_address, user_agent, metadata)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(ip_address)
    .bind(user_agent)
    .bind(&metadata_str)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to write audit log: {}", e)))?;

    Ok(())
}
