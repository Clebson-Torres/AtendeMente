use std::path::Path;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::audit;
use crate::config::{AppConfig, MAX_UPLOAD_SIZE_BYTES};
use crate::crypto;
use crate::db::models::{FileUploadRequest, RecordFile};
use crate::errors::AppError;

const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "docx", "png", "jpg", "jpeg"];
const BLOCKED_EXTENSIONS: &[&str] = &["exe", "dll", "bat", "cmd", "ps1", "msi", "scr", "js", "vbs"];

fn sanitize_file_name(name: &str) -> String {
    let base = name
        .replace('\\', "/")
        .split('/')
        .next_back()
        .unwrap_or("arquivo")
        .chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim_matches('.')
        .trim()
        .to_string();
    if base.is_empty() {
        "arquivo.bin".to_string()
    } else {
        base
    }
}

fn file_extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn validate_file_metadata(input: &FileUploadRequest) -> Result<String, AppError> {
    if input.file_size < 0 || input.file_size as u64 > MAX_UPLOAD_SIZE_BYTES {
        return Err(AppError::bad_request("Arquivo excede o tamanho máximo de 20 MB."));
    }

    let sanitized = sanitize_file_name(&input.file_name);
    let ext = file_extension(&sanitized);
    if BLOCKED_EXTENSIONS.contains(&ext.as_str()) || !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::bad_request("Extensao de arquivo nao permitida."));
    }
    Ok(sanitized)
}

fn validate_file_content(bytes: &[u8], declared_name: &str, declared_mime: &str) -> Result<(), AppError> {
    let kind = infer::get(bytes);
    let ext = file_extension(declared_name);

    let allowed_pairs: &[(&str, &[&str])] = &[
        ("application/pdf",  &["pdf"]),
        ("image/jpeg",       &["jpg", "jpeg"]),
        ("image/png",        &["png"]),
        ("application/vnd.openxmlformats-officedocument.wordprocessingml.document", &["docx"]),
    ];
    let declared_pair_valid = allowed_pairs.iter().any(|(m, exts)| {
        *m == declared_mime && exts.contains(&ext.as_str())
    });
    if !declared_pair_valid {
        return Err(AppError::bad_request(
            "MIME type declarado nao corresponde a extensao permitida.",
        ));
    }

    match kind {
        Some(k) => {
            let mime = k.mime_type();
            let mime = if ext == "docx" && mime == "application/zip" {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            } else {
                mime
            };
            let valid = allowed_pairs.iter().any(|(m, exts)| {
                *m == mime && exts.contains(&ext.as_str())
            });
            if !valid {
                return Err(AppError::bad_request(
                    "Tipo de arquivo não permitido ou extensão não corresponde ao conteúdo.",
                ));
            }
        }
        None => {
            return Err(AppError::bad_request("Tipo de arquivo não reconhecido."));
        }
    }
    Ok(())
}

pub async fn create_upload_session(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    input: &FileUploadRequest,
) -> Result<(String, String), AppError> {
    let sanitized_name = validate_file_metadata(input)?;

    // Verify appointment
    let _appt = sqlx::query_as::<_, (String,)>(
        r#"SELECT id FROM appointments
        WHERE id = ? AND user_id = ? AND patient_id = ? AND deleted_at IS NULL"#,
    )
    .bind(&input.appointment_id)
    .bind(user_id)
    .bind(&input.patient_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Atendimento nao encontrado."))?;

    // Verify payment if receipt
    let payment_id = if input.kind == "payment_receipt" {
        let pid = input.payment_id.as_ref().ok_or_else(|| {
            AppError::bad_request("Informe o pagamento vinculado ao recibo.")
        })?;

        sqlx::query_as::<_, (String,)>(
            r#"SELECT id FROM payments
            WHERE id = ? AND user_id = ? AND appointment_id = ? AND deleted_at IS NULL"#,
        )
        .bind(pid)
        .bind(user_id)
        .bind(&input.appointment_id)
        .fetch_optional(db)
        .await
        .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
        .ok_or_else(|| AppError::not_found("Pagamento nao encontrado para vincular o recibo."))?;

        Some(pid.clone())
    } else {
        None
    };

    // Create storage path
    let storage_path = config.storage_path_for(
        user_id,
        &input.patient_id,
        &input.appointment_id,
        &sanitized_name,
    )?;

    // Ensure storage dir exists
    if let Some(parent) = storage_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("Failed to create storage dir: {}", e)))?;
    }

    let storage_path_str = storage_path.to_string_lossy().to_string();

    // Create record
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"INSERT INTO record_files (id, user_id, patient_id, appointment_id, payment_id,
            kind, storage_path, original_name, mime_type, byte_size, uploaded_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))"#,
    )
    .bind(&id)
    .bind(user_id)
    .bind(&input.patient_id)
    .bind(&input.appointment_id)
    .bind(&payment_id)
    .bind(&input.kind)
    .bind(&storage_path_str)
    .bind(&sanitized_name)
    .bind(&input.mime_type)
    .bind(input.file_size)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create file record: {}", e)))?;

    Ok((id, storage_path_str))
}

