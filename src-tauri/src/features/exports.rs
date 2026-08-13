use sqlx::SqlitePool;
use zip::write::FileOptions;

use crate::audit;
use crate::crypto;
use crate::db::models::RecordFile;
use crate::errors::AppError;

pub struct ExportBundle {
    pub buffer: Vec<u8>,
    pub appointment_count: usize,
    pub file_count: usize,
}

pub async fn export_patients_csv(
    db: &SqlitePool,
    user_id: &str,
) -> Result<String, AppError> {
    // Read through the patients feature so PII comes from the encrypted blob
    // (with the legacy plaintext columns as fallback) instead of querying the
    // plaintext columns directly, which are no longer written.
    let patients = crate::features::patients::list_all_for_export(db, user_id).await?;

    let mut csv = String::from("Nome,Prontuário,Telefone,Email,Data de Nascimento,Status\n");
    for p in patients {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            escape_csv(&p.full_name),
            escape_csv(p.chart_number.as_deref().unwrap_or("")),
            escape_csv(p.phone.as_deref().unwrap_or("")),
            escape_csv(p.email.as_deref().unwrap_or("")),
            escape_csv(p.birth_date.as_deref().unwrap_or("")),
            escape_csv(&p.status),
        ));
    }
    Ok(csv)
}

/// Quotes a CSV field and neutralises spreadsheet formula injection.
///
/// A patient name such as `=cmd|'/c calc'!A1` is a valid name as far as this app
/// is concerned, but Excel/LibreOffice execute it on open. Prefixing with a
/// single quote makes the cell literal text.
pub(crate) fn escape_csv(s: &str) -> String {
    let needs_formula_guard = s
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'));

    let body = if needs_formula_guard {
        format!("'{}", s)
    } else {
        s.to_string()
    };

    if body.contains(',')
        || body.contains('"')
        || body.contains('\n')
        || body.contains('\r')
        || body.contains(';')
    {
        format!("\"{}\"", body.replace('"', "\"\""))
    } else {
        body
    }
}

/// Tamanho minimo da senha do export. Igual ao do backup, pelo mesmo motivo: o
/// arquivo contem o prontuario completo em texto claro dentro do envelope.
pub const MIN_EXPORT_PASSWORD_LEN: usize = 8;

