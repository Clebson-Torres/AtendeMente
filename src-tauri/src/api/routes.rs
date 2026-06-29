use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::models::{
    CreateAppointmentInput, CreatePatientInput, FileUploadRequest, SaveRecordInput,
    UpdateAppointmentInput, UpdatePatientInput, UpsertPaymentInput,
};
use crate::errors::{ActionResponse, AppError, PaginatedData};
use crate::features;
use crate::AppState;

// ─── Query params ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CalendarQuery {
    pub start: String,
    pub end: String,
}

#[derive(Deserialize)]
pub struct PatientSearchQuery {
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct CancelInput {
    pub cancel_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct MonthYearQuery {
    pub month: Option<u32>,
    pub year: Option<i32>,
}

// ─── Auth Helper ────────────────────────────────────────────────────────────

async fn get_authenticated_user(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<crate::features::auth::AuthenticatedUser, AppError> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Token nao informado."))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::unauthorized("Formato invalido. Use: Bearer <token>."))?;

    let (user_id, email, full_name) =
        crate::auth::auth_service::validate_session(&state.auth_db, token)
            .await
            .map_err(|e| AppError::unauthorized(e))?;

    Ok(crate::features::auth::AuthenticatedUser {
        id: user_id,
        email,
        full_name: Some(full_name),
    })
}

// ─── Router ─────────────────────────────────────────────────────────────────

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/patients", get(list_patients).post(create_patient))
        .route("/patients/{id}", get(get_patient).put(update_patient))
        .route("/patients/{id}/activate", post(activate_patient))
        .route("/patients/{id}/deactivate", post(deactivate_patient))
        .route("/patients/{id}/appointments", get(list_patient_appointments))
        .route("/appointments", get(list_calendar).post(create_appointment))
        .route("/appointments/{id}", get(get_appointment).put(update_appointment))
        .route("/appointments/{id}/cancel", post(cancel_appointment))
        .route("/appointments/series/{id}/cancel", post(cancel_recurring_series))
        .route("/payments/upsert", post(upsert_payment))
        .route("/payments", get(list_payments))
        .route("/payments/pending", get(list_pending_payments))
        .route("/payments/summary", get(payment_summary))
        .route("/records/save", post(save_record))
        .route("/records/{appointment_id}", get(get_record))
        .route("/files/upload-session", post(create_upload_session))
        .route("/files/upload/{file_id}", put(upload_file_content))
        .route("/files/confirm", post(confirm_file_upload))
        .route("/files/appointment/{appointment_id}", get(list_files_by_appointment))
        .route("/files/{id}/download", get(download_file))
        .route("/files/{id}", delete(delete_file_handler))
        .route("/exports/patient/{id}", get(export_patient))
        .route("/exports/patients/csv", get(export_patients_csv))
        .route("/exports/appointments/csv", get(export_appointments_csv))
        .route("/exports/payments/csv", get(export_payments_csv))
        .route("/patients/import/preview", post(import_preview))
        .route("/patients/import/commit", post(import_commit))
        .route("/dashboard", get(dashboard))
        .route("/backup", post(create_backup_handler))
        .route("/backup/restore", post(restore_backup_handler))
        .route("/backup/config", get(get_backup_config_handler).put(set_backup_config_handler))
        .route("/audit/logs", get(list_audit_logs))
        .route("/network-info", get(network_info))
        .route("/settings/mobile-access", get(get_mobile_access).put(set_mobile_access))
        .with_state(state)
}

// ─── Mobile Access ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MobileAccessResponse {
    enabled: bool,
}

#[derive(Deserialize)]
struct MobileAccessInput {
    enabled: bool,
}

