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

// ─── Legacy plaintext PII migration ──────────────────────────────────────────────

/// Moves patient PII out of the legacy plaintext columns and into the encrypted
/// blob, then clears the plaintext.
///
/// Covers two cases left behind by older versions:
///   * rows written before migration 20240101000008 (plaintext only), and
///   * rows written by versions that wrote *both* the plaintext columns and
///     `pii_encrypted` — for those the encrypted copy is authoritative and the
///     plaintext is simply removed.
///
/// Requires the user's key to be loaded, so it runs after login/unlock. It is
/// idempotent and safe to call on every login: once no row has plaintext left,
/// the initial query returns nothing.
pub async fn migrate_plaintext_pii(db: &SqlitePool, user_id: &str) -> Result<u64, AppError> {
    let rows = sqlx::query_as::<_, PatientRow>(
        r#"SELECT * FROM patients
        WHERE user_id = ?
        AND (phone IS NOT NULL OR email IS NOT NULL OR birth_date IS NOT NULL
             OR emergency_phone IS NOT NULL OR health_history IS NOT NULL
             OR medications_in_use IS NOT NULL OR admin_notes IS NOT NULL)"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao listar PII em texto claro: {}", e)))?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut migrated = 0u64;

    for row in rows {
        let patient_id = row.id.clone();

        if row.pii_encrypted.is_some() {
            // Only drop the plaintext once the encrypted copy is proven readable.
            //
            // Trusting `pii_encrypted.is_some()` was destructive: if the blob could
            // not be decrypted — a pepper that was rotated, restored from another
            // machine, or lost — this deleted the only surviving readable copy of
            // the patient's contact data. Verify first, and when the blob is
            // unreadable treat the plaintext as the source of truth and re-encrypt
            // from it instead.
            match decrypt_pii(&row, user_id) {
                Ok(_) => {
                    clear_plaintext_pii(db, user_id, &patient_id).await?;
                    migrated += 1;
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        "[Patients] PII cifrada do paciente {} nao pode ser lida ({}); \
                         re-cifrando a partir das colunas em texto claro em vez de descarta-las.",
                        patient_id,
                        e
                    );
                    // Falls through to the re-encrypt path below.
                }
            }
        }

        // Normalize empty strings to None, exactly like `input_to_pii` does on the
        // write path. Older rows can hold `''` instead of NULL, and carrying that
        // through produced `Some("")` in the blob plus a junk empty row in the
        // search index.
        let non_empty = |v: &Option<String>| v.clone().filter(|s| !s.trim().is_empty());
        let pii = PatientPii {
            phone: non_empty(&row.phone),
            email: non_empty(&row.email),
            birth_date: non_empty(&row.birth_date),
            emergency_phone: non_empty(&row.emergency_phone),
            health_history: non_empty(&row.health_history),
            medications_in_use: non_empty(&row.medications_in_use),
            admin_notes: non_empty(&row.admin_notes),
        };
        let json = serde_json::to_string(&pii)
            .map_err(|e| AppError::internal(format!("Erro ao serializar PII: {}", e)))?;
        let encrypted = crypto::encrypt_content(&json, user_id)?;

        sqlx::query(
            r#"UPDATE patients SET
                pii_encrypted = ?, pii_iv = ?, pii_auth_tag = ?,
                phone = NULL, email = NULL, birth_date = NULL, emergency_phone = NULL,
                health_history = NULL, medications_in_use = NULL, admin_notes = NULL
            WHERE id = ? AND user_id = ?"#,
        )
        .bind(&encrypted.encrypted_payload)
        .bind(&encrypted.iv)
        .bind(&encrypted.auth_tag)
        .bind(&patient_id)
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao cifrar PII do paciente: {}", e)))?;

        // Keep the search index in sync with the values just encrypted.
        let tokens = generate_patient_tokens(&patient_id, &row.full_name, &pii);
        set_patient_tokens(db, &patient_id, &tokens).await?;

        migrated += 1;
    }

    tracing::info!("[Patients] PII migrada para armazenamento cifrado: {} registro(s)", migrated);
    Ok(migrated)
}

