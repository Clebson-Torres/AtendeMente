use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;
use zip::write::FileOptions;

use crate::audit::{self, AuditAction};
use crate::config::AppConfig;
use crate::crypto;
use crate::errors::AppError;

const BACKUP_VERSION: u32 = 2;
const BACKUP_VERSION_LEGACY: u32 = 1;
const DB_ENTRY: &str = "database/atendemente.db";
const MANIFEST_ENTRY: &str = "manifest.json";
const ATND_MAGIC: &[u8; 4] = b"ATND";
const SALT_SIZE: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: u32,
    pub created_at: String,
    pub user_id: String,
    pub app_version: String,
    pub file_hashes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pepper: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pepper_fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupBundle {
    pub file_name: String,
    pub bytes: Vec<u8>,
    pub manifest: BackupManifest,
    pub encrypted: bool,
}

pub async fn create_backup(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
) -> Result<BackupBundle, AppError> {
    create_backup_with_password(db, config, user_id, None).await
}

pub async fn create_backup_with_password(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    password: Option<&str>,
) -> Result<BackupBundle, AppError> {
    let created_at = chrono::Utc::now();
    let is_encrypted = password.is_some();
    let ext = if is_encrypted { "atendemente" } else { "zip" };
    let file_name = format!("backup_{}.{}", created_at.format("%Y%m%d_%H%M%S"), ext);
    let temp_db = std::env::temp_dir().join(format!("atendemente-backup-{}.db", Uuid::new_v4()));
    let temp_db_sql = sqlite_path_literal(&temp_db);
    sqlx::query(&format!("VACUUM INTO '{}'", temp_db_sql))
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao gerar copia consistente do banco: {}", e)))?;

    let db_bytes = tokio::fs::read(&temp_db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao ler banco temporario: {}", e)))?;
    let _ = tokio::fs::remove_file(&temp_db).await;

    let mut entries: Vec<(String, Vec<u8>)> = vec![(DB_ENTRY.to_string(), db_bytes)];
    let storage_root = config.storage_dir.join(user_id);
    if storage_root.exists() {
        collect_files(&storage_root, &storage_root, user_id, &mut entries).await?;
    }

    let mut file_hashes = BTreeMap::new();
    for (path, bytes) in &entries {
        file_hashes.insert(path.clone(), sha256_hex(bytes));
    }

    let (salt_hex, pepper_field, pepper_fp) = if is_encrypted {
        let mut salt = [0u8; SALT_SIZE];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        let fp = crypto::pepper_fingerprint().ok();
        let pepper_hex = crypto::get_pepper().map(|p| hex_encode(p));
        (Some(hex_encode(&salt)), pepper_hex, fp)
    } else {
        (None, None, None)
    };

    let manifest = BackupManifest {
        version: BACKUP_VERSION,
        created_at: created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        user_id: user_id.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        file_hashes,
        encrypted: if is_encrypted { Some(true) } else { None },
        kdf: if is_encrypted { Some("argon2id".into()) } else { None },
        salt: salt_hex.clone(),
        pepper: pepper_field,
        pepper_fingerprint: pepper_fp,
    };

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::internal(format!("Erro ao serializar manifesto: {}", e)))?;

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);

    zip.start_file(MANIFEST_ENTRY, options)
        .map_err(|e| AppError::internal(format!("Erro ZIP: {}", e)))?;
    zip.write_all(&manifest_bytes)
        .map_err(|e| AppError::internal(format!("Erro ao escrever manifesto: {}", e)))?;

    for (path, bytes) in &entries {
        zip.start_file(path, options)
            .map_err(|e| AppError::internal(format!("Erro ZIP: {}", e)))?;
        zip.write_all(bytes)
            .map_err(|e| AppError::internal(format!("Erro ao escrever backup: {}", e)))?;
    }

    let zip_bytes = zip
        .finish()
        .map_err(|e| AppError::internal(format!("Erro ao finalizar ZIP: {}", e)))?
        .into_inner();

    let final_bytes = if let Some(pass) = password {
        let salt_bytes = hex_decode(salt_hex.as_deref().unwrap_or_default())?;
        let key = crypto::derive_key_from_password(pass, &salt_bytes)?;
        let encrypted = crypto::encrypt_file(&zip_bytes, &key)?;
        let mut out = Vec::with_capacity(ATND_MAGIC.len() + SALT_SIZE + encrypted.len());
        out.extend_from_slice(ATND_MAGIC);
        out.extend_from_slice(&salt_bytes);
        out.extend_from_slice(&encrypted);
        out
    } else {
        zip_bytes
    };

    audit::write_audit_event(
        db,
        user_id,
        AuditAction::BackupCreated,
        "backup",
        None,
        serde_json::json!({"file_name": file_name, "entries": manifest.file_hashes.len(), "encrypted": is_encrypted}),
        Some("local-device"),
    )
    .await?;

    Ok(BackupBundle {
        file_name,
        bytes: final_bytes,
        manifest,
        encrypted: is_encrypted,
    })
}