/// Exporta o dossie de um paciente num ZIP **protegido por senha**.
///
/// O arquivo contem tudo em claro: nome, contato, historico clinico, medicacoes,
/// anotacoes, resumo de cada sessao e os anexos decifrados. Ele nascia sem
/// nenhuma protecao e ia para a pasta de Downloads — que frequentemente esta
/// sincronizada com nuvem. Era o maior vazamento em texto claro que restava no
/// app depois da reforma de criptografia.
///
/// A cifra e AES-256 do proprio formato ZIP, e nao o envelope `ATND` usado nos
/// backups. A diferenca importa: um export existe para SER ABERTO — pelo proprio
/// usuario, por um colega, as vezes pelo paciente — e nao ha caminho de import
/// para ele no app. Num formato proprietario o arquivo seria inutil. Assim ele
/// continua abrindo no 7-Zip, WinRAR ou Explorador, pedindo a senha.
pub async fn export_patient_bundle(
    db: &SqlitePool,
    user_id: &str,
    patient_id: &str,
    password: &str,
) -> Result<ExportBundle, AppError> {
    if password.chars().count() < MIN_EXPORT_PASSWORD_LEN {
        return Err(AppError::bad_request(format!(
            "Defina uma senha de no minimo {} caracteres para proteger a exportacao.",
            MIN_EXPORT_PASSWORD_LEN
        )));
    }
    // Get patient (PII decrypted from the encrypted blob)
    let patient = crate::features::patients::get_patient_detail(db, user_id, patient_id).await?;

    // Get appointments with records and payments
    let appointments = sqlx::query_as::<_, (String, String, String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>)>(
        r#"
        SELECT
            a.id, a.starts_at, a.ends_at, a.status, a.patient_id,
            a.session_price_cents, a.quick_notes,
            pay.status, pay.method, pay.amount_received_cents, pay.paid_at,
            sr.encrypted_payload, sr.iv, sr.auth_tag, sr.key_version
        FROM appointments a
        LEFT JOIN payments pay ON pay.appointment_id = a.id AND pay.deleted_at IS NULL
        LEFT JOIN session_records sr ON sr.appointment_id = a.id AND sr.deleted_at IS NULL
        WHERE a.user_id = ? AND a.patient_id = ? AND a.deleted_at IS NULL
        ORDER BY a.starts_at DESC
        "#,
    )
    .bind(user_id)
    .bind(patient_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get appointments: {}", e)))?;

    // Get all files for this patient
    let files = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files
        WHERE user_id = ? AND patient_id = ? AND deleted_at IS NULL
        ORDER BY uploaded_at"#,
    )
    .bind(user_id)
    .bind(patient_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to get files: {}", e)))?;

    let file_count = files.len();

    // Build manifest
    let manifest = build_manifest(user_id, &patient, &appointments, &files);

    // Create ZIP
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));

    let options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644)
        .with_aes_encryption(zip::AesMode::Aes256, password);

    // Add manifest.json
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| AppError::internal(format!("Failed to serialize manifest: {}", e)))?;
    zip.start_file("manifest.json", options)
        .map_err(|e| AppError::internal(format!("ZIP error: {}", e)))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| AppError::internal(format!("ZIP write error: {}", e)))?;

    use std::io::Write;

    // Add files directory
    for (i, appt) in appointments.iter().enumerate() {
        let appt_files: Vec<&RecordFile> = files
            .iter()
            .filter(|f| f.appointment_id == appt.0)
            .collect();

        for file in &appt_files {
            let path = std::path::Path::new(&file.storage_path);
            let raw = match tokio::fs::read(path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Attachments are stored encrypted; the export has to decrypt them
            // or the ZIP contains unreadable AES blobs. `decrypt_file` passes
            // through files written before at-rest encryption was added.
            let data = match crypto::load_key(user_id) {
                Ok(_) => match crypto::decrypt_file_trying_all(&raw, user_id) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!(
                            "[Export] Falha ao descriptografar anexo {}: {}",
                            file.original_name,
                            e
                        );
                        continue;
                    }
                },
                Err(e) => {
                    return Err(AppError::internal(format!(
                        "Chave de criptografia nao disponivel para exportar anexos: {}",
                        e
                    )))
                }
            };

            let zip_path = format!(
                "files/session_{}/{}/{}",
                i + 1,
                file.kind,
                &file.original_name
            );
            zip.start_file(&zip_path, options)
                .map_err(|e| AppError::internal(format!("ZIP error: {}", e)))?;
            zip.write_all(&data)
                .map_err(|e| AppError::internal(format!("ZIP write error: {}", e)))?;
        }
    }

    let buffer = zip
        .finish()
        .map_err(|e| AppError::internal(format!("ZIP finish error: {}", e)))?
        .into_inner();

    // Audit
    audit::write_audit_log(
        db,
        user_id,
        "patient_export",
        "patient",
        Some(patient_id),
        Some(&serde_json::json!({
            "appointment_count": appointments.len(),
            "file_count": file_count,
        })),
        None,
        None,
    )
    .await?;

    Ok(ExportBundle {
        buffer,
        appointment_count: appointments.len(),
        file_count,
    })
}

#[cfg(test)]
mod tests {
    use super::escape_csv;
    use sqlx::SqlitePool;

    const UID: &str = "550e8400-e29b-41d4-a716-4466554400e1";

