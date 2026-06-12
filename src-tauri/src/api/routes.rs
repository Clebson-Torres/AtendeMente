use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::db::models::{
    CreateAppointmentInput, CreatePatientInput, FileUploadRequest, SaveRecordInput,
    UpdatePatientInput, UpsertPaymentInput,
};
use crate::errors::{ActionResponse, AppError};
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
}

#[derive(Deserialize)]
pub struct CancelInput {
    pub cancel_reason: Option<String>,
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
        .route("/files/confirm", post(confirm_file_upload))
        .route("/files/{id}/download", get(download_file))
        .route("/exports/patient/{id}", post(export_patient))
        .route("/dashboard", get(dashboard))
        .with_state(state)
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
) -> Result<Json<ActionResponse<Vec<crate::db::models::PatientListItem>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let patients = features::patients::list_patients(&db, &user.id, query.search.as_deref().unwrap_or("")).await?;
    Ok(Json(ActionResponse::success("", patients)))
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
    Json(input): Json<CreateAppointmentInput>,
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
) -> Result<Json<ActionResponse<Vec<crate::db::models::PaymentWithAppointment>>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let payments = features::payments::list_payments(&db, &user.id).await?;
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
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let (paid_cents, pending_cents) = features::payments::get_financial_summary(&db, &user.id).await?;
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

// ─── Dashboard ──────────────────────────────────────────────────────────────

async fn dashboard(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user = get_authenticated_user(&headers, &state).await?;
    let db = state.get_or_open_user_db(&user.id).await?;
    let (count, todays, upcoming) = features::dashboard::get_dashboard_data(&db, &user.id).await?;
    Ok(Json(ActionResponse::success("", serde_json::json!({
        "appointments_count": count,
        "todays_appointments": todays,
        "upcoming_appointments": upcoming,
    }))))
}