pub async fn restore_backup(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    backup_bytes: &[u8],
) -> Result<BackupManifest, AppError> {
    restore_backup_with_password(db, config, user_id, backup_bytes, None).await
}

pub async fn restore_backup_with_password(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    backup_bytes: &[u8],
    password: Option<&str>,
) -> Result<BackupManifest, AppError> {
    let decrypted_bytes = if backup_bytes.starts_with(ATND_MAGIC) {
        if password.is_none() {
            return Err(AppError::bad_request("Backup criptografado requer senha."));
        }
        let pass = password.unwrap();
        if backup_bytes.len() < ATND_MAGIC.len() + SALT_SIZE + 29 {
            return Err(AppError::bad_request("Arquivo de backup invalido."));
        }
        let salt = &backup_bytes[ATND_MAGIC.len()..ATND_MAGIC.len() + SALT_SIZE];
        let encrypted = &backup_bytes[ATND_MAGIC.len() + SALT_SIZE..];
        let key = crypto::derive_key_from_password(pass, salt)?;
        crypto::decrypt_file(encrypted, &key)?
    } else {
        backup_bytes.to_vec()
    };

    let mut archive = zip::ZipArchive::new(Cursor::new(&decrypted_bytes))
        .map_err(|_| AppError::bad_request("Backup invalido ou corrompido."))?;
    let manifest = read_manifest(&mut archive)?;

    let accepted_versions = [BACKUP_VERSION, BACKUP_VERSION_LEGACY];
    if !accepted_versions.contains(&manifest.version) {
        return Err(AppError::bad_request(format!(
            "Versao de backup {} nao suportada.",
            manifest.version
        )));
    }
    if manifest.user_id != user_id {
        return Err(AppError::bad_request("Backup pertence a outro usuario."));
    }
    if !manifest.file_hashes.contains_key(DB_ENTRY) {
        return Err(AppError::bad_request("Backup sem banco de dados."));
    }

    validate_hashes(&mut archive, &manifest)?;

    let restore_root = std::env::temp_dir().join(format!("atendemente-restore-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&restore_root)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao preparar restauracao: {}", e)))?;

    let db_path = restore_root.join("atendemente.db");
    let db_bytes = read_zip_entry(&mut archive, DB_ENTRY)?;
    tokio::fs::write(&db_path, db_bytes)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao escrever banco restaurado: {}", e)))?;

    import_database(db, &db_path).await?;
    restore_storage(&mut archive, &manifest, &config.storage_dir.join(user_id)).await?;
    let _ = tokio::fs::remove_dir_all(&restore_root).await;

    // Re-encrypt PII if pepper differs
    if let Some(pepper_hex) = &manifest.pepper {
        let old_pepper_bytes = hex_decode(pepper_hex)?;
        if old_pepper_bytes.len() == 32 {
            let mut old_pepper = [0u8; 32];
            old_pepper.copy_from_slice(&old_pepper_bytes);
            crypto::reencrypt_all_pii(db, &old_pepper, user_id).await?;
        }
    }

    audit::write_audit_event(
        db,
        user_id,
        AuditAction::BackupRestored,
        "backup",
        None,
        serde_json::json!({"version": manifest.version, "entries": manifest.file_hashes.len(), "encrypted": backup_bytes.starts_with(ATND_MAGIC)}),
        Some("local-device"),
    )
    .await?;

    Ok(manifest)
}

