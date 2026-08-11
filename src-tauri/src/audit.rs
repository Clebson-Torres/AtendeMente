use sqlx::SqlitePool;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Copy)]
pub enum AuditAction {
    LoginSucceeded,
    LoginFailed,
    Logout,
    Locked,
    Unlocked,
    PatientCreated,
    PatientUpdated,
    PatientDeleted,
    PatientViewed,
    AppointmentCreated,
    AppointmentUpdated,
    AppointmentDeleted,
    FileUploadApproved,
    FileUploadRejected,
    FileDownloaded,
    FileDeleted,
    ExportCreated,
    ImportCreated,
    BackupCreated,
    BackupRestored,
    RecordUpdated,
    PaymentUpdated,
    PasswordReset,
    /// Fallback for an update on an entity without a dedicated action.
    EntityUpdated,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoginSucceeded => "auth.login.succeeded",
            Self::LoginFailed => "auth.login.failed",
            Self::Logout => "auth.logout",
            Self::Locked => "auth.locked",
            Self::Unlocked => "auth.unlocked",
            Self::PatientCreated => "patient.created",
            Self::PatientUpdated => "patient.updated",
            Self::PatientDeleted => "patient.deleted",
            Self::PatientViewed => "patient.viewed",
            Self::AppointmentCreated => "appointment.created",
            Self::AppointmentUpdated => "appointment.updated",
            Self::AppointmentDeleted => "appointment.deleted",
            Self::FileUploadApproved => "file.upload.approved",
            Self::FileUploadRejected => "file.upload.rejected",
            Self::FileDownloaded => "file.downloaded",
            Self::FileDeleted => "file.deleted",
            Self::ExportCreated => "system.export",
            Self::ImportCreated => "system.import",
            Self::BackupCreated => "system.backup.created",
            Self::BackupRestored => "system.backup.restored",
            Self::RecordUpdated => "record.updated",
            Self::PaymentUpdated => "payment.updated",
            Self::PasswordReset => "auth.password.reset",
            Self::EntityUpdated => "entity.updated",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub user_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub details: String,
    pub ip_or_device: Option<String>,
}

pub async fn write_audit_event(
    db: &SqlitePool,
    user_id: &str,
    action: AuditAction,
    entity_type: &str,
    entity_id: Option<&str>,
    details: serde_json::Value,
    ip_or_device: Option<&str>,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let details_str = details.to_string();

    sqlx::query(
        r#"INSERT INTO audit_logs
        (id, timestamp, user_id, action, entity_type, entity_id, details, ip_or_device)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(&timestamp)
    .bind(user_id)
    .bind(action.as_str())
    .bind(entity_type)
    .bind(entity_id)
    .bind(&details_str)
    .bind(ip_or_device)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to write audit event: {}", e)))?;

    Ok(())
}

pub async fn list_audit_events(
    db: &SqlitePool,
    user_id: &str,
    limit: i64,
) -> Result<Vec<AuditEvent>, AppError> {
    let limit = limit.clamp(1, 500);
    sqlx::query_as::<_, AuditEvent>(
        r#"SELECT id, timestamp, user_id, action, entity_type, entity_id, details, ip_or_device
        FROM audit_logs
        WHERE user_id = ?
        ORDER BY timestamp DESC
        LIMIT ?"#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to list audit events: {}", e)))
}

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
    let metadata_str = metadata
        .map(|m| m.to_string())
        .unwrap_or_else(|| "{}".to_string());
    let device = ip_address.or(user_agent);
    // Every caller's action string must map explicitly. The previous catch-all
    // recorded `file_delete` and session-record edits as `patient.updated`,
    // which made the LGPD trail actively misleading.
    let mapped = match action {
        "login" => AuditAction::LoginSucceeded,
        "logout" => AuditAction::Logout,
        "file_upload" => AuditAction::FileUploadApproved,
        "file_download" => AuditAction::FileDownloaded,
        "file_delete" => AuditAction::FileDeleted,
        "patient_export" => AuditAction::ExportCreated,
        "patient_import" => AuditAction::ImportCreated,
        "delete" => AuditAction::PatientDeleted,
        "update" => match entity_type {
            "patient" => AuditAction::PatientUpdated,
            "appointment" | "recurring_series" => AuditAction::AppointmentUpdated,
            "session_record" => AuditAction::RecordUpdated,
            "payment" => AuditAction::PaymentUpdated,
            _ => AuditAction::EntityUpdated,
        },
        other => {
            tracing::warn!("[Audit] Acao nao mapeada '{}' em '{}'", other, entity_type);
            AuditAction::EntityUpdated
        }
    };
    let details = serde_json::from_str(&metadata_str).unwrap_or_else(|_| serde_json::json!({}));
    write_audit_event(db, user_id, mapped, entity_type, entity_id, details, device).await
}