async fn get_mobile_access(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ActionResponse<MobileAccessResponse>>, AppError> {
    let _user = get_authenticated_user(&headers, &state).await?;
    let cfg = crate::config::load_config_file().unwrap_or_default();
    let enabled = cfg.mobile_access_enabled.unwrap_or(false);
    Ok(Json(ActionResponse::success("", MobileAccessResponse {
        enabled,
    })))
}

async fn set_mobile_access(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(input): Json<MobileAccessInput>,
) -> Result<Json<ActionResponse<MobileAccessResponse>>, AppError> {
    let _user = get_authenticated_user(&headers, &state).await?;
    crate::config::set_mobile_access_enabled(input.enabled);
    if input.enabled {
        if let Err(e) = crate::add_firewall_rule() {
            tracing::warn!("[Mobile] Firewall: {}", e);
        }
    } else {
        if let Err(e) = crate::remove_firewall_rule() {
            tracing::warn!("[Mobile] Firewall: {}", e);
        }
    }
    Ok(Json(ActionResponse::success("", MobileAccessResponse {
        enabled: input.enabled,
    })))
}

// ─── Network Info ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct NetworkInterface {
    name: String,
    ip: String,
    is_recommended: bool,
    is_vpn: bool,
    interface_type: String,
}

#[derive(Serialize)]
struct NetworkInfo {
    interfaces: Vec<NetworkInterface>,
    port: u16,
    has_vpn: bool,
    server_bound_to: String,
}

async fn network_info(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ActionResponse<NetworkInfo>>, AppError> {
    let _user = get_authenticated_user(&headers, &state).await?;

    let interfaces_raw = netdev::get_interfaces();
    let default_iface = netdev::get_default_interface().ok();

    let mut interfaces = Vec::new();
    let mut has_vpn = false;

    for iface in &interfaces_raw {
        if iface.is_loopback() || iface.is_tun() || !iface.is_up() {
            tracing::debug!("[Mobile] Interface ignorada: {} (loopback/tun/down)", iface.name);
            continue;
        }

        if is_virtual_interface(iface) {
            tracing::debug!("[Mobile] Interface ignorada: {} (virtual)", iface.name);
            continue;
        }

        let ipv4_addrs = iface.ipv4_addrs();
        if ipv4_addrs.is_empty() {
            continue;
        }

        let ip = ipv4_addrs[0];
        if !is_private_ip(&ip) {
            tracing::debug!("[Mobile] Interface ignorada: {} - {} (nao e IP privado)", iface.name, ip);
            continue;
        }

        let name = iface.friendly_name.clone()
            .unwrap_or_else(|| iface.name.clone());
        let is_default = default_iface.as_ref()
            .map(|d| d.name == iface.name)
            .unwrap_or(false);
        let iface_type = detect_interface_type(iface);
        let is_vpn = iface_type == "vpn";

        if is_vpn { has_vpn = true; }

        tracing::info!(
            "[Mobile] Interface encontrada: {} - {} (tipo={}, default={})",
            name, ip, iface_type, is_default
        );

        interfaces.push(NetworkInterface {
            name,
            ip: ip.to_string(),
            is_recommended: is_default,
            is_vpn,
            interface_type: iface_type,
        });
    }

    interfaces.sort_by(|a, b| b.is_recommended.cmp(&a.is_recommended));

    if let Some(ref best) = interfaces.first() {
        tracing::info!("[Mobile] Interface recomendada: {} - {}", best.name, best.ip);
    }

    Ok(Json(ActionResponse::success("", NetworkInfo {
        interfaces,
        port: state.config.server_port,
        has_vpn,
        server_bound_to: if state.config.mobile_access_enabled { "0.0.0.0".into() } else { "127.0.0.1".into() },
    })))
}

fn is_virtual_interface(iface: &netdev::Interface) -> bool {
    let name_lower = iface.name.to_lowercase();
    let friendly_lower = iface.friendly_name
        .as_deref()
        .unwrap_or("")
        .to_lowercase();

    let virtual_keywords = [
        "hyper-v", "hyper_v", "virtual", "vmware", "virtualbox", "vbox",
        "docker", "wsl", "tailscale", "zerotier", "wireguard", "openvpn",
        "tun", "tap", "pseudo", "isatap", "teredo", "6to4",
    ];

    for kw in &virtual_keywords {
        if name_lower.contains(kw) || friendly_lower.contains(kw) {
            return true;
        }
    }

    !iface.is_physical()
}

fn detect_interface_type(iface: &netdev::Interface) -> String {
    let name_lower = iface.name.to_lowercase();
    let friendly_lower = iface.friendly_name
        .as_deref()
        .unwrap_or("")
        .to_lowercase();

    if name_lower.contains("wi-fi") || name_lower.contains("wifi")
        || name_lower.contains("wlan") || name_lower.contains("en0")
        || friendly_lower.contains("wi-fi") || friendly_lower.contains("wifi")
    {
        return "wifi".into();
    }

    if name_lower.contains("ethernet") || name_lower.contains("eth")
        || name_lower.contains("enp") || name_lower.contains("eno")
        || friendly_lower.contains("ethernet")
    {
        return "ethernet".into();
    }

    if name_lower.contains("vpn") || name_lower.contains("wireguard")
        || name_lower.contains("openvpn") || name_lower.contains("tailscale")
        || name_lower.contains("zerotier")
    {
        return "vpn".into();
    }

    "other".into()
}

fn is_private_ip(ip: &std::net::Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

// ─── Backup ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RestoreInput {
    backup_base64: String,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
struct CreateBackupInput {
    #[serde(default)]
    password: Option<String>,
}

// ─── Backup Config ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct BackupConfigResponse {
    frequency: String,
    last_backup_at: Option<String>,
}

#[derive(Deserialize)]
struct BackupConfigInput {
    frequency: String,
}

async fn get_backup_config_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<BackupConfigResponse>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let config = features::backup::get_backup_config(&db, &user.id).await?;
    Ok(Json(ActionResponse::success("", BackupConfigResponse {
        frequency: config.frequency,
        last_backup_at: config.last_backup_at,
    })))
}