async fn clear_plaintext_pii(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE patients SET
            phone = NULL, email = NULL, birth_date = NULL, emergency_phone = NULL,
            health_history = NULL, medications_in_use = NULL, admin_notes = NULL
        WHERE id = ? AND user_id = ?"#,
    )
    .bind(patient_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao limpar PII em texto claro: {}", e)))?;
    Ok(())
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

    // One row that fails to decrypt must not take down the whole list. That is
    // reachable after restoring a backup made under a different pepper: the main
    // patient screen answered 500 and the app became unusable instead of showing
    // the records that are fine.
    //
    // The row is still listed, with the contact fields empty — the name and chart
    // number live in plaintext columns, so the patient stays visible and can be
    // opened and corrected. Dropping it silently would be worse: the patient would
    // simply vanish from the list.
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        match row_to_patient_list_item(row.clone(), user_id) {
            Ok(item) => items.push(item),
            Err(e) => {
                tracing::warn!(
                    "[Patients] PII do paciente {} nao pode ser lida; listado sem os \
                     dados de contato: {}",
                    row.id,
                    e
                );
                items.push(PatientListItem {
                    id: row.id,
                    full_name: row.full_name,
                    chart_number: row.chart_number,
                    phone: None,
                    email: None,
                    birth_date: None,
                    status: row.status,
                    created_at: row.created_at,
                });
            }
        }
    }

    Ok(PaginatedData {
        items,
        total,
        page,
        per_page,
    })
}