async fn collect_files(
    root: &Path,
    current: &Path,
    user_id: &str,
    entries: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), AppError> {
    let mut stack = vec![current.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut read_dir = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao listar anexos: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| AppError::internal(format!("Erro ao ler anexo: {}", e)))?
        {
            let path = entry.path();
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| AppError::internal(format!("Erro ao ler metadados: {}", e)))?;
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| AppError::internal("Caminho de anexo invalido."))?;
                let zip_path = format!("storage/{}", normalize_zip_path(relative));
                    let bytes = tokio::fs::read(&path)
                        .await
                        .map_err(|e| AppError::internal(format!("Erro ao ler anexo: {}", e)))?;
                    let decrypted = match crypto::load_key(user_id) {
                        Ok(key) => crypto::decrypt_file(&bytes, &key).unwrap_or(bytes.clone()),
                        Err(_) => bytes,
                    };
                    entries.push((zip_path, decrypted));
            }
        }
    }
    Ok(())
}

fn normalize_zip_path(path: &Path) -> String {
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn read_manifest<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<BackupManifest, AppError> {
    let mut manifest_file = archive
        .by_name(MANIFEST_ENTRY)
        .map_err(|_| AppError::bad_request("Manifesto do backup nao encontrado."))?;
    let mut data = Vec::new();
    manifest_file
        .read_to_end(&mut data)
        .map_err(|e| AppError::bad_request(format!("Erro ao ler manifesto: {}", e)))?;
    serde_json::from_slice(&data)
        .map_err(|e| AppError::bad_request(format!("Manifesto invalido: {}", e)))
}

fn validate_hashes<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &BackupManifest,
) -> Result<(), AppError> {
    for (path, expected) in &manifest.file_hashes {
        if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
            return Err(AppError::bad_request("Backup contem caminho inseguro."));
        }
        let bytes = read_zip_entry(archive, path)?;
        let actual = sha256_hex(&bytes);
        if &actual != expected {
            return Err(AppError::bad_request("Hash de arquivo do backup nao confere."));
        }
    }
    Ok(())
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, AppError> {
    let mut file = archive
        .by_name(path)
        .map_err(|_| AppError::bad_request(format!("Arquivo ausente no backup: {}", path)))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| AppError::bad_request(format!("Erro ao ler backup: {}", e)))?;
    Ok(data)
}

async fn import_database(db: &SqlitePool, source_db_path: &Path) -> Result<(), AppError> {
    let path = sqlite_path_literal(source_db_path);
    let mut conn = db
        .acquire()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao abrir conexao de restauracao: {}", e)))?;
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao preparar banco: {}", e)))?;
    sqlx::query(&format!("ATTACH DATABASE '{}' AS backup_src", path))
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::bad_request(format!("Banco do backup invalido: {}", e)))?;

    let source_tables: Vec<String> = sqlx::query_scalar(
        r#"SELECT name FROM backup_src.sqlite_schema
        WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations'"#,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| AppError::bad_request(format!("Estrutura do banco do backup invalida: {}", e)))?;
    if !source_tables.iter().any(|t| t == "users") {
        let _ = sqlx::query("DETACH DATABASE backup_src").execute(&mut *conn).await;
        return Err(AppError::bad_request("Backup sem tabela principal de usuarios."));
    }

    let table_order = [
        "request_limits",
        "audit_logs",
        "record_files",
        "session_records",
        "payments",
        "appointments",
        "recurring_series",
        "patient_search_tokens",
        "patients",
        "users",
    ];
    let tables: Vec<&str> = table_order
        .iter()
        .copied()
        .filter(|table| source_tables.iter().any(|source| source == table))
        .collect();

    for table in &tables {
        sqlx::query(&format!("DELETE FROM {}", table))
            .execute(&mut *conn)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao limpar tabela {}: {}", table, e)))?;
    }
    for table in tables.iter().rev() {
        sqlx::query(&format!("INSERT INTO {} SELECT * FROM backup_src.{}", table, table))
            .execute(&mut *conn)
            .await
            .map_err(|e| AppError::bad_request(format!("Erro ao restaurar tabela {}: {}", table, e)))?;
    }

    sqlx::query("DETACH DATABASE backup_src")
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao finalizar restauracao: {}", e)))?;
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao reativar integridade: {}", e)))?;
    Ok(())
}