async fn set_backup_config_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<BackupConfigInput>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    features::backup::set_backup_config(&db, &user.id, &input.frequency).await?;
    Ok(Json(ActionResponse::<()>::success_empty("Configuracao de backup atualizada.")))
}

async fn create_backup_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<CreateBackupInput>,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, String); 2], Vec<u8>), AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let bundle = if let Some(pass) = &input.password {
        features::backup::create_backup_with_password(&db, &state.config, &user.id, Some(pass)).await?
    } else {
        features::backup::create_backup(&db, &state.config, &user.id).await?
    };
    features::backup::touch_backup_timestamp(&db, &user.id).await?;
    let content_type = if bundle.encrypted { "application/octet-stream" } else { "application/zip" };
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::HeaderName::from_static("content-type"), content_type.into()),
            (axum::http::HeaderName::from_static("content-disposition"), format!("attachment; filename=\"{}\"", bundle.file_name)),
        ],
        bundle.bytes,
    ))
}

async fn restore_backup_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<RestoreInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let data = base64_decode(&input.backup_base64)?;
    let manifest = if let Some(pass) = &input.password {
        features::backup::restore_backup_with_password(&db, &state.config, &user.id, &data, Some(pass)).await?
    } else {
        features::backup::restore_backup(&db, &state.config, &user.id, &data).await?
    };
    Ok(Json(ActionResponse::success("Backup restaurado com sucesso.", serde_json::json!({
        "version": manifest.version,
        "entries": manifest.file_hashes.len(),
    }))))
}

// ─── Audit ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<i64>,
}

async fn list_audit_logs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<ActionResponse<Vec<crate::audit::AuditEvent>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let events = crate::audit::list_audit_events(&db, &user.id, query.limit.unwrap_or(100)).await?;
    Ok(Json(ActionResponse::success("", events)))
}

