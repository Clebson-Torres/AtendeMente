use serde::{Deserialize, Serialize};

// ─── Users ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub two_factor_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Patients ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Patient {
    pub id: String,
    pub user_id: String,
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub status: String,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub emergency_phone: Option<String>,
    pub admin_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientListItem {
    pub id: String,
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePatientInput {
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub emergency_phone: Option<String>,
    pub admin_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePatientInput {
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub emergency_phone: Option<String>,
    pub admin_notes: Option<String>,
}

// ─── Appointments ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Appointment {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub series_id: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    pub status: String,
    pub confirmation_status: String,
    pub session_price_cents: i64,
    pub quick_notes: Option<String>,
    pub cancel_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub patient_id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub status: String,
    pub confirmation_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppointmentDetail {
    pub id: String,
    pub patient_id: String,
    pub patient_name: String,
    pub starts_at: String,
    pub ends_at: String,
    pub series_id: Option<String>,
    pub status: String,
    pub confirmation_status: String,
    pub session_price_cents: i64,
    pub quick_notes: Option<String>,
    pub cancel_reason: Option<String>,
    pub payment_id: Option<String>,
    pub payment_status: Option<String>,
    pub payment_method: Option<String>,
    pub amount_received_cents: Option<i64>,
    pub paid_at: Option<String>,
    pub payment_notes: Option<String>,
    pub record_id: Option<String>,
    pub encrypted_payload: Option<String>,
    pub iv: Option<String>,
    pub auth_tag: Option<String>,
    pub key_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAppointmentInput {
    pub patient_id: String,
    pub starts_at: String,
    pub ends_at: String,
    pub status: Option<String>,
    pub confirmation_status: Option<String>,
    pub session_price_cents: Option<i64>,
    pub quick_notes: Option<String>,
    pub cancel_reason: Option<String>,
    // Recurrence
    pub recurrence_frequency: Option<String>,
    pub recurrence_end_mode: Option<String>,
    pub recurrence_until_date: Option<String>,
    pub recurrence_occurrences: Option<i32>,
}

// ─── Recurring Series ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecurringSeries {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub frequency: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub occurrences_count: Option<i32>,
    pub start_time: String,
    pub end_time: String,
    pub cancelled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Payments ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id: String,
    pub user_id: String,
    pub appointment_id: String,
    pub status: String,
    pub method: String,
    pub paid_at: Option<String>,
    pub amount_received_cents: i64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentWithAppointment {
    pub payment_id: Option<String>,
    pub appointment_id: String,
    pub appointment_status: String,
    pub status: String,
    pub method: String,
    pub amount_received_cents: i64,
    pub paid_at: Option<String>,
    pub patient_name: String,
    pub starts_at: String,
    pub session_price_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertPaymentInput {
    pub appointment_id: String,
    pub status: String,
    pub method: String,
    pub paid_at: Option<String>,
    pub amount_received_cents: i64,
    pub notes: Option<String>,
}

// ─── Session Records ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub appointment_id: String,
    pub encrypted_payload: String,
    pub iv: String,
    pub auth_tag: String,
    pub key_version: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRecordInput {
    pub appointment_id: String,
    pub patient_id: String,
    pub content: String,
}

// ─── Record Files ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordFile {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub appointment_id: String,
    pub payment_id: Option<String>,
    pub kind: String,
    pub storage_path: String,
    pub original_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub uploaded_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadRequest {
    pub appointment_id: String,
    pub patient_id: String,
    pub payment_id: Option<String>,
    pub kind: String,
    pub file_name: String,
    pub file_size: i64,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSession {
    pub file_id: String,
    pub storage_path: String,
}

// ─── Audit Logs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: String,
    pub created_at: String,
}

// ─── Dashboard ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub upcoming_appointments: Vec<CalendarEvent>,
    pub todays_appointments: Vec<CalendarEvent>,
    pub financial_summary: FinancialSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialSummary {
    pub paid_cents: i64,
    pub pending_cents: i64,
    pub appointments_count: i64,
}

// ─── Export ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub exported_at: String,
    pub patient: serde_json::Value,
    pub appointments: Vec<serde_json::Value>,
}
