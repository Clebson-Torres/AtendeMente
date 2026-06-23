use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::crypto;
use crate::db::models::{
    CreatePatientInput, Patient, PatientListItem, PatientPii, PatientRow, UpdatePatientInput,
};
use crate::errors::{AppError, PaginatedData};
use crate::utils;

// ─── PII helpers ─────────────────────────────────────────────────────────────────

fn input_to_pii(input: &CreatePatientInput) -> PatientPii {
    PatientPii {
        phone: input.phone.clone().filter(|s| !s.is_empty()),
        email: input.email.clone().filter(|s| !s.is_empty()),
        birth_date: input.birth_date.clone().filter(|s| !s.is_empty()),
        emergency_phone: input.emergency_phone.clone().filter(|s| !s.is_empty()),
        health_history: input.health_history.clone().filter(|s| !s.is_empty()),
        medications_in_use: input.medications_in_use.clone().filter(|s| !s.is_empty()),
        admin_notes: input.admin_notes.clone().filter(|s| !s.is_empty()),
    }
}

fn encrypt_pii(input: &CreatePatientInput, user_id: &str) -> Result<(String, String, String), AppError> {
    let pii = input_to_pii(input);
    let json = serde_json::to_string(&pii).map_err(|e| AppError::internal(format!("Erro ao serializar PII: {}", e)))?;
    let encrypted = crypto::encrypt_content(&json, user_id)?;
    Ok((encrypted.encrypted_payload, encrypted.iv, encrypted.auth_tag))
}

fn decrypt_pii(row: &PatientRow, user_id: &str) -> Result<PatientPii, AppError> {
    if let Some(ref encrypted) = row.pii_encrypted {
        let payload = crypto::EncryptedPayload {
            encrypted_payload: encrypted.clone(),
            iv: row.pii_iv.clone().unwrap_or_default(),
            auth_tag: row.pii_auth_tag.clone().unwrap_or_default(),
            key_version: 1,
        };
        let decrypted = crypto::decrypt_content(&payload, user_id)?;
        Ok(serde_json::from_str(&decrypted).unwrap_or(PatientPii {
            phone: None,
            email: None,
            birth_date: None,
            emergency_phone: None,
            health_history: None,
            medications_in_use: None,
            admin_notes: None,
        }))
    } else {
        Ok(PatientPii {
            phone: row.phone.clone(),
            email: row.email.clone(),
            birth_date: row.birth_date.clone(),
            emergency_phone: row.emergency_phone.clone(),
            health_history: row.health_history.clone(),
            medications_in_use: row.medications_in_use.clone(),
            admin_notes: row.admin_notes.clone(),
        })
    }
}

fn row_to_patient(row: PatientRow, user_id: &str) -> Result<Patient, AppError> {
    let pii = decrypt_pii(&row, user_id)?;
    Ok(Patient {
        id: row.id,
        user_id: row.user_id,
        full_name: row.full_name,
        chart_number: row.chart_number,
        phone: pii.phone,
        email: pii.email,
        birth_date: pii.birth_date,
        status: row.status,
        health_history: pii.health_history,
        medications_in_use: pii.medications_in_use,
        emergency_phone: pii.emergency_phone,
        admin_notes: pii.admin_notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    })
}

fn row_to_patient_list_item(row: PatientRow, user_id: &str) -> Result<PatientListItem, AppError> {
    let pii = decrypt_pii(&row, user_id)?;
    Ok(PatientListItem {
        id: row.id,
        full_name: row.full_name,
        chart_number: row.chart_number,
        phone: pii.phone,
        email: pii.email,
        birth_date: pii.birth_date,
        status: row.status,
        created_at: row.created_at,
    })
}

// ─── Search tokens ───────────────────────────────────────────────────────────────