// ─── Health Check ───────────────────────────────────────────────────────────

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ─── Patients ───────────────────────────────────────────────────────────────

async fn list_patients(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<PatientSearchQuery>,
) -> Result<Json<ActionResponse<PaginatedData<crate::db::models::PatientListItem>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let result = features::patients::list_patients(&db, &user.id, query.search.as_deref().unwrap_or(""), page, per_page, query.status.as_deref()).await?;
    Ok(Json(ActionResponse::success("", result)))
}

async fn create_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<CreatePatientInput>,
) -> Result<Json<ActionResponse<crate::db::models::Patient>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let patient = features::patients::create_patient(&db, &user.id, &input).await?;
    Ok(Json(ActionResponse::success("Paciente cadastrado com sucesso.", patient)))
}

async fn get_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<crate::db::models::Patient>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let patient = features::patients::get_patient_detail(&db, &user.id, &id).await?;
    Ok(Json(ActionResponse::success("", patient)))
}

async fn update_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdatePatientInput>,
) -> Result<Json<ActionResponse<crate::db::models::Patient>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let patient = features::patients::update_patient(&db, &user.id, &id, &input).await?;
    Ok(Json(ActionResponse::success("Paciente atualizado com sucesso.", patient)))
}

async fn activate_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<crate::db::models::Patient>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let patient = features::patients::set_patient_status(&db, &user.id, &id, true).await?;
    Ok(Json(ActionResponse::success("Paciente reativado com sucesso.", patient)))
}

async fn deactivate_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<crate::db::models::Patient>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let patient = features::patients::set_patient_status(&db, &user.id, &id, false).await?;
    Ok(Json(ActionResponse::success("Paciente desativado com sucesso.", patient)))
}

async fn list_patient_appointments(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<Vec<crate::db::models::CalendarEvent>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let events = features::appointments::list_patient_appointments(&db, &user.id, &id).await?;
    Ok(Json(ActionResponse::success("", events)))
}

// ─── Appointments ───────────────────────────────────────────────────────────

async fn list_calendar(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<CalendarQuery>,
) -> Result<Json<ActionResponse<Vec<crate::db::models::CalendarEvent>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let events = features::appointments::list_calendar_events(&db, &user.id, &query.start, &query.end).await?;
    Ok(Json(ActionResponse::success("", events)))
}

async fn create_appointment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<CreateAppointmentInput>,
) -> Result<Json<ActionResponse<crate::db::models::AppointmentDetail>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let appt = features::appointments::create_appointment(&db, &user.id, &input).await?;
    Ok(Json(ActionResponse::success("Atendimento criado com sucesso.", appt)))
}

async fn get_appointment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<crate::db::models::AppointmentDetail>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let appt = features::appointments::get_appointment_detail(&db, &user.id, &id).await?;
    Ok(Json(ActionResponse::success("", appt)))
}

async fn update_appointment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<UpdateAppointmentInput>,
) -> Result<Json<ActionResponse<crate::db::models::AppointmentDetail>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let appt = features::appointments::update_appointment(&db, &user.id, &id, &input).await?;
    Ok(Json(ActionResponse::success("Atendimento atualizado com sucesso.", appt)))
}

async fn cancel_appointment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<CancelInput>,
) -> Result<Json<ActionResponse<crate::db::models::AppointmentDetail>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let reason = input.cancel_reason.unwrap_or_default();
    let appt = features::appointments::cancel_appointment(&db, &user.id, &id, &reason).await?;
    Ok(Json(ActionResponse::success("Atendimento cancelado.", appt)))
}

async fn cancel_recurring_series(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    features::appointments::cancel_recurring_series(&db, &user.id, &id).await?;
    Ok(Json(ActionResponse::<()>::success_empty("Serie recorrente encerrada.")))
}

// ─── Payments ───────────────────────────────────────────────────────────────