async fn restore_storage<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &BackupManifest,
    target_root: &Path,
) -> Result<(), AppError> {
    if target_root.exists() {
        tokio::fs::remove_dir_all(target_root)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao limpar anexos: {}", e)))?;
    }
    tokio::fs::create_dir_all(target_root)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao recriar anexos: {}", e)))?;

    for path in manifest.file_hashes.keys().filter(|p| p.starts_with("storage/")) {
        let relative = path.trim_start_matches("storage/");
        let target = safe_join(target_root, relative)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::internal(format!("Erro ao criar diretorio de anexo: {}", e)))?;
        }
        let bytes = read_zip_entry(archive, path)?;
        tokio::fs::write(&target, bytes)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao restaurar anexo: {}", e)))?;
    }
    Ok(())
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, AppError> {
    let candidate = Path::new(relative);
    if candidate.is_absolute()
        || relative.contains("..")
        || relative.contains('\\')
        || relative.contains(':')
    {
        return Err(AppError::bad_request("Backup contem caminho inseguro."));
    }
    Ok(root.join(candidate))
}

fn sqlite_path_literal(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").replace('\'', "''")
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, AppError> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::bad_request("Valor hexadecimal invalido no manifesto."))
}

// ─── Backup Config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BackupConfig {
    pub frequency: String,
    pub last_backup_at: Option<String>,
}