fn generate_patient_tokens(patient_id: &str, full_name: &str, pii: &PatientPii) -> Vec<(String, String, String)> {
    let mut tokens = Vec::new();

    // Identity key for duplicate detection
    let identity_key = utils::build_patient_identity_key(full_name, pii.phone.as_deref());
    tokens.push((patient_id.to_string(), "identity_key".to_string(), identity_key));

    // Phone digits
    if let Some(phone) = &pii.phone {
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            tokens.push((patient_id.to_string(), "phone".to_string(), digits));
        }
    }

    // Email
    if let Some(email) = &pii.email {
        let lower = email.to_lowercase();
        tokens.push((patient_id.to_string(), "email".to_string(), lower.clone()));
        if let Some(local) = lower.split('@').next() {
            if !local.is_empty() {
                tokens.push((patient_id.to_string(), "email".to_string(), local.to_string()));
            }
        }
    }

    tokens
}

async fn set_patient_tokens(db: &SqlitePool, patient_id: &str, tokens: &[(String, String, String)]) -> Result<(), AppError> {
    sqlx::query("DELETE FROM patient_search_tokens WHERE patient_id = ?")
        .bind(patient_id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to clear search tokens: {}", e)))?;

    for (pid, token_type, token_text) in tokens {
        sqlx::query(
            r#"INSERT INTO patient_search_tokens (patient_id, token_type, token_text) VALUES (?, ?, ?)"#,
        )
        .bind(pid)
        .bind(token_type)
        .bind(token_text)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to insert search token: {}", e)))?;
    }

    Ok(())
}

// ─── Query helpers ───────────────────────────────────────────────────────────────

/// Build the WHERE clause for searching patients.
/// Uses plaintext `full_name LIKE` for name search and the token index for phone/email.
fn search_where_clause(search: &str) -> (String, String) {
    let phone_digits: String = search.chars().filter(|c| c.is_ascii_digit()).collect();

    // Token pattern: for phone-dominant queries, use phone digits; otherwise raw search
    let token_pattern = if phone_digits.len() >= 3 && phone_digits.len() >= search.len().saturating_sub(2) {
        format!("%{}%", phone_digits)
    } else {
        format!("%{}%", search)
    };

    let name_like = format!("%{}%", search);

    (name_like, token_pattern)
}

// ─── Public API ─────────────────────────────────────────────────────────────────

pub async fn list_patients(
    db: &SqlitePool,
    user_id: &str,
    search: &str,
    page: i64,
    per_page: i64,
    status_filter: Option<&str>,
) -> Result<PaginatedData<PatientListItem>, AppError> {
    let offset = (page - 1) * per_page;
    let has_status = status_filter.map(|s| !s.is_empty()).unwrap_or(false);

    let (rows, total) = if search.trim().is_empty() {
        let total: (i64,) = if has_status {
            sqlx::query_as(
                "SELECT COUNT(*) FROM patients WHERE user_id = ? AND deleted_at IS NULL AND status = ?",
            )
            .bind(user_id)
            .bind(status_filter.unwrap())
            .fetch_one(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to count patients: {}", e)))?
        } else {
            sqlx::query_as(
                "SELECT COUNT(*) FROM patients WHERE user_id = ? AND deleted_at IS NULL",
            )
            .bind(user_id)
            .fetch_one(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to count patients: {}", e)))?
        };

        let rows = if has_status {
            sqlx::query_as::<_, PatientRow>(
                "SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL AND status = ? ORDER BY full_name LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(status_filter.unwrap())
            .bind(per_page)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to list patients: {}", e)))?
        } else {
            sqlx::query_as::<_, PatientRow>(
                "SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL ORDER BY full_name LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to list patients: {}", e)))?
        };

        (rows, total.0)
    } else {
        let (name_pattern, token_pattern) = search_where_clause(search);

        let total: (i64,) = if has_status {
            sqlx::query_as(
                "SELECT COUNT(*) FROM patients WHERE user_id = ? AND deleted_at IS NULL AND status = ? AND (full_name LIKE ? OR id IN (SELECT patient_id FROM patient_search_tokens WHERE token_text LIKE ?))",
            )
            .bind(user_id)
            .bind(status_filter.unwrap())
            .bind(&name_pattern)
            .bind(&token_pattern)
            .fetch_one(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to count patients: {}", e)))?
        } else {
            sqlx::query_as(
                "SELECT COUNT(*) FROM patients WHERE user_id = ? AND deleted_at IS NULL AND (full_name LIKE ? OR id IN (SELECT patient_id FROM patient_search_tokens WHERE token_text LIKE ?))",
            )
            .bind(user_id)
            .bind(&name_pattern)
            .bind(&token_pattern)
            .fetch_one(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to count patients: {}", e)))?
        };

        let rows = if has_status {
            sqlx::query_as::<_, PatientRow>(
                "SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL AND status = ? AND (full_name LIKE ? OR id IN (SELECT patient_id FROM patient_search_tokens WHERE token_text LIKE ?)) ORDER BY full_name LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(status_filter.unwrap())
            .bind(&name_pattern)
            .bind(&token_pattern)
            .bind(per_page)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to search patients: {}", e)))?
        } else {
            sqlx::query_as::<_, PatientRow>(
                "SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL AND (full_name LIKE ? OR id IN (SELECT patient_id FROM patient_search_tokens WHERE token_text LIKE ?)) ORDER BY full_name LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(&name_pattern)
            .bind(&token_pattern)
            .bind(per_page)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to search patients: {}", e)))?
        };

        (rows, total.0)
    };

    let items = rows.into_iter()
        .map(|row| row_to_patient_list_item(row, user_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PaginatedData {
        items,
        total,
        page,
        per_page,
    })
}

pub async fn get_patient_detail(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
) -> Result<Patient, AppError> {
    let row = sqlx::query_as::<_, PatientRow>(
        r#"SELECT * FROM patients WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(patient_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get patient: {}", e)))?
    .ok_or_else(|| AppError::not_found("Paciente nao encontrado."))?;

    row_to_patient(row, user_id)
}

async fn find_duplicate_patient(
    db: &SqlitePool,
    user_id: &str,
    full_name: &str,
    phone: Option<&str>,
    exclude_patient_id: Option<&str>,
) -> Result<Option<Patient>, AppError> {
    let input_key = utils::build_patient_identity_key(full_name, phone);

    // Try identity_key token index first
    let matched_id: Option<String> = if let Some(exclude) = exclude_patient_id {
        sqlx::query_scalar(
            r#"SELECT t.patient_id FROM patient_search_tokens t
            JOIN patients p ON p.id = t.patient_id
            WHERE t.token_type = 'identity_key' AND t.token_text = ?
            AND p.user_id = ? AND p.deleted_at IS NULL
            AND p.id != ?
            LIMIT 1"#,
        )
        .bind(&input_key)
        .bind(user_id)
        .bind(exclude)
        .fetch_optional(db)
        .await
        .map_err(|_| AppError::internal("Failed to check duplicates."))?
    } else {
        sqlx::query_scalar(
            r#"SELECT t.patient_id FROM patient_search_tokens t
            JOIN patients p ON p.id = t.patient_id
            WHERE t.token_type = 'identity_key' AND t.token_text = ?
            AND p.user_id = ? AND p.deleted_at IS NULL
            LIMIT 1"#,
        )
        .bind(&input_key)
        .bind(user_id)
        .fetch_optional(db)
        .await
        .map_err(|_| AppError::internal("Failed to check duplicates."))?
    };

    if let Some(matched_id) = matched_id {
        return Ok(Some(get_patient_detail(db, user_id, &matched_id).await?));
    }

    // Fallback: load all + decrypt + check in-memory (for old records without tokens)
    let rows = sqlx::query_as::<_, PatientRow>(
        r#"SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|_| AppError::internal("Failed to check duplicates."))?;

    for row in rows {
        if let Some(exclude) = exclude_patient_id {
            if row.id == exclude {
                continue;
            }
        }
        let pii = decrypt_pii(&row, user_id)?;
        let existing_key = utils::build_patient_identity_key(&row.full_name, pii.phone.as_deref());
        if existing_key == input_key {
            return Ok(Some(row_to_patient(row, user_id)?));
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

    let mut q = sqlx::query_as::<_, PatientRow>(&query)
        .bind(user_id)
        .bind(normalized);
    if let Some(pid) = patient_id {
        q = q.bind(pid);
    }

    let result = q
        .fetch_optional(db)
        .await
        .map_err(|_| AppError::internal("Failed to check chart number."))?;

    match result {
        Some(row) => Ok(Some(row_to_patient(row, user_id)?)),
        None => Ok(None),
    }
}

pub async fn create_patient(
    db: &SqlitePool,
    user_id: &str,
    input: &CreatePatientInput,
) -> Result<Patient, AppError> {
    if input.full_name.trim().len() < 3 {
        return Err(AppError::bad_request("Informe o nome completo (min. 3 caracteres)."));
    }

    if (find_duplicate_patient(
        db, user_id, &input.full_name, input.phone.as_deref(), None,
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe um paciente com o mesmo nome e telefone na sua base.",
        ));
    }

    if (find_duplicate_chart_number(
        db, user_id, input.chart_number.as_deref(), None,
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe um paciente com este numero do prontuario na sua base.",
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let (pii_encrypted, pii_iv, pii_auth_tag) = encrypt_pii(input, user_id)?;

    sqlx::query(
        r#"INSERT INTO patients (id, user_id, full_name, chart_number, phone, email, birth_date,
            emergency_phone, status, created_at, updated_at,
            pii_encrypted, pii_iv, pii_auth_tag)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.full_name)
    .bind(&input.chart_number)
    .bind(&input.phone)
    .bind(&input.email)
    .bind(&input.birth_date)
    .bind(&input.emergency_phone)
    .bind(&now)
    .bind(&now)
    .bind(&pii_encrypted)
    .bind(&pii_iv)
    .bind(&pii_auth_tag)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create patient: {}", e)))?;

    // Index search tokens
    let pii = input_to_pii(input);
    let tokens = generate_patient_tokens(&id, &input.full_name, &pii);
    set_patient_tokens(db, &id, &tokens).await?;

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
    let _existing = get_patient_detail(db, user_id, patient_id).await?;

    if (find_duplicate_patient(
        db, user_id, &input.full_name, input.phone.as_deref(), Some(patient_id),
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe outro paciente com o mesmo nome e telefone na sua base.",
        ));
    }

    if (find_duplicate_chart_number(
        db, user_id, input.chart_number.as_deref(), Some(patient_id),
    ).await?).is_some() {
        return Err(AppError::conflict(
            "Ja existe um paciente com este numero do prontuario na sua base.",
        ));
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // Build a CreatePatientInput from the UpdatePatientInput for encryption
    let create_input = CreatePatientInput {
        full_name: input.full_name.clone(),
        chart_number: input.chart_number.clone(),
        phone: input.phone.clone(),
        email: input.email.clone(),
        birth_date: input.birth_date.clone(),
        health_history: input.health_history.clone(),
        medications_in_use: input.medications_in_use.clone(),
        emergency_phone: input.emergency_phone.clone(),
        admin_notes: input.admin_notes.clone(),
    };

    let (pii_encrypted, pii_iv, pii_auth_tag) = encrypt_pii(&create_input, user_id)?;

    sqlx::query(
        r#"UPDATE patients SET
            full_name = ?, chart_number = ?, phone = ?, email = ?, birth_date = ?,
            emergency_phone = ?,
            pii_encrypted = ?, pii_iv = ?, pii_auth_tag = ?,
            updated_at = ?
        WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.full_name)
    .bind(&input.chart_number)
    .bind(&input.phone)
    .bind(&input.email)
    .bind(&input.birth_date)
    .bind(&input.emergency_phone)
    .bind(&pii_encrypted)
    .bind(&pii_iv)
    .bind(&pii_auth_tag)
    .bind(&now)
    .bind(patient_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to update patient: {}", e)))?;

    // Re-index search tokens
    let pii = input_to_pii(&create_input);
    let tokens = generate_patient_tokens(patient_id, &input.full_name, &pii);
    set_patient_tokens(db, patient_id, &tokens).await?;

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