async fn upsert_payment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<UpsertPaymentInput>,
) -> Result<Json<ActionResponse<crate::db::models::Payment>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let payment = features::payments::upsert_payment(&db, &user.id, &input).await?;
    Ok(Json(ActionResponse::success("Pagamento salvo com sucesso.", payment)))
}

async fn list_payments(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<MonthYearQuery>,
) -> Result<Json<ActionResponse<Vec<crate::db::models::PaymentWithAppointment>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let payments = features::payments::list_payments(&db, &user.id, query.month, query.year).await?;
    Ok(Json(ActionResponse::success("", payments)))
}

async fn list_pending_payments(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<Vec<crate::db::models::PaymentWithAppointment>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let payments = features::payments::list_pending_payments(&db, &user.id).await?;
    Ok(Json(ActionResponse::success("", payments)))
}

async fn payment_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<MonthYearQuery>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let (paid_cents, pending_cents) = features::payments::get_financial_summary(&db, &user.id, query.month, query.year).await?;
    Ok(Json(ActionResponse::success("", serde_json::json!({
        "paid_cents": paid_cents,
        "pending_cents": pending_cents,
    }))))
}

// ─── Records ────────────────────────────────────────────────────────────────

async fn save_record(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<SaveRecordInput>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    features::records::save_session_record(&db, &user.id, &input).await?;
    Ok(Json(ActionResponse::<()>::success_empty("Registro salvo com seguranca.")))
}

async fn get_record(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(appointment_id): Path<String>,
) -> Result<Json<ActionResponse<String>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let content = features::records::get_session_record(&db, &user.id, &appointment_id).await?;
    Ok(Json(ActionResponse::success("", content)))
}

// ─── Files ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ConfirmUploadInput {
    file_id: String,
}

async fn create_upload_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<FileUploadRequest>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    crate::rate_limit::enforce_rate_limit(&db, "upload", &user.id, 20, 3600_000).await?;
    let (file_id, storage_path) = features::files::create_upload_session(&db, &state.config, &user.id, &input).await?;
    Ok(Json(ActionResponse::success("", serde_json::json!({
        "file_id": file_id,
        "storage_path": storage_path,
    }))))
}

async fn upload_file_content(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(file_id): Path<String>,
    body: Bytes,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    features::files::write_upload_content(&db, &user.id, &file_id, &body).await?;
    Ok(Json(ActionResponse::<()>::success_empty("Conteúdo do arquivo salvo.")))
}

async fn list_files_by_appointment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(appointment_id): Path<String>,
) -> Result<Json<ActionResponse<Vec<crate::db::models::RecordFile>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let files = features::files::list_files_by_appointment(&db, &user.id, &appointment_id).await?;
    Ok(Json(ActionResponse::success("", files)))
}

async fn confirm_file_upload(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<ConfirmUploadInput>,
) -> Result<Json<ActionResponse<crate::db::models::RecordFile>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let file = features::files::confirm_upload(&db, &state.config, &user.id, &input.file_id).await?;
    Ok(Json(ActionResponse::success("Upload confirmado com sucesso.", file)))
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, String); 3], Vec<u8>), AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let (file, data) = features::files::download_file(&db, &user.id, &id).await?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::HeaderName::from_static("content-type"), file.mime_type),
            (axum::http::HeaderName::from_static("content-disposition"), format!("attachment; filename=\"{}\"", file.original_name)),
            (axum::http::HeaderName::from_static("content-length"), data.len().to_string()),
        ],
        data,
    ))
}

async fn delete_file_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    features::files::delete_file(&db, &user.id, &id).await?;
    Ok(Json(ActionResponse::<()>::success_empty("Arquivo excluído.")))
}

// ─── Exports ────────────────────────────────────────────────────────────────

