use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::db::models::{
    CreatePatientInput, Patient, PatientListItem, UpdatePatientInput,
};
use crate::errors::AppError;
use crate::utils;

pub async fn list_patients(
    db: &SqlitePool,
    user_id: &str,
    search: &str,
) -> Result<Vec<PatientListItem>, AppError> {
    let patients = if search.trim().is_empty() {
        sqlx::query_as::<_, Patient>(
            r#"SELECT * FROM patients
            WHERE user_id = ? AND deleted_at IS NULL
            ORDER BY full_name"#,
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to list patients: {}", e)))?
    } else {
        let pattern = format!("%{}%", search.trim());
        let phone_search: String = search.chars().filter(|c| c.is_ascii_digit()).collect();

        sqlx::query_as::<_, Patient>(
            r#"SELECT * FROM patients
            WHERE user_id = ? AND deleted_at IS NULL
            AND (full_name LIKE ? OR email LIKE ?
                 OR (phone LIKE ?))
            ORDER BY full_name"#,
        )
        .bind(user_id)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&phone_search)
        .fetch_all(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to search patients: {}", e)))?
    };

    Ok(patients
        .into_iter()
        .map(|p| PatientListItem {
            id: p.id,
            full_name: p.full_name,
            chart_number: p.chart_number,
            phone: p.phone,
            email: p.email,
            birth_date: p.birth_date,
            status: p.status,
            created_at: p.created_at,
        })
        .collect())
}

pub async fn get_patient_detail(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
) -> Result<Patient, AppError> {
    sqlx::query_as::<_, Patient>(
        r#"SELECT * FROM patients WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(patient_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get patient: {}", e)))?
    .ok_or_else(|| AppError::not_found("Paciente nao encontrado."))
}

async fn find_duplicate_patient(
    db: &SqlitePool,
    user_id: &str,
    full_name: &str,
    phone: Option<&str>,
    patient_id: Option<&str>,
) -> Result<Option<Patient>, AppError> {
    let input_key = utils::build_patient_identity_key(full_name, phone);

    let all_patients = sqlx::query_as::<_, Patient>(
        r#"SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|_| AppError::internal("Failed to check duplicates."))?;

    for p in &all_patients {
        if let Some(active_patient_id) = patient_id {
            if p.id == active_patient_id {
                continue;
            }
        }
        let existing_key = utils::build_patient_identity_key(&p.full_name, p.phone.as_deref());
        if existing_key == input_key {
            return Ok(Some(p.clone()));
        }
    }

    Ok(None)
}

async fn find_duplicate_chart_number(
    db: &SqlitePool,
    user_id: &str,
    chart_number: Option<&str>,
    patient_id: Option<&str>,
) -> Result<Option<Patient>, AppError> {
    let normalized = chart_number.map(|c| c.trim()).unwrap_or("");

    if normalized.is_empty() {
        return Ok(None);
    }

    let mut query = r#"SELECT * FROM patients WHERE user_id = ? AND chart_number = ? AND deleted_at IS NULL"#.to_string();
    if patient_id.is_some() {
        query.push_str(" AND id != ?");
    }
    query.push_str(" LIMIT 1");

    let mut q = sqlx::query_as::<_, Patient>(&query)
        .bind(user_id)
        .bind(normalized);
    if let Some(pid) = patient_id {
        q = q.bind(pid);
    }

    let result = q.fetch_optional(db).await
        .map_err(|_| AppError::internal("Failed to check chart number."))?;

    Ok(result)
}

pub async fn create_patient(
    db: &SqlitePool,
    user_id: &str,
    input: &CreatePatientInput,
) -> Result<Patient, AppError> {
    // Validate
    if input.full_name.trim().len() < 3 {
        return Err(AppError::bad_request("Informe o nome completo (min. 3 caracteres)."));
    }

    // Check duplicate
    if (find_duplicate_patient(
        db,
        user_id,
        &input.full_name,
        input.phone.as_deref(),
        None,
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe um paciente com o mesmo nome e telefone na sua base.",
        ));
    }

    // Check chart number duplicate
    if (find_duplicate_chart_number(
        db,
        user_id,
        input.chart_number.as_deref(),
        None,
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe um paciente com este numero do prontuario na sua base.",
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    sqlx::query(
        r#"INSERT INTO patients (id, user_id, full_name, chart_number, phone, email, birth_date,
            health_history, medications_in_use, emergency_phone, admin_notes, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.full_name)
    .bind(&input.chart_number)
    .bind(&input.phone)
    .bind(&input.email)
    .bind(&input.birth_date)
    .bind(&input.health_history)
    .bind(&input.medications_in_use)
    .bind(&input.emergency_phone)
    .bind(&input.admin_notes)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create patient: {}", e)))?;

    audit::write_audit_log(
        db, user_id, "update", "patient", Some(&id),
        Some(&serde_json::json!({"action": "create"})), None, None,
    ).await?;

    get_patient_detail(db, user_id, &id).await
}

pub async fn update_patient(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
    input: &UpdatePatientInput,
) -> Result<Patient, AppError> {
    // Verify exists
    let _existing = get_patient_detail(db, user_id, patient_id).await?;

    // Check duplicate
    if (find_duplicate_patient(
        db,
        user_id,
        &input.full_name,
        input.phone.as_deref(),
        Some(patient_id),
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe outro paciente com o mesmo nome e telefone na sua base.",
        ));
    }

    // Check chart number duplicate
    if (find_duplicate_chart_number(
        db,
        user_id,
        input.chart_number.as_deref(),
        Some(patient_id),
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe um paciente com este numero do prontuario na sua base.",
        ));
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    sqlx::query(
        r#"UPDATE patients SET
            full_name = ?, chart_number = ?, phone = ?, email = ?, birth_date = ?,
            health_history = ?, medications_in_use = ?, emergency_phone = ?, admin_notes = ?,
            updated_at = ?
        WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.full_name)
    .bind(&input.chart_number)
    .bind(&input.phone)
    .bind(&input.email)
    .bind(&input.birth_date)
    .bind(&input.health_history)
    .bind(&input.medications_in_use)
    .bind(&input.emergency_phone)
    .bind(&input.admin_notes)
    .bind(&now)
    .bind(patient_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to update patient: {}", e)))?;

    audit::write_audit_log(
        db, user_id, "update", "patient", Some(patient_id),
        None, None, None,
    ).await?;

    get_patient_detail(db, user_id, patient_id).await
}

pub async fn set_patient_status(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
    active: bool,
) -> Result<Patient, AppError> {
    let _existing = get_patient_detail(db, user_id, patient_id).await?;

    let status = if active { "active" } else { "inactive" };
    let action = if active { "reactivate" } else { "deactivate" };
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    sqlx::query(
        r#"UPDATE patients SET status = ?, updated_at = ? WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(status)
    .bind(&now)
    .bind(patient_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to update patient status: {}", e)))?;

    audit::write_audit_log(
        db, user_id, "update", "patient", Some(patient_id),
        Some(&serde_json::json!({"action": action})), None, None,
    ).await?;

    get_patient_detail(db, user_id, patient_id).await
}



#[derive(serde::Deserialize)]
pub struct ListPatientsQuery {
    pub search: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct PatientIdPath {
    pub id: String,
}