    async fn base() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}?mode=rwc", dir.path().join("e.db").display());
        let db = crate::db::init_database(&url).await.unwrap();
        crate::crypto::set_pepper(&[0x31u8; 32]);
        crate::crypto::init_user_crypto(UID).unwrap();
        sqlx::query(
            "INSERT INTO users (id, email, full_name, created_at, updated_at) \
             VALUES (?, 'e@e.invalid', 'Nome', '2026-01-01', '2026-01-01')",
        )
        .bind(UID)
        .execute(&db)
        .await
        .unwrap();
        let pii = crate::crypto::encrypt_content(
            r#"{"phone":"11999990000","health_history":"FICTICIO: dado sensivel"}"#,
            UID,
        )
        .unwrap();
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, status, created_at, updated_at, \
             pii_encrypted, pii_iv, pii_auth_tag) VALUES ('pac', ?, 'Paciente Teste', \
             'active', '2026-01-01', '2026-01-01', ?, ?, ?)",
        )
        .bind(UID)
        .bind(&pii.encrypted_payload)
        .bind(&pii.iv)
        .bind(&pii.auth_tag)
        .execute(&db)
        .await
        .unwrap();
        (dir, db)
    }

    /// O dossie leva o prontuario completo em claro dentro dele e vai para a
    /// pasta de Downloads, que costuma estar sincronizada com nuvem. Exportar
    /// sem senha nao pode ser possivel.
    #[tokio::test]
    async fn recusa_exportar_sem_senha_ou_com_senha_curta() {
        let (_d, db) = base().await;
        for senha in ["", "1234567"] {
            // `expect_err` exigiria `Debug` em `ExportBundle`, e derivar Debug num
            // tipo que carrega o dossie inteiro em claro seria um convite a
            // vazamento por log. Melhor casar o resultado a mao.
            match super::export_patient_bundle(&db, UID, "pac", senha).await {
                Ok(_) => panic!("exportar com senha {:?} deveria ser recusado", senha),
                Err(e) => assert!(
                    format!("{:?}", e).contains("senha"),
                    "a mensagem tem de dizer o que falta: {:?}",
                    e
                ),
            }
        }
    }

    /// O ZIP tem de estar cifrado de verdade — e nao apenas "marcado" como tal.
    ///
    /// Duas garantias: a senha errada nao abre, e o conteudo sensivel nao aparece
    /// nos bytes do arquivo. A segunda existe porque um ZIP com nomes de entrada
    /// em claro e conteudo cifrado ainda vazaria se o manifesto nao estivesse
    /// coberto.
    #[tokio::test]
    async fn o_zip_exportado_exige_a_senha_e_nao_vaza_o_conteudo() {
        let (_d, db) = base().await;
        let bundle = super::export_patient_bundle(&db, UID, "pac", "senha-do-export")
            .await
            .unwrap();

        assert!(
            !String::from_utf8_lossy(&bundle.buffer).contains("11999990000"),
            "o telefone nao pode aparecer nos bytes do ZIP"
        );
        assert!(
            !String::from_utf8_lossy(&bundle.buffer).contains("dado sensivel"),
            "o historico clinico nao pode aparecer nos bytes do ZIP"
        );

        let mut zip =
            zip::ZipArchive::new(std::io::Cursor::new(bundle.buffer.clone())).unwrap();
        assert!(
            zip.by_name("manifest.json").is_err(),
            "sem senha, a entrada nao pode ser lida"
        );

        // Com a senha certa, o manifesto abre e traz a PII decifrada.
        let mut zip =
            zip::ZipArchive::new(std::io::Cursor::new(bundle.buffer)).unwrap();
        let mut entrada = zip
            .by_name_decrypt("manifest.json", b"senha-do-export")
            .expect("manifest.json deveria existir");
        let mut conteudo = String::new();
        std::io::Read::read_to_string(&mut entrada, &mut conteudo).unwrap();
        assert!(conteudo.contains("11999990000"), "o manifesto deve trazer a PII");
    }


    #[test]
    fn plain_values_pass_through() {
        assert_eq!(escape_csv("Maria Souza"), "Maria Souza");
        assert_eq!(escape_csv(""), "");
        assert_eq!(escape_csv("11999998888"), "11999998888");
    }

    #[test]
    fn quotes_fields_that_would_break_the_structure() {
        // A comma in any column used to shift every column after it.
        assert_eq!(escape_csv("Souza, Maria"), "\"Souza, Maria\"");
        assert_eq!(escape_csv("Ana \"Aninha\""), "\"Ana \"\"Aninha\"\"\"");
        assert_eq!(escape_csv("linha1\nlinha2"), "\"linha1\nlinha2\"");
        assert_eq!(escape_csv("a;b"), "\"a;b\"");
    }

    #[test]
    fn neutralizes_spreadsheet_formulas() {
        // Would execute on open in Excel/LibreOffice without the guard. No comma
        // in this value, so it only gets the apostrophe, not quoting.
        assert_eq!(escape_csv("=cmd|'/c calc'!A1"), "'=cmd|'/c calc'!A1");
        // With a comma it gets both.
        assert_eq!(escape_csv("=SUM(A1,B2)"), "\"'=SUM(A1,B2)\"");
        assert_eq!(escape_csv("+1+1"), "'+1+1");
        assert_eq!(escape_csv("-2+3"), "'-2+3");
        assert_eq!(escape_csv("@SUM(A1)"), "'@SUM(A1)");
    }

    #[test]
    fn does_not_mangle_names_containing_those_characters_mid_value() {
        // Only a *leading* formula character is dangerous.
        assert_eq!(escape_csv("Jean-Pierre"), "Jean-Pierre");
        assert_eq!(escape_csv("a@b.com"), "a@b.com");
    }
}