async fn export_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, String); 2], Vec<u8>), AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    crate::rate_limit::enforce_rate_limit(&db, "export", &user.id, 10, 3600_000).await?;
    let bundle = features::exports::export_patient_bundle(&db, &user.id, &id).await?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::HeaderName::from_static("content-type"), "application/zip".into()),
            (axum::http::HeaderName::from_static("content-disposition"), format!("attachment; filename=\"paciente-{}.zip\"", id)),
        ],
        bundle.buffer,
    ))
}

async fn export_patients_csv(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, String); 2], Vec<u8>), AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    crate::rate_limit::enforce_rate_limit(&db, "export", &user.id, 10, 3600_000).await?;
    let csv = features::exports::export_patients_csv(&db, &user.id).await?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::HeaderName::from_static("content-type"), "text/csv; charset=utf-8".into()),
            (axum::http::HeaderName::from_static("content-disposition"), "attachment; filename=\"pacientes.csv\"".into()),
        ],
        csv.into_bytes(),
    ))
}

async fn export_appointments_csv(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<MonthYearQuery>,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, String); 2], Vec<u8>), AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    crate::rate_limit::enforce_rate_limit(&db, "export", &user.id, 10, 3600_000).await?;
    let csv = features::appointments::export_appointments_csv(&db, &user.id, query.month, query.year).await?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::HeaderName::from_static("content-type"), "text/csv; charset=utf-8".into()),
            (axum::http::HeaderName::from_static("content-disposition"), "attachment; filename=\"agenda.csv\"".into()),
        ],
        csv.into_bytes(),
    ))
}

async fn export_payments_csv(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<MonthYearQuery>,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, String); 2], Vec<u8>), AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    crate::rate_limit::enforce_rate_limit(&db, "export", &user.id, 10, 3600_000).await?;
    let csv = features::payments::export_payments_csv(&db, &user.id, query.month, query.year).await?;
    Ok((
        axum::http::StatusCode::OK,
        [
            (axum::http::HeaderName::from_static("content-type"), "text/csv; charset=utf-8".into()),
            (axum::http::HeaderName::from_static("content-disposition"), "attachment; filename=\"financeiro.csv\"".into()),
        ],
        csv.into_bytes(),
    ))
}

// ─── Import ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ImportPayload {
    pub content_base64: String,
}

#[derive(Serialize)]
pub struct ImportState {
    pub session_id: String,
    pub preview: crate::features::patients_import::ImportPreview,
}

async fn import_preview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ImportPayload>,
) -> Result<Json<ActionResponse<ImportState>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;

    crate::rate_limit::enforce_rate_limit(&db, "import", &user.id, 5, 3600_000).await?;

    let data = base64_decode(&payload.content_base64)?;
    let preview = crate::features::patients_import::parse_csv_bytes(&data)?;

    let session_id = Uuid::new_v4().to_string();

    Ok(Json(ActionResponse::success("", ImportState {
        session_id,
        preview,
    })))
}

#[derive(Deserialize)]
pub struct ImportCommitPayload {
    pub session_id: String,
    pub rows: Vec<crate::features::patients_import::CsvRow>,
}

async fn import_commit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ImportCommitPayload>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;

    let imported = crate::features::patients_import::commit_import(
        &db, &user.id, &payload.rows,
    ).await?;

    Ok(Json(ActionResponse::success("", serde_json::json!({
        "imported": imported,
    }))))
}

fn base64_decode(input: &str) -> Result<Vec<u8>, AppError> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    engine.decode(input).map_err(|e| AppError::bad_request(format!("Erro ao decodificar base64: {}", e)))
}

// ─── Dashboard ──────────────────────────────────────────────────────────────

async fn dashboard(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let (count, todays, upcoming, monthly_appointments, monthly_financial) = features::dashboard::get_dashboard_data(&db, &user.id).await?;
    Ok(Json(ActionResponse::success("", serde_json::json!({
        "appointments_count": count,
        "todays_appointments": todays,
        "upcoming_appointments": upcoming,
        "monthly_appointments": monthly_appointments,
        "monthly_financial": monthly_financial,
    }))))
}