pub async fn write_upload_content(
    db: &SqlitePool,
    user_id: &str,
    file_id: &str,
    data: &[u8],
) -> Result<(), AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Upload session nao encontrada."))?;

    if let Some(parent) = std::path::Path::new(&file.storage_path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("Failed to create dir: {}", e)))?;
    }

    let key = crypto::load_key(user_id)
        .map_err(|e| AppError::internal(format!("Chave de criptografia nao disponivel: {}", e)))?;
    let encrypted = crypto::encrypt_file(data, &key)
        .map_err(|e| AppError::internal(format!("Falha ao criptografar arquivo: {}", e)))?;

    tokio::fs::write(&file.storage_path, &encrypted)
        .await
        .map_err(|e| AppError::internal(format!("Failed to write file: {}", e)))?;

    Ok(())
}

pub async fn confirm_upload(
    db: &SqlitePool,
    _config: &AppConfig,
    user_id: &str,
    file_id: &str,
) -> Result<RecordFile, AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Arquivo nao encontrado."))?;

    // Verify file exists on disk and read it
    let path = std::path::Path::new(&file.storage_path);
    let raw = tokio::fs::read(path).await
        .map_err(|_| AppError::bad_request(
            "Arquivo nao encontrado no armazenamento. Faca o upload novamente.",
        ))?;

    // Sem a chave, NAO seguir adiante.
    //
    // O fallback anterior servia o ciphertext como se fosse o conteudo. Como o
    // ciphertext e maior que o texto claro, a validacao de tamanho logo abaixo
    // era sempre falsa e o bloco de rejeicao APAGAVA o arquivo do usuario e
    // marcava a linha como excluida. Bastava a tela bloquear ou o backend
    // reiniciar durante um upload para o anexo ser destruido.
    //
    // "Sem chave" e uma condicao transitoria — a resposta certa e pedir
    // desbloqueio e preservar o arquivo, nunca confundir com "arquivo invalido".
    crypto::load_key(user_id).map_err(|_| {
        AppError::unauthorized(
            "Sessao bloqueada: desbloqueie o aplicativo para concluir o envio. \
             O arquivo foi preservado.",
        )
    })?;
    // Tentativa dupla: durante uma rotacao de chave o arquivo pode ter sido
    // gravado sob a chave que esta saindo.
    let data = crypto::decrypt_file_trying_all(&raw, user_id)
        .map_err(|e| AppError::internal(format!("Erro ao descriptografar: {}", e)))?;

    // Verify plaintext size matches declared size
    if data.len() != file.byte_size as usize {
        let _ = tokio::fs::remove_file(path).await;
        sqlx::query("UPDATE record_files SET deleted_at = datetime('now') WHERE id = ? AND user_id = ?")
            .bind(file_id)
            .bind(user_id)
            .execute(db)
            .await
            .ok();
        return Err(AppError::bad_request(
            "Arquivo rejeitado pela validacao de seguranca (tamanho divergente).",
        ));
    }

    if data.len() > MAX_UPLOAD_SIZE_BYTES as usize {
        let _ = tokio::fs::remove_file(path).await;
        sqlx::query("UPDATE record_files SET deleted_at = datetime('now') WHERE id = ? AND user_id = ?")
            .bind(file_id)
            .bind(user_id)
            .execute(db)
            .await
            .ok();
        return Err(AppError::bad_request(
            "Arquivo excede o tamanho máximo de 20 MB.",
        ));
    }

    // Validate magic bytes
    validate_file_content(&data, &file.original_name, &file.mime_type)?;

    // Audit log
    audit::write_audit_log(
        db,
        user_id,
        "file_upload",
        "record_file",
        Some(file_id),
        // Sem `original_name`: um nome escolhido pelo usuario descreve o
        // paciente e muitas vezes o diagnostico ("laudo-maria-silva-cid-f41.pdf"),
        // e ficava em texto claro no MESMO banco onde o prontuario e cifrado.
        // A extensao e o tamanho respondem o que a auditoria precisa saber.
        Some(&serde_json::json!({
            "kind": file.kind,
            "ext": file_extension(&file.original_name),
            "byte_size": file.byte_size,
            "appointment_id": file.appointment_id,
            "patient_id": file.patient_id,
        })),
        None,
        None,
    )
    .await?;

    Ok(file)
}