/// All non-deleted patients with PII decrypted, for CSV export.
/// Rows that fail to decrypt are skipped with a warning rather than aborting
/// the whole export.
pub async fn list_all_for_export(
    db: &SqlitePool,
    user_id: &str,
) -> Result<Vec<Patient>, AppError> {
    let rows = sqlx::query_as::<_, PatientRow>(
        r#"SELECT * FROM patients WHERE user_id = ? AND deleted_at IS NULL ORDER BY full_name"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to export patients: {}", e)))?;

    let mut patients = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row.id.clone();
        match row_to_patient(row, user_id) {
            Ok(p) => patients.push(p),
            Err(e) => tracing::warn!("[Patients] Paciente {} omitido do export: {}", id, e),
        }
    }
    Ok(patients)
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
        // A single undecryptable row (e.g. restored from a backup made under a
        // different pepper) must not block creating or editing every other
        // patient — skip it instead of failing the whole request.
        let pii = match decrypt_pii(&row, user_id) {
            Ok(pii) => pii,
            Err(e) => {
                tracing::warn!(
                    "[Patients] Ignorando paciente {} na checagem de duplicatas: {}",
                    row.id,
                    e
                );
                continue;
            }
        };
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

    // PII (phone/email/birth_date/emergency_phone/...) goes only into the
    // encrypted blob. The legacy plaintext columns are left NULL — writing both
    // defeated migration 20240101000008 and left contact data readable in the
    // database file. `decrypt_pii` still reads those columns for rows written
    // before this change.
    sqlx::query(
        r#"INSERT INTO patients (id, user_id, full_name, chart_number,
            status, created_at, updated_at,
            pii_encrypted, pii_iv, pii_auth_tag)
        VALUES (?, ?, ?, ?, 'active', ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.full_name)
    .bind(&input.chart_number)
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

    // Clear the legacy plaintext PII columns: editing a patient created by an
    // older version is what removes their readable contact data from the file.
    sqlx::query(
        r#"UPDATE patients SET
            full_name = ?, chart_number = ?,
            phone = NULL, email = NULL, birth_date = NULL, emergency_phone = NULL,
            health_history = NULL, medications_in_use = NULL, admin_notes = NULL,
            pii_encrypted = ?, pii_iv = ?, pii_auth_tag = ?,
            updated_at = ?
        WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.full_name)
    .bind(&input.chart_number)
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("patients-test.db");
        let url = format!("sqlite:{}?mode=rwc", path.to_string_lossy());
        let pool = crate::db::init_database(&url).await.unwrap();
        (dir, pool)
    }

    async fn seed_user(db: &SqlitePool, user_id: &str) {
        sqlx::query(
            "INSERT INTO users (id, email, full_name, created_at, updated_at) \
             VALUES (?, 'u@test.com', 'U', '2026-01-01T00:00:00', '2026-01-01T00:00:00')",
        )
        .bind(user_id)
        .execute(db)
        .await
        .unwrap();
    }

    /// The plaintext columns must survive when the encrypted blob cannot be read.
    ///
    /// Regression test for a destructive bug: the migration used to drop the
    /// plaintext whenever `pii_encrypted` was merely present, so a pepper that no
    /// longer matched turned a recoverable row into permanent data loss.
    #[tokio::test]
    async fn does_not_discard_plaintext_when_the_encrypted_blob_is_unreadable() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400f1";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        // Row as an older version left it: plaintext PII *and* a blob that this
        // key cannot decrypt (simulating a rotated/lost pepper).
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, phone, email, birth_date, \
             admin_notes, status, created_at, updated_at, pii_encrypted, pii_iv, pii_auth_tag) \
             VALUES ('p1', ?, 'Paciente Legado', '11999998888', 'p@test.com', '1990-05-02', \
             'nota', 'active', '2026-01-01T00:00:00', '2026-01-01T00:00:00', \
             'bm90LXJlYWxseS1jaXBoZXI=', 'YWFhYWFhYWFhYWFh', 'YmJiYmJiYmJiYmJiYmJiYg==')",
        )
        .bind(user_id)
        .execute(&db)
        .await
        .unwrap();

        let migrated = migrate_plaintext_pii(&db, user_id).await.unwrap();
        assert_eq!(migrated, 1);

        // The data is still readable through the normal path...
        let p = get_patient_detail(&db, user_id, "p1").await.unwrap();
        assert_eq!(p.phone.as_deref(), Some("11999998888"));
        assert_eq!(p.email.as_deref(), Some("p@test.com"));
        assert_eq!(p.birth_date.as_deref(), Some("1990-05-02"));
        assert_eq!(p.admin_notes.as_deref(), Some("nota"));

        // ...and it is now stored encrypted with the current key, with the
        // plaintext columns cleared only after a readable blob replaced them.
        let (phone, notes, blob): (Option<String>, Option<String>, Option<String>) =
            sqlx::query_as("SELECT phone, admin_notes, pii_encrypted FROM patients WHERE id = 'p1'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(phone.is_none(), "coluna em claro deve ter sido limpa apos re-cifrar");
        assert!(notes.is_none());
        assert_ne!(
            blob.as_deref(),
            Some("bm90LXJlYWxseS1jaXBoZXI="),
            "o blob ilegivel deve ter sido substituido"
        );
    }

    /// Fixa a protecao contra sobrescrita cega de um registro ilegivel.
    ///
    /// Se o blob de um paciente nao decifra e `list_patients` devolve a linha com
    /// os campos vazios, a usuaria pode "corrigir" um nome em branco e confirmar
    /// a perda. Isso hoje NAO acontece, porque `update_patient` comeca chamando
    /// `get_patient_detail`, que propaga o erro de decifra — mas nada no codigo
    /// dizia que essa chamada era a salvaguarda, e o `let _existing` parece
    /// descartavel. Este teste existe para que ela nao seja removida por engano.
    #[tokio::test]
    async fn update_recusa_paciente_com_blob_ilegivel_em_vez_de_sobrescrever() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400f7";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        // Blob que esta chave nao abre, e nenhuma coluna em claro para cair.
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, status, created_at, updated_at, \
             pii_encrypted, pii_iv, pii_auth_tag) \
             VALUES ('ilegivel', ?, 'Paciente Ilegivel', 'active', \
             '2026-01-01T00:00:00', '2026-01-01T00:00:00', \
             'bm90LXJlYWxseS1jaXBoZXI=', 'YWFhYWFhYWFhYWFh', 'YmJiYmJiYmJiYmJiYmJiYg==')",
        )
        .bind(user_id)
        .execute(&db)
        .await
        .unwrap();

        let alteracao = UpdatePatientInput {
            full_name: "Nome Corrigido".into(),
            chart_number: None,
            phone: None,
            email: None,
            birth_date: None,
            health_history: None,
            medications_in_use: None,
            emergency_phone: None,
            admin_notes: None,
        };
        assert!(
            update_patient(&db, user_id, "ilegivel", &alteracao).await.is_err(),
            "editar um registro que nao decifra tem de falhar, nunca sobrescrever"
        );

        // E o blob original continua intacto.
        let blob: Option<String> =
            sqlx::query_scalar("SELECT pii_encrypted FROM patients WHERE id = 'ilegivel'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            blob.as_deref(),
            Some("bm90LXJlYWxseS1jaXBoZXI="),
            "o blob nao pode ter sido substituido"
        );
    }

    fn input(nome: &str, prontuario: Option<&str>, tel: Option<&str>) -> CreatePatientInput {
        CreatePatientInput {
            full_name: nome.into(),
            chart_number: prontuario.map(Into::into),
            phone: tel.map(Into::into),
            email: None,
            birth_date: None,
            health_history: None,
            medications_in_use: None,
            emergency_phone: None,
            admin_notes: None,
        }
    }

    #[tokio::test]
    async fn rejects_short_name_and_duplicate_name_plus_phone() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a1";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        assert!(
            create_patient(&db, user_id, &input("Jo", None, None)).await.is_err(),
            "nome com menos de 3 caracteres deve ser rejeitado"
        );

        create_patient(&db, user_id, &input("Maria Souza", None, Some("11911112222")))
            .await
            .unwrap();

        // Mesmo nome + mesmo telefone = duplicata.
        let err = create_patient(&db, user_id, &input("Maria Souza", None, Some("11911112222")))
            .await
            .expect_err("mesmo nome e telefone deve conflitar");
        assert!(matches!(err, AppError::Conflict { .. }), "obtido: {err:?}");

        // Mesmo nome com telefone diferente e permitido (homonimos existem).
        create_patient(&db, user_id, &input("Maria Souza", None, Some("11933334444")))
            .await
            .expect("homonimo com telefone diferente deve ser aceito");
    }

    #[tokio::test]
    async fn rejects_duplicate_chart_number_but_allows_empty() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a2";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        create_patient(&db, user_id, &input("Paciente Um", Some("P001"), None)).await.unwrap();

        let err = create_patient(&db, user_id, &input("Paciente Dois", Some("P001"), None))
            .await
            .expect_err("prontuario repetido deve conflitar");
        assert!(matches!(err, AppError::Conflict { .. }), "obtido: {err:?}");

        // Prontuario vazio nao participa da unicidade.
        create_patient(&db, user_id, &input("Paciente Tres", None, None)).await.unwrap();
        create_patient(&db, user_id, &input("Paciente Quatro", None, None)).await.unwrap();
    }

    #[tokio::test]
    async fn status_toggles_and_filters_the_listing() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a3";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        let p = create_patient(&db, user_id, &input("Ana Beatriz", None, None)).await.unwrap();
        assert_eq!(p.status, "active", "paciente novo comeca ativo");

        let inativo = set_patient_status(&db, user_id, &p.id, false).await.unwrap();
        assert_eq!(inativo.status, "inactive");

        let ativos = list_patients(&db, user_id, "", 1, 50, Some("active")).await.unwrap();
        assert_eq!(ativos.total, 0, "inativo nao deve aparecer no filtro de ativos");

        let inativos = list_patients(&db, user_id, "", 1, 50, Some("inactive")).await.unwrap();
        assert_eq!(inativos.total, 1);

        let todos = list_patients(&db, user_id, "", 1, 50, None).await.unwrap();
        assert_eq!(todos.total, 1, "sem filtro deve listar independente do status");

        set_patient_status(&db, user_id, &p.id, true).await.unwrap();
        let ativos = list_patients(&db, user_id, "", 1, 50, Some("active")).await.unwrap();
        assert_eq!(ativos.total, 1, "reativar deve devolver ao filtro de ativos");
    }

    #[tokio::test]
    async fn searches_by_name_and_by_phone() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a4";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        create_patient(&db, user_id, &input("Carlos Eduardo", Some("P010"), Some("11987654321")))
            .await
            .unwrap();
        create_patient(&db, user_id, &input("Fernanda Costa", Some("P011"), Some("11943210987")))
            .await
            .unwrap();

        let por_nome = list_patients(&db, user_id, "Carlos", 1, 50, None).await.unwrap();
        assert_eq!(por_nome.total, 1);
        assert_eq!(por_nome.items[0].full_name, "Carlos Eduardo");

        // Telefone esta cifrado na linha; a busca usa o indice de tokens.
        let por_telefone = list_patients(&db, user_id, "11943210987", 1, 50, None).await.unwrap();
        assert_eq!(por_telefone.total, 1, "busca por telefone deve usar o indice");
        assert_eq!(por_telefone.items[0].full_name, "Fernanda Costa");

        let nada = list_patients(&db, user_id, "Inexistente", 1, 50, None).await.unwrap();
        assert_eq!(nada.total, 0);
    }

    #[tokio::test]
    async fn pagination_reports_the_full_total() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a5";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        for i in 0..5 {
            create_patient(&db, user_id, &input(&format!("Paciente {i:02}"), None, None))
                .await
                .unwrap();
        }

        let pag1 = list_patients(&db, user_id, "", 1, 2, None).await.unwrap();
        assert_eq!(pag1.items.len(), 2);
        assert_eq!(pag1.total, 5, "total deve refletir todos, nao a pagina");

        let pag3 = list_patients(&db, user_id, "", 3, 2, None).await.unwrap();
        assert_eq!(pag3.items.len(), 1);
        assert_ne!(pag1.items[0].id, pag3.items[0].id);
    }

    #[tokio::test]
    async fn patients_are_scoped_to_their_owner() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a6";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;
        let outro = "550e8400-e29b-41d4-a716-4466554400ff";

        let p = create_patient(&db, user_id, &input("Paciente Privado", None, None)).await.unwrap();

        assert!(get_patient_detail(&db, outro, &p.id).await.is_err());
        assert!(set_patient_status(&db, outro, &p.id, false).await.is_err());
        let lista = list_patients(&db, outro, "", 1, 50, None).await.unwrap();
        assert_eq!(lista.total, 0);
    }

    #[tokio::test]
    async fn update_replaces_pii_and_keeps_it_readable() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400a7";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        let p = create_patient(&db, user_id, &input("Bruno Lima", Some("P020"), Some("11911112222")))
            .await
            .unwrap();

        let upd = UpdatePatientInput {
            full_name: "Bruno Lima".into(),
            chart_number: Some("P021".into()),
            phone: Some("11999998888".into()),
            email: Some("bruno@test.com".into()),
            birth_date: Some("1988-03-04".into()),
            health_history: Some("historico novo".into()),
            medications_in_use: None,
            emergency_phone: None,
            admin_notes: None,
        };
        let depois = update_patient(&db, user_id, &p.id, &upd).await.unwrap();

        assert_eq!(depois.chart_number.as_deref(), Some("P021"));
        assert_eq!(depois.phone.as_deref(), Some("11999998888"));
        assert_eq!(depois.email.as_deref(), Some("bruno@test.com"));
        assert_eq!(depois.health_history.as_deref(), Some("historico novo"));

        // Nenhuma coluna em texto claro deve sobrar depois de editar.
        let (tel, mail): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT phone, email FROM patients WHERE id = ?")
                .bind(&p.id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(tel.is_none() && mail.is_none(), "PII nao deve ficar em claro apos update");

        // E o indice de busca acompanhou o telefone novo.
        let achou = list_patients(&db, user_id, "11999998888", 1, 50, None).await.unwrap();
        assert_eq!(achou.total, 1, "tokens de busca devem ser reindexados no update");
    }

    /// Legacy rows can hold `''` instead of NULL. Those must not become
    /// `Some("")` in the blob nor create an empty row in the search index.
    #[tokio::test]
    async fn migration_normalizes_empty_strings_to_none() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400f4";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, phone, email, admin_notes, \
             status, created_at, updated_at) \
             VALUES ('p3', ?, 'Paciente', '11955558844', '', '   ', 'active', \
             '2026-01-01T00:00:00', '2026-01-01T00:00:00')",
        )
        .bind(user_id)
        .execute(&db)
        .await
        .unwrap();

        migrate_plaintext_pii(&db, user_id).await.unwrap();

        let p = get_patient_detail(&db, user_id, "p3").await.unwrap();
        assert_eq!(p.phone.as_deref(), Some("11955558844"));
        assert!(p.email.is_none(), "email vazio deve virar None, nao Some(\"\")");
        assert!(p.admin_notes.is_none(), "campo so com espacos deve virar None");

        let tokens: Vec<(String, String)> =
            sqlx::query_as("SELECT token_type, token_text FROM patient_search_tokens WHERE patient_id = 'p3'")
                .fetch_all(&db)
                .await
                .unwrap();
        assert!(
            !tokens.iter().any(|(_, txt)| txt.is_empty()),
            "nenhum token vazio no indice de busca; obtido: {tokens:?}"
        );
        assert!(tokens.iter().any(|(t, txt)| t == "phone" && txt == "11955558844"));
    }

    /// An undecryptable row must not break the whole patient list.
    #[tokio::test]
    async fn lists_patients_even_when_one_row_cannot_be_decrypted() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400f3";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        // Um paciente normal.
        create_patient(
            &db,
            user_id,
            &CreatePatientInput {
                full_name: "Paciente Bom".into(),
                chart_number: Some("P001".into()),
                phone: Some("11911112222".into()),
                email: None,
                birth_date: None,
                health_history: None,
                medications_in_use: None,
                emergency_phone: None,
                admin_notes: None,
            },
        )
        .await
        .unwrap();

        // Um paciente com blob ilegivel e SEM copia em claro (dado realmente perdido).
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, chart_number, status, \
             created_at, updated_at, pii_encrypted, pii_iv, pii_auth_tag) \
             VALUES ('quebrado', ?, 'Paciente Ilegivel', 'P002', 'active', \
             '2026-01-01T00:00:00', '2026-01-01T00:00:00', \
             'bm90LXJlYWxseS1jaXBoZXI=', 'YWFhYWFhYWFhYWFh', 'YmJiYmJiYmJiYmJiYmJiYg==')",
        )
        .bind(user_id)
        .execute(&db)
        .await
        .unwrap();

        let page = list_patients(&db, user_id, "", 1, 50, None)
            .await
            .expect("a listagem nao deve falhar por causa de uma linha ilegivel");

        assert_eq!(page.items.len(), 2, "ambos os pacientes devem aparecer");
        let quebrado = page.items.iter().find(|p| p.id == "quebrado").unwrap();
        assert_eq!(quebrado.full_name, "Paciente Ilegivel");
        assert_eq!(quebrado.chart_number.as_deref(), Some("P002"));
        assert!(quebrado.phone.is_none(), "contato indisponivel fica vazio");

        let bom = page.items.iter().find(|p| p.id != "quebrado").unwrap();
        assert_eq!(bom.phone.as_deref(), Some("11911112222"));
    }

    /// When the blob *is* readable it stays authoritative and the redundant
    /// plaintext is removed.
    #[tokio::test]
    async fn drops_plaintext_when_the_encrypted_blob_is_readable() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400f2";
        crate::crypto::set_pepper(&[3u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();
        seed_user(&db, user_id).await;

        let pii = PatientPii {
            phone: Some("11912345678".into()),
            email: None,
            birth_date: None,
            emergency_phone: None,
            health_history: None,
            medications_in_use: None,
            admin_notes: None,
        };
        let enc =
            crypto::encrypt_content(&serde_json::to_string(&pii).unwrap(), user_id).unwrap();

        // Double-written row: valid blob plus a stale plaintext copy.
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, phone, status, created_at, updated_at, \
             pii_encrypted, pii_iv, pii_auth_tag) \
             VALUES ('p2', ?, 'Paciente', '99999999999', 'active', \
             '2026-01-01T00:00:00', '2026-01-01T00:00:00', ?, ?, ?)",
        )
        .bind(user_id)
        .bind(&enc.encrypted_payload)
        .bind(&enc.iv)
        .bind(&enc.auth_tag)
        .execute(&db)
        .await
        .unwrap();

        migrate_plaintext_pii(&db, user_id).await.unwrap();

        let (phone, blob): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT phone, pii_encrypted FROM patients WHERE id = 'p2'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(phone.is_none(), "plaintext redundante deve ser removido");
        assert_eq!(
            blob.as_deref(),
            Some(enc.encrypted_payload.as_str()),
            "blob legivel deve ser preservado como esta"
        );

        // The blob's value wins, not the stale plaintext.
        let p = get_patient_detail(&db, user_id, "p2").await.unwrap();
        assert_eq!(p.phone.as_deref(), Some("11912345678"));
    }
}