fn build_manifest(
    user_id: &str,
    patient: &crate::db::models::Patient,
    appointments: &Vec<(String, String, String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>)>,
    files: &[RecordFile],
) -> serde_json::Value {
    let appts: Vec<serde_json::Value> = appointments
        .iter()
        .enumerate()
        .map(|(_i, a)| {
            let summary = if let (Some(ep), Some(iv), Some(at), Some(kv)) =
                (&a.11, &a.12, &a.13, a.14)
            {
                let payload = crypto::EncryptedPayload {
                    encrypted_payload: ep.clone(),
                    iv: iv.clone(),
                    auth_tag: at.clone(),
                    key_version: kv,
                };
                crypto::decrypt_content(&payload, user_id).ok()
            } else {
                None
            };

            let appt_files: Vec<serde_json::Value> = files
                .iter()
                .filter(|f| f.appointment_id == a.0)
                .map(|f| {
                    serde_json::json!({
                        "id": f.id,
                        "kind": f.kind,
                        "original_name": f.original_name,
                        "mime_type": f.mime_type,
                        "byte_size": f.byte_size,
                        "uploaded_at": f.uploaded_at,
                    })
                })
                .collect();

            serde_json::json!({
                "appointmentId": a.0,
                "startsAt": a.1,
                "endsAt": a.2,
                "status": a.3,
                "sessionPriceCents": a.5,
                "quickNotes": a.6,
                "payment": {
                    "status": a.7,
                    "method": a.8,
                    "amountReceivedCents": a.9,
                    "paidAt": a.10,
                },
                "summary": summary,
                "files": appt_files,
            })
        })
        .collect();

    serde_json::json!({
        "exportedAt": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        "patient": {
            "id": patient.id,
            "fullName": patient.full_name,
            "chartNumber": patient.chart_number,
            "phone": patient.phone,
            "email": patient.email,
            "birthDate": patient.birth_date,
            "status": patient.status,
            "emergencyPhone": patient.emergency_phone,
            "healthHistory": patient.health_history,
            "medicationsInUse": patient.medications_in_use,
            "adminNotes": patient.admin_notes,
        },
        "appointments": appts,
    })
}