pub async fn download_file(
    db: &SqlitePool,
    user_id: &str,
    file_id: &str,
) -> Result<(RecordFile, Vec<u8>), AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Arquivo nao encontrado."))?;

    let path = std::path::Path::new(&file.storage_path);
    let raw = tokio::fs::read(path)
        .await
        .map_err(|_| AppError::not_found("Arquivo nao encontrado no disco."))?;

    // Sem a chave, recusar. Servir o ciphertext entregava bytes ilegiveis com
    // status 200 e nome de arquivo legitimo: o usuario salvaria um PDF corrompido
    // sem entender por que, e um anexo assim tem tudo para ser confundido com
    // perda de dado.
    crypto::load_key(user_id).map_err(|_| {
        AppError::unauthorized("Sessao bloqueada: desbloqueie o aplicativo para baixar o anexo.")
    })?;
    let data = crypto::decrypt_file_trying_all(&raw, user_id)
        .map_err(|e| AppError::internal(format!("Erro ao descriptografar: {}", e)))?;

    audit::write_audit_log(
        db,
        user_id,
        "file_download",
        "record_file",
        Some(file_id),
        Some(&serde_json::json!({
            "kind": file.kind,
            "ext": file_extension(&file.original_name),
        })),
        None,
        None,
    )
    .await?;

    Ok((file, data))
}

pub async fn list_files_by_appointment(
    db: &SqlitePool,
    user_id: &str,
    appointment_id: &str,
) -> Result<Vec<RecordFile>, AppError> {
    let files = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files
        WHERE user_id = ? AND appointment_id = ? AND deleted_at IS NULL
        ORDER BY uploaded_at DESC"#,
    )
    .bind(user_id)
    .bind(appointment_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?;

    Ok(files)
}