pub async fn get_backup_config(
    db: &SqlitePool,
    user_id: &str,
) -> Result<BackupConfig, AppError> {
    let config = sqlx::query_as::<_, BackupConfig>(
        r#"SELECT frequency, last_backup_at FROM backup_config WHERE user_id = ?"#,
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler config de backup: {}", e)))?;

    Ok(config.unwrap_or(BackupConfig {
        frequency: "never".to_string(),
        last_backup_at: None,
    }))
}

pub async fn set_backup_config(
    db: &SqlitePool,
    user_id: &str,
    frequency: &str,
) -> Result<(), AppError> {
    if !["never", "daily", "weekly"].contains(&frequency) {
        return Err(AppError::bad_request("Frequencia invalida. Use: never, daily, weekly."));
    }
    sqlx::query(
        r#"INSERT INTO backup_config (user_id, frequency, updated_at)
        VALUES (?, ?, datetime('now'))
        ON CONFLICT(user_id) DO UPDATE SET frequency = ?, updated_at = datetime('now')"#,
    )
    .bind(user_id)
    .bind(frequency)
    .bind(frequency)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao salvar config de backup: {}", e)))?;
    Ok(())
}

pub async fn touch_backup_timestamp(
    db: &SqlitePool,
    user_id: &str,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    sqlx::query(
        r#"INSERT INTO backup_config (user_id, frequency, last_backup_at, updated_at)
        VALUES (?, 'manual', ?, datetime('now'))
        ON CONFLICT(user_id) DO UPDATE SET last_backup_at = ?, updated_at = datetime('now')"#,
    )
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao atualizar timestamp de backup: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::config::AppConfig;
    use crate::db;

    async fn test_db(name: &str) -> (tempfile::TempDir, SqlitePool, AppConfig) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join(format!("{name}.db"));
        let db_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = db::init_database(&db_url).await.unwrap();
        let storage_dir = dir.path().join("uploads");
        tokio::fs::create_dir_all(&storage_dir).await.unwrap();
        let config = AppConfig {
            database_url: db_url,
            auth_database_url: String::new(),
            server_port: 3001,
            master_pepper: [0u8; 32],
            storage_dir,
            mobile_access_enabled: false,
        };
        (dir, pool, config)
    }

    async fn seed_user(db: &SqlitePool, user_id: &str) {
        sqlx::query(
            "INSERT INTO users (id, email, full_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind("test@example.com")
        .bind("Test User")
        .bind("2026-06-18T10:00:00")
        .bind("2026-06-18T10:00:00")
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn creates_backup_with_manifest_hashes_and_files() {
        let (_dir, db, config) = test_db("backup-create").await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;
        let attachment = config.storage_dir.join(user_id).join("sample.txt");
        tokio::fs::create_dir_all(attachment.parent().unwrap()).await.unwrap();
        tokio::fs::write(&attachment, b"attachment").await.unwrap();

        let backup = super::create_backup(&db, &config, user_id).await.unwrap();

        assert!(backup.file_name.starts_with("backup_"));
        assert!(backup.file_name.ends_with(".zip"));
        assert!(!backup.bytes.is_empty());
        assert!(backup.manifest.file_hashes.contains_key("database/atendemente.db"));
    }

    #[tokio::test]
    async fn restores_valid_backup_and_rejects_invalid_backup() {
        let (_dir, db, config) = test_db("backup-restore-source").await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;
        let backup = super::create_backup(&db, &config, user_id).await.unwrap();

        let (restore_dir, restore_db, restore_config) = test_db("backup-restore-target").await;
        super::restore_backup(&restore_db, &restore_config, user_id, &backup.bytes)
            .await
            .unwrap();

        let restored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&restore_db)
            .await
            .unwrap();
        assert_eq!(restored_count, 1);

        let invalid = super::restore_backup(&restore_db, &restore_config, user_id, b"not a zip").await;
        assert!(invalid.is_err());
        drop(restore_dir);
    }

    #[tokio::test]
    async fn creates_encrypted_backup_and_restores() {
        let (_dir, db, config) = test_db("backup-encrypted").await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;

        let backup = super::create_backup_with_password(&db, &config, user_id, Some("minha-senha-12c"))
            .await
            .unwrap();

        assert!(backup.file_name.ends_with(".atendemente"));
        assert!(backup.encrypted);
        assert!(backup.manifest.encrypted == Some(true));
        assert_eq!(backup.manifest.kdf, Some("argon2id".into()));
        assert!(backup.manifest.salt.is_some());
        // pepper field is present but might be None if no pepper is set globally
        // Verify format: ATND magic + salt + encrypted data
        assert!(backup.bytes.starts_with(b"ATND"));
        assert_eq!(&backup.bytes[4..20].len(), &16); // salt

        // Restore with wrong password should fail
        let (r_dir, r_db, r_cfg) = test_db("backup-encrypted-restore-fail").await;
        let result = super::restore_backup_with_password(
            &r_db, &r_cfg, user_id, &backup.bytes, Some("senha-errada"),
        )
        .await;
        assert!(result.is_err());
        drop(r_dir);

        // Restore with correct password should succeed
        let (restore_dir, restore_db, restore_config) = test_db("backup-encrypted-restore").await;
        let manifest = super::restore_backup_with_password(
            &restore_db, &restore_config, user_id, &backup.bytes, Some("minha-senha-12c"),
        )
        .await
        .unwrap();

        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.user_id, user_id);

        let restored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&restore_db)
            .await
            .unwrap();
        assert_eq!(restored_count, 1);
        drop(restore_dir);
    }

    #[tokio::test]
    async fn encrypted_backup_needs_password_on_restore() {
        let (_dir, db, config) = test_db("backup-needs-password").await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;

        let backup = super::create_backup_with_password(&db, &config, user_id, Some("minha-senha-12c"))
            .await
            .unwrap();

        // Trying to restore without password should fail
        let (r_dir, r_db, r_cfg) = test_db("backup-needs-password-restore").await;
        let result = super::restore_backup(&r_db, &r_cfg, user_id, &backup.bytes).await;
        assert!(result.is_err());
        drop(r_dir);
    }
}