pub async fn delete_file(
    db: &SqlitePool,
    user_id: &str,
    file_id: &str,
) -> Result<(), AppError> {
    let file = sqlx::query_as::<_, RecordFile>(
        r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("DB error: {}", e)))?
    .ok_or_else(|| AppError::not_found("Arquivo nao encontrado."))?;

    if let Err(e) = tokio::fs::remove_file(&file.storage_path).await {
        tracing::warn!("[Files] Falha ao remover arquivo fisico '{}': {}", file.storage_path, e);
    }

    sqlx::query("UPDATE record_files SET deleted_at = datetime('now') WHERE id = ? AND user_id = ?")
        .bind(file_id)
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("DB error: {}", e)))?;

    audit::write_audit_log(
        db,
        user_id,
        "file_delete",
        "record_file",
        Some(file_id),
        Some(&serde_json::json!({
            "ext": file_extension(&file.original_name),
        })),
        None,
        None,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db;

    fn valid_pdf_bytes() -> Vec<u8> {
        b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF".to_vec()
    }

    async fn test_db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("files-test.db");
        let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = db::init_database(&url).await.unwrap();
        (dir, pool)
    }

    fn test_config(tmp: &tempfile::TempDir) -> AppConfig {
        let storage_dir = tmp.path().join("uploads");
        std::fs::create_dir_all(&storage_dir).unwrap();
        let pepper = [0x42u8; 32];
        crate::crypto::set_pepper(&pepper);
        AppConfig {
            database_url: String::new(),
            auth_database_url: String::new(),
            server_port: 3001,
            master_pepper: [0u8; 32],
            storage_dir,
            data_dir: tmp.path().join("data"),
        }
    }

    async fn seed_data(db: &SqlitePool, user_id: &str, patient_id: &str, appointment_id: &str) {
        let now = "2026-06-17T10:00:00";
        sqlx::query("INSERT INTO users (id, email, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(user_id)
            .bind("test@test.com")
            .bind(now)
            .bind(now)
            .execute(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO patients (id, user_id, full_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(patient_id)
            .bind(user_id)
            .bind("Test Patient")
            .bind(now)
            .bind(now)
            .execute(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO appointments (id, user_id, patient_id, starts_at, ends_at, status, session_price_cents, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(appointment_id)
            .bind(user_id)
            .bind(patient_id)
            .bind("2026-06-17T14:00:00")
            .bind("2026-06-17T15:00:00")
            .bind("scheduled")
            .bind(10000i64)
            .bind(now)
            .bind(now)
            .execute(db)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_create_upload_session() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: 200,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _storage_path) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session should be created");

        assert!(!file_id.is_empty());
    }

    /// Sem a chave, `confirm_upload` APAGAVA o arquivo do usuario.
    ///
    /// O fallback servia o ciphertext como se fosse o conteudo; como ele e maior
    /// que o texto claro, a validacao de tamanho reprovava sempre e o bloco de
    /// rejeicao fazia `remove_file` + soft delete. Bastava a tela bloquear ou o
    /// backend reiniciar durante um upload. Agora e 401 e o arquivo fica.
    #[tokio::test]
    async fn confirm_upload_sem_chave_preserva_o_arquivo() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-4466554400aa";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let patient_id = "550e8400-e29b-41d4-a716-4466554400ab";
        let appt_id = "550e8400-e29b-41d4-a716-4466554400ac";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf = valid_pdf_bytes();
        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "preservar.pdf".into(),
            file_size: pdf.len() as i64,
            mime_type: "application/pdf".into(),
        };
        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf)
            .await
            .expect("write");

        let caminho: String =
            sqlx::query_scalar("SELECT storage_path FROM record_files WHERE id = ?")
                .bind(&file_id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(std::path::Path::new(&caminho).exists(), "arquivo deveria existir");

        // Simula a tela bloqueada / backend reiniciado entre o upload e o confirm.
        crate::crypto::clear_user_crypto(user_id);
        let erro = confirm_upload(&db, &config, user_id, &file_id).await.expect_err("deveria recusar");

        assert!(
            std::path::Path::new(&caminho).exists(),
            "o arquivo NAO pode ser apagado por falta de chave; erro foi: {:?}",
            erro
        );
        let excluido: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM record_files WHERE id = ?")
                .bind(&file_id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(excluido.is_none(), "a linha nao pode ser marcada como excluida");

        // Com a chave de volta, o mesmo confirm conclui normalmente.
        crate::crypto::init_user_crypto(user_id).unwrap();
        confirm_upload(&db, &config, user_id, &file_id).await.expect("confirm apos desbloqueio");
    }

    /// Sem a chave, `download_file` entregava ciphertext com status 200 — o
    /// usuario salvaria um PDF corrompido sem entender por que.
    #[tokio::test]
    async fn download_sem_chave_recusa_em_vez_de_servir_ciphertext() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-4466554400ba";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let patient_id = "550e8400-e29b-41d4-a716-4466554400bb";
        let appt_id = "550e8400-e29b-41d4-a716-4466554400bc";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf = valid_pdf_bytes();
        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "baixar.pdf".into(),
            file_size: pdf.len() as i64,
            mime_type: "application/pdf".into(),
        };
        let (file_id, _) = create_upload_session(&db, &config, user_id, &input).await.unwrap();
        write_upload_content(&db, user_id, &file_id, &pdf).await.unwrap();
        confirm_upload(&db, &config, user_id, &file_id).await.unwrap();

        let (_, claro) = download_file(&db, user_id, &file_id).await.expect("download com chave");
        assert_eq!(claro, pdf);

        crate::crypto::clear_user_crypto(user_id);
        assert!(
            download_file(&db, user_id, &file_id).await.is_err(),
            "sem chave o download tem de recusar, nao devolver ciphertext"
        );
    }

    #[tokio::test]
    async fn test_write_and_confirm_upload() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");

        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");

        let file = confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        assert_eq!(file.original_name, "test.pdf");
        assert_eq!(file.kind, "session_attachment");
        assert_eq!(file.byte_size, pdf_data.len() as i64);
    }

    #[tokio::test]
    async fn test_list_files_by_appointment() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");
        confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        let files = list_files_by_appointment(&db, user_id, appt_id)
            .await
            .expect("list files");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].original_name, "test.pdf");
    }

    #[tokio::test]
    async fn test_download_file() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");
        confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        let (file, data) = download_file(&db, user_id, &file_id)
            .await
            .expect("download file");

        assert_eq!(file.original_name, "test.pdf");
        assert_eq!(data, pdf_data);
    }

    #[tokio::test]
    async fn test_validate_file_content_pdf() {
        let pdf_data = valid_pdf_bytes();
        assert!(validate_file_content(&pdf_data, "test.pdf", "application/pdf").is_ok());
    }

    #[tokio::test]
    async fn test_validate_file_content_rejects_mismatch() {
        let pdf_data = valid_pdf_bytes();
        // PDF content with .png extension should fail
        assert!(validate_file_content(&pdf_data, "test.png", "image/png").is_err());
    }

    #[tokio::test]
    async fn test_upload_wrong_user_cannot_access() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let wrong_user = "550e8400-e29b-41d4-a716-446655449999";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let pdf_data = valid_pdf_bytes();
        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("upload session");
        write_upload_content(&db, user_id, &file_id, &pdf_data)
            .await
            .expect("write content");
        confirm_upload(&db, &config, user_id, &file_id)
            .await
            .expect("confirm upload");

        // Wrong user should not be able to list
        let result = list_files_by_appointment(&db, wrong_user, appt_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        // Wrong user should not be able to download
        let result = download_file(&db, wrong_user, &file_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_invalid_extension_before_writing_file() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "malware.pdf.exe".into(),
            file_size: 10,
            mime_type: "application/pdf".into(),
        };

        assert!(create_upload_session(&db, &config, user_id, &input).await.is_err());
    }

    #[tokio::test]
    async fn rejects_declared_mime_that_does_not_match_content() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        crate::crypto::init_user_crypto(user_id).unwrap();
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;
        let pdf_data = valid_pdf_bytes();

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "test.pdf".into(),
            file_size: pdf_data.len() as i64,
            mime_type: "image/png".into(),
        };
        let (file_id, _) = create_upload_session(&db, &config, user_id, &input)
            .await
            .expect("session is created before content validation");
        write_upload_content(&db, user_id, &file_id, &pdf_data).await.unwrap();

        assert!(confirm_upload(&db, &config, user_id, &file_id).await.is_err());
    }

    #[tokio::test]
    async fn rejects_file_above_configured_limit() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "large.pdf".into(),
            file_size: (crate::config::MAX_UPLOAD_SIZE_BYTES + 1) as i64,
            mime_type: "application/pdf".into(),
        };

        assert!(create_upload_session(&db, &config, user_id, &input).await.is_err());
    }

    #[tokio::test]
    async fn sanitizes_original_name_and_uses_uuid_storage_name() {
        let (tmp, db) = test_db().await;
        let config = test_config(&tmp);
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        let patient_id = "550e8400-e29b-41d4-a716-446655440002";
        let appt_id = "550e8400-e29b-41d4-a716-446655440003";
        seed_data(&db, user_id, patient_id, appt_id).await;

        let input = FileUploadRequest {
            appointment_id: appt_id.into(),
            patient_id: patient_id.into(),
            payment_id: None,
            kind: "session_attachment".into(),
            file_name: "..\\laudo:clinico?.pdf".into(),
            file_size: valid_pdf_bytes().len() as i64,
            mime_type: "application/pdf".into(),
        };

        let (file_id, storage_path) = create_upload_session(&db, &config, user_id, &input)
            .await
            .unwrap();
        let file = sqlx::query_as::<_, RecordFile>("SELECT * FROM record_files WHERE id = ?")
            .bind(file_id)
            .fetch_one(&db)
            .await
            .unwrap();

        assert_eq!(file.original_name, "laudo_clinico_.pdf");
        assert!(!storage_path.contains("laudo"));
        assert!(storage_path.ends_with(".pdf"));
    }
}
