use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Acquire, SqlitePool};
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
/// Minimum length for a user-supplied backup password. Only enforced when
/// *creating* a backup — older backups with weaker passwords still restore.
const MIN_BACKUP_PASSWORD_LEN: usize = 8;

/// Removes a temporary file on drop, including on early `?` returns.
/// Backup temporaries hold the database in plaintext, so they must never
/// be left behind on an error path.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Removes a temporary directory (recursively) on drop.
struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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
    if let Some(pass) = password {
        if pass.chars().count() < MIN_BACKUP_PASSWORD_LEN {
            return Err(AppError::bad_request(format!(
                "A senha do backup deve ter no mínimo {} caracteres.",
                MIN_BACKUP_PASSWORD_LEN
            )));
        }
    }

    let created_at = chrono::Utc::now();
    let is_encrypted = password.is_some();
    let ext = if is_encrypted { "atendemente" } else { "zip" };
    let file_name = format!("backup_{}.{}", created_at.format("%Y%m%d_%H%M%S"), ext);
    let temp_db = config
        .temp_dir()?
        .join(format!("atendemente-backup-{}.db", Uuid::new_v4()));
    // Guard first: the file exists from the moment VACUUM INTO runs, so every
    // error path below must still delete it.
    let _temp_guard = TempFileGuard(temp_db.clone());
    let temp_db_sql = sqlite_path_literal(&temp_db);
    sqlx::query(&format!("VACUUM INTO '{}'", temp_db_sql))
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao gerar copia consistente do banco: {}", e)))?;

    let db_bytes = tokio::fs::read(&temp_db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao ler banco temporario: {}", e)))?;

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
        // pepper_hex is only stored inside the encrypted ZIP (protected by password)
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

    let restore_root = config
        .temp_dir()?
        .join(format!("atendemente-restore-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&restore_root)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao preparar restauracao: {}", e)))?;
    // Holds the backup's database in plaintext — remove it even if the import fails.
    let _restore_guard = TempDirGuard(restore_root.clone());

    let db_path = restore_root.join("atendemente.db");
    let db_bytes = read_zip_entry(&mut archive, DB_ENTRY)?;
    tokio::fs::write(&db_path, db_bytes)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao escrever banco restaurado: {}", e)))?;

    // Tudo que pode falhar acontece sobre a copia em area temporaria, ANTES de
    // encostar no banco e nos anexos reais.
    //
    // A ordem anterior era import_database -> restore_storage -> re-cifra, e a
    // re-cifra abortava com "no such column: key_version". O usuario recebia
    // "erro interno" e concluia que nada tinha acontecido, mas o banco ja havia
    // sido substituido: 11 de 12 pacientes ficavam ilegiveis, sem sequer um
    // registro em auditoria, porque o evento fica depois do ponto que abortava.
    // E o estado local anterior — a unica copia — ja tinha sido sobrescrito.
    //
    // O cenario nao e exotico: e "reinstalei o Windows e copiei minha pasta de
    // dados". Os bancos sao arquivos e sobrevivem; o cofre de credenciais nao,
    // entao o pepper e regenerado com o mesmo user_id.
    let old_pepper = match &manifest.pepper {
        Some(hex) => {
            let bytes = hex_decode(hex)?;
            if bytes.len() == 32 {
                let mut p = [0u8; 32];
                p.copy_from_slice(&bytes);
                Some(p)
            } else {
                None
            }
        }
        None => None,
    };

    if let Some(old_pepper) = old_pepper {
        // O banco dentro do backup tem o schema da versao que o gerou, que pode
        // nao ter `patients.key_version` (criada na migration 14). Subir as
        // migrations na copia primeiro deixa o schema atual antes da conversao,
        // e tambem alinha as colunas que `import_database` mapeia por nome.
        let staged_url = format!("sqlite:{}?mode=rw", db_path.display());
        let staged = crate::db::init_database(&staged_url)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao preparar copia do backup: {}", e)))?;
        let report = crypto::reencrypt_all_pii(&staged, &old_pepper, user_id).await;
        staged.close().await;
        let report = report?;
        if report.patients > 0 || report.session_records > 0 {
            tracing::info!(
                "[Backup] Pepper do backup difere do atual; re-cifrados {} paciente(s) e {} \
                 prontuario(s) na copia antes de importar.",
                report.patients,
                report.session_records
            );
        }
    }

    // Anexos: extraidos e convertidos numa area de staging, e so entao trocados
    // pelos reais. O filesystem nao participa da transacao do SQLite, entao esta
    // e a unica forma de nao destruir os anexos existentes por causa de uma
    // falha de conversao.
    let staged_storage = restore_root.join("storage");
    stage_storage(&mut archive, &manifest, &staged_storage, old_pepper, user_id).await?;

    import_database(db, &db_path).await?;
    swap_storage(&staged_storage, &config.storage_dir.join(user_id)).await?;
    rewrite_storage_paths(db, config, user_id).await?;

    // A version-1 backup stores patient PII in the legacy plaintext columns.
    // Encrypt it now that the rows are in place; best-effort so a restore never
    // fails over it.
    if let Err(e) = crate::features::patients::migrate_plaintext_pii(db, user_id).await {
        tracing::warn!("[Backup] Falha ao migrar PII em texto claro apos restauracao: {}", e);
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
                        Ok(key) => match crypto::decrypt_file(&bytes, &key) {
                            Ok(d) => d,
                            Err(e) => {
                                tracing::warn!("[Backup] Falha ao descriptografar anexo {}: {}", zip_path, e);
                                bytes.clone()
                            }
                        },
                        Err(e) => {
                            tracing::warn!("[Backup] Chave indisponivel para usuario {}: {}", user_id, e);
                            bytes
                        }
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

/// Tables restored from a backup, ordered child → parent (delete order).
/// Only names from this list are ever interpolated into SQL.
const RESTORE_TABLE_ORDER: &[&str] = &[
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

/// A column of a SQLite table, as reported by `PRAGMA table_info`.
struct ColumnInfo {
    name: String,
    not_null: bool,
    has_default: bool,
    is_pk: bool,
}

async fn table_columns(
    conn: &mut sqlx::SqliteConnection,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, AppError> {
    // `schema` and `table` are always crate constants ("main"/"backup_src" and
    // RESTORE_TABLE_ORDER entries), never values taken from the backup file.
    let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as(&format!("PRAGMA {}.table_info({})", schema, table))
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                AppError::internal(format!("Erro ao inspecionar tabela {}: {}", table, e))
            })?;

    Ok(rows
        .into_iter()
        .map(|(_cid, name, _ty, notnull, dflt, pk)| ColumnInfo {
            name,
            not_null: notnull != 0,
            has_default: dflt.is_some(),
            is_pk: pk != 0,
        })
        .collect())
}

/// Maps a destination column to a SELECT expression over the backup's table.
///
/// Matching is by column *name*, never by position, so a backup written by an
/// older version restores correctly even when columns were added in between or
/// a table was rebuilt in a different order (which is what migration 11 did to
/// `audit_logs`). Columns missing from the backup are omitted so the
/// destination's DEFAULT applies.
fn select_expr_for(table: &str, dest_col: &str, src_cols: &[String]) -> Option<String> {
    let has = |c: &str| src_cols.iter().any(|s| s == c);

    if has(dest_col) {
        return Some(format!("\"{}\"", dest_col));
    }

    // Legacy schema fallbacks. `audit_logs` was rebuilt in migration
    // 20240101000011 with renamed columns; a version-1 backup still has the
    // original names. Mirror the same mapping that migration used.
    match (table, dest_col) {
        ("audit_logs", "timestamp") if has("created_at") => {
            Some("COALESCE(\"created_at\", datetime('now'))".into())
        }
        ("audit_logs", "details") if has("metadata") => {
            Some("COALESCE(\"metadata\", '{}')".into())
        }
        ("audit_logs", "ip_or_device") if has("ip_address") && has("user_agent") => {
            Some("COALESCE(\"ip_address\", \"user_agent\")".into())
        }
        ("audit_logs", "ip_or_device") if has("ip_address") => Some("\"ip_address\"".into()),
        ("audit_logs", "ip_or_device") if has("user_agent") => Some("\"user_agent\"".into()),
        _ => None,
    }
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
        .map_err(|e| {
            // Nothing was attached, but foreign_keys is off on this pooled
            // connection — restore it before handing it back.
            AppError::bad_request(format!("Banco do backup invalido: {}", e))
        })?;

    // From here on the connection holds non-default state (attached DB +
    // foreign_keys off). Run the import separately so cleanup always happens,
    // otherwise the connection returns to the pool with integrity checks
    // disabled for every later request that picks it up.
    let result = import_attached_database(&mut conn).await;

    if let Err(e) = sqlx::query("DETACH DATABASE backup_src")
        .execute(&mut *conn)
        .await
    {
        tracing::error!("[Backup] Falha ao desanexar banco de restauracao: {}", e);
    }
    if let Err(e) = sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut *conn)
        .await
    {
        tracing::error!("[Backup] Falha ao reativar foreign_keys: {}", e);
    }

    result
}

async fn import_attached_database(conn: &mut sqlx::SqliteConnection) -> Result<(), AppError> {
    let source_tables: Vec<String> = sqlx::query_scalar(
        r#"SELECT name FROM backup_src.sqlite_schema
        WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations'"#,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| AppError::bad_request(format!("Estrutura do banco do backup invalida: {}", e)))?;

    if !source_tables.iter().any(|t| t == "users") {
        return Err(AppError::bad_request("Backup sem tabela principal de usuarios."));
    }

    let tables: Vec<&str> = RESTORE_TABLE_ORDER
        .iter()
        .copied()
        .filter(|table| source_tables.iter().any(|source| source == table))
        .collect();

    // Resolve the column mapping for every table before touching any data, so a
    // schema we cannot map safely aborts the restore instead of half-applying it.
    let mut plans: Vec<(&str, String, String)> = Vec::new();
    for table in &tables {
        let src_cols: Vec<String> = table_columns(&mut *conn, "backup_src", table)
            .await?
            .into_iter()
            .map(|c| c.name)
            .collect();
        let dest_cols = table_columns(&mut *conn, "main", table).await?;

        let mut names = Vec::new();
        let mut exprs = Vec::new();
        for dest in &dest_cols {
            match select_expr_for(table, &dest.name, &src_cols) {
                Some(expr) => {
                    names.push(format!("\"{}\"", dest.name));
                    exprs.push(expr);
                }
                None => {
                    if dest.not_null && !dest.has_default && !dest.is_pk {
                        return Err(AppError::bad_request(format!(
                            "Backup incompativel: a coluna obrigatoria '{}' da tabela '{}' \
                             nao existe no backup e nao possui valor padrao.",
                            dest.name, table
                        )));
                    }
                    // Omitted: the destination DEFAULT (or NULL) applies.
                }
            }
        }

        if names.is_empty() {
            return Err(AppError::bad_request(format!(
                "Backup incompativel: nenhuma coluna da tabela '{}' pôde ser mapeada.",
                table
            )));
        }

        plans.push((table, names.join(", "), exprs.join(", ")));
    }

    let mut tx = conn
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao iniciar transacao: {}", e)))?;

    for table in &tables {
        sqlx::query(&format!("DELETE FROM \"{}\"", table))
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao limpar tabela {}: {}", table, e)))?;
    }
    // Parent → child so foreign keys line up even if enforcement is re-enabled.
    for (table, names, exprs) in plans.iter().rev() {
        sqlx::query(&format!(
            "INSERT INTO \"{}\" ({}) SELECT {} FROM backup_src.\"{}\"",
            table, names, exprs, table
        ))
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::bad_request(format!("Erro ao restaurar tabela {}: {}", table, e)))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao confirmar restauracao: {}", e)))?;

    Ok(())
}

/// Extrai os anexos do bundle para uma area de staging, convertendo a chave
/// quando o pepper do backup difere do atual.
///
/// Nada dos anexos reais e tocado aqui: se a conversao falhar, quem chama
/// retorna erro e o que estava em disco continua intacto.
async fn stage_storage<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &BackupManifest,
    staging_root: &Path,
    old_pepper: Option<[u8; 32]>,
    user_id: &str,
) -> Result<(), AppError> {
    tokio::fs::create_dir_all(staging_root)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao preparar anexos: {}", e)))?;

    let mut convertidos = 0usize;
    for path in manifest.file_hashes.keys().filter(|p| p.starts_with("storage/")) {
        let relative = path.trim_start_matches("storage/");
        let target = safe_join(staging_root, relative)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::internal(format!("Erro ao criar diretorio de anexo: {}", e)))?;
        }
        let bytes = read_zip_entry(archive, path)?;
        let bytes = match old_pepper {
            Some(p) => {
                let out = crypto::reencrypt_file_bytes(&bytes, &p, user_id).map_err(|_| {
                    AppError::bad_request(format!(
                        "Nao foi possivel converter o anexo '{}' para a chave desta maquina. \
                         Nada foi alterado.",
                        relative
                    ))
                })?;
                convertidos += 1;
                out
            }
            None => bytes,
        };
        tokio::fs::write(&target, bytes)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao preparar anexo: {}", e)))?;
    }
    if convertidos > 0 {
        tracing::info!("[Backup] {} anexo(s) re-cifrados na area de staging.", convertidos);
    }
    Ok(())
}

/// Reaponta `record_files.storage_path` para o layout desta maquina.
///
/// `download_file` le o caminho **absoluto** gravado no momento do upload
/// (`files.rs`, `Path::new(&file.storage_path)`), e `import_database` copia essa
/// coluna verbatim. Sem esta reescrita, restaurar num computador diferente — ou
/// so numa conta de usuario diferente, ou com outro `STORAGE_DIR` — deixa
/// **todos** os anexos inalcancaveis: as linhas apontam para um caminho que nao
/// existe ali, ainda que os arquivos tenham sido restaurados corretamente.
///
/// O nome do arquivo e preservado, porque e ele que veio no ZIP; so a parte da
/// raiz e trocada. O layout e deterministico:
/// `<storage_dir>/<user_id>/<patient_id>/<appointment_id>/<uuid>.<ext>`.
async fn rewrite_storage_paths(
    db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
) -> Result<(), AppError> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        r#"SELECT id, patient_id, appointment_id, storage_path
           FROM record_files WHERE user_id = ?"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler anexos: {}", e)))?;

    let mut ajustados = 0usize;
    for (id, patient_id, appointment_id, antigo) in &rows {
        // O separador pode ser o da maquina de origem, que nao e o desta.
        let nome = antigo.rsplit(['/', '\\']).next().unwrap_or_default();
        if nome.is_empty() {
            continue;
        }
        let novo = config
            .storage_dir
            .join(user_id)
            .join(patient_id)
            .join(appointment_id)
            .join(nome);
        let novo = novo.to_string_lossy().to_string();
        if &novo == antigo {
            continue;
        }
        sqlx::query("UPDATE record_files SET storage_path = ? WHERE id = ?")
            .bind(&novo)
            .bind(id)
            .execute(db)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao reapontar anexo: {}", e)))?;
        ajustados += 1;
    }
    if ajustados > 0 {
        tracing::info!(
            "[Backup] {} anexo(s) reapontados para o layout desta maquina.",
            ajustados
        );
    }
    Ok(())
}

/// Troca o diretorio de anexos real pelo que foi preparado no staging.
///
/// O antigo e movido para o lado antes de o novo entrar, e so removido quando o
/// novo esta no lugar — assim uma falha no meio nao deixa o usuario sem anexo
/// nenhum.
async fn swap_storage(staging_root: &Path, target_root: &Path) -> Result<(), AppError> {
    if let Some(parent) = target_root.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao preparar anexos: {}", e)))?;
    }

    let backup_side = target_root.with_extension(format!("pre-restore-{}", Uuid::new_v4()));
    let had_previous = target_root.exists();
    if had_previous {
        tokio::fs::rename(target_root, &backup_side)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao reservar anexos atuais: {}", e)))?;
    }

    if let Err(e) = tokio::fs::rename(staging_root, target_root).await {
        // Falhou ao instalar o novo: devolve o antigo para o lugar.
        if had_previous {
            let _ = tokio::fs::rename(&backup_side, target_root).await;
        }
        return Err(AppError::internal(format!("Erro ao restaurar anexos: {}", e)));
    }

    if had_previous {
        let _ = tokio::fs::remove_dir_all(&backup_side).await;
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
    let invalid = || AppError::bad_request("Valor hexadecimal invalido no manifesto.");
    // Values reach here straight from an untrusted manifest: reject anything
    // that is not an even number of ASCII hex digits before slicing, otherwise
    // `&hex[i..i + 2]` panics on odd lengths or multi-byte characters.
    let bytes = hex.as_bytes();
    if bytes.len() % 2 != 0 || !bytes.iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())
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
        VALUES (?, 'never', ?, datetime('now'))
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
            data_dir: dir.path().join("data"),
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

    // ── Portao de compatibilidade com a v1.0.19 ─────────────────────────────
    //
    // Restaura os bundles congelados em tests/fixtures/ e confere a PII
    // decifrada contra o golden, campo por campo. Roda com um pepper diferente
    // do que gerou o fixture, que e o cenario "reinstalei o Windows e copiei
    // minha pasta de dados": os bancos sao arquivos e sobrevivem, o cofre de
    // credenciais nao, entao o pepper e regenerado com o mesmo user_id.
    //
    // Antes da correcao este teste falhava com "no such column: key_version" —
    // e o pior nao era a falha, era o estado que ela deixava: banco e anexos ja
    // substituidos, 11 de 12 pacientes ilegiveis, nada em auditoria.
    //
    // Ver tests/fixtures/README.md.
    const FIXTURE_PEPPER: &[u8; 32] = b"fixture-pepper-de-teste-32bytes!";
    const FIXTURE_BACKUP_PASSWORD: &str = "SenhaDoBackupFixture#2026";

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn golden() -> serde_json::Value {
        let raw = std::fs::read_to_string(fixture_path("backup-v1.0.19.expected.json"))
            .expect("golden do fixture ausente");
        serde_json::from_str(&raw).expect("golden invalido")
    }

    async fn restaura_fixture_e_confere(bundle: &str) {
        // `set_pepper` e OnceLock: se outro teste ja definiu, o dele vale. Nao
        // importa qual seja, desde que difira do pepper do fixture — e nenhum
        // dos usados nos testes ([3;32], [7;32], [9;32], [0xab;32]) coincide com
        // texto ASCII. O assert abaixo garante que o caminho de conversao roda.
        crate::crypto::set_pepper(&[7u8; 32]);
        let atual = crate::crypto::get_pepper().expect("pepper nao inicializado");
        assert_ne!(
            atual, FIXTURE_PEPPER,
            "o teste precisa de um pepper diferente do que gerou o fixture"
        );

        let g = golden();
        let user_id = g["user_id"].as_str().unwrap().to_string();

        let (_dir, db, config) = test_db("fixture-v1019").await;
        seed_user(&db, &user_id).await;

        let bytes = std::fs::read(fixture_path(bundle)).expect("bundle do fixture ausente");
        super::restore_backup_with_password(
            &db,
            &config,
            &user_id,
            &bytes,
            Some(FIXTURE_BACKUP_PASSWORD),
        )
        .await
        .unwrap_or_else(|e| panic!("restore do fixture {} falhou: {:?}", bundle, e));

        // A chave desta maquina precisa estar carregada para decifrar a PII que
        // acabou de ser convertida.
        crate::crypto::init_user_crypto(&user_id).unwrap();

        let esperados = g["pacientes"].as_array().unwrap();
        assert_eq!(esperados.len(), 12, "o fixture deve ter 12 pacientes");

        for p in esperados {
            let id = p["id"].as_str().unwrap();
            let caso = p["caso"].as_str().unwrap();
            let e = &p["esperado"];
            let lido = crate::features::patients::get_patient_detail(&db, &user_id, id)
                .await
                .unwrap_or_else(|err| panic!("paciente ilegivel [{}]: {:?}", caso, err));

            let s = |v: &serde_json::Value| v.as_str().map(|x| x.to_string());
            assert_eq!(Some(lido.full_name.clone()), s(&e["full_name"]), "full_name [{}]", caso);
            assert_eq!(lido.chart_number, s(&e["chart_number"]), "chart_number [{}]", caso);
            assert_eq!(lido.phone, s(&e["phone"]), "phone [{}]", caso);
            assert_eq!(lido.email, s(&e["email"]), "email [{}]", caso);
            assert_eq!(lido.birth_date, s(&e["birth_date"]), "birth_date [{}]", caso);
            assert_eq!(lido.emergency_phone, s(&e["emergency_phone"]), "emergency_phone [{}]", caso);
            assert_eq!(lido.health_history, s(&e["health_history"]), "health_history [{}]", caso);
            assert_eq!(lido.medications_in_use, s(&e["medications_in_use"]), "medications [{}]", caso);
            assert_eq!(lido.admin_notes, s(&e["admin_notes"]), "admin_notes [{}]", caso);
            assert_eq!(Some(lido.status.clone()), s(&e["status"]), "status [{}]", caso);
        }

        // Os prontuarios de sessao tambem precisam abrir: eram o artefato que a
        // re-cifra antiga nao cobria, e por isso ficavam silenciosamente ilegiveis.
        for r in g["prontuarios"].as_array().unwrap() {
            let ap = r["appointment_id"].as_str().unwrap();
            let esperado = r["content"].as_str().unwrap();
            let lido = crate::features::records::get_session_record(&db, &user_id, ap)
                .await
                .unwrap_or_else(|e| panic!("prontuario de {} ilegivel: {:?}", ap, e));
            assert_eq!(lido, esperado, "conteudo do prontuario de {}", ap);
        }

        // E os anexos: devem existir em disco e abrir com a chave desta maquina.
        let key = crate::crypto::load_key(&user_id).unwrap();
        for a in g["anexos"].as_array().unwrap() {
            let nome = a["original_name"].as_str().unwrap();
            let esperado = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                a["conteudo_b64"].as_str().unwrap(),
            )
            .unwrap();
            let fid = a["file_id"].as_str().unwrap();
            // Le pelo `storage_path` do banco, e nao varrendo o diretorio: e
            // exatamente o que `download_file` faz, entao o teste tambem prova
            // que o download funciona depois do restore.
            let caminho: String =
                sqlx::query_scalar("SELECT storage_path FROM record_files WHERE id = ?")
                    .bind(fid)
                    .fetch_one(&db)
                    .await
                    .unwrap_or_else(|e| panic!("anexo {} ausente no banco: {:?}", nome, e));
            let bytes = std::fs::read(&caminho).unwrap_or_else(|e| {
                panic!("anexo {} nao esta em {}: {:?}", nome, caminho, e)
            });
            let claro = crate::crypto::decrypt_file(&bytes, &key)
                .unwrap_or_else(|e| panic!("anexo {} nao decifra: {:?}", nome, e));
            assert_eq!(claro, esperado, "conteudo do anexo {}", nome);
        }
    }


    #[tokio::test]
    async fn restaura_fixture_v1019_agendado_com_pepper_diferente() {
        // Bundle feito SEM sessao de cripto: o paciente 12 esta no formato v1
        // (PII em colunas de texto claro) e os anexos entraram cifrados no ZIP.
        restaura_fixture_e_confere("backup-v1.0.19-agendado.atendemente").await;
    }

    #[tokio::test]
    async fn restaura_fixture_v1019_interativo_com_pepper_diferente() {
        // Bundle feito APOS login: o paciente 12 ja migrou para o blob cifrado e
        // os anexos entraram em texto claro no ZIP.
        restaura_fixture_e_confere("backup-v1.0.19-interativo.atendemente").await;
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

    /// The pre-migration-11 schema, as a version-1 backup would contain it.
    /// `audit_logs` here is the *original* layout: same column count as today's
    /// but a different order and different names, which is exactly what made
    /// `INSERT ... SELECT *` shift values into the wrong columns.
    const LEGACY_SCHEMA: &str = r#"
        CREATE TABLE users (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            full_name TEXT,
            two_factor_enabled INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE patients (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            full_name TEXT NOT NULL,
            phone TEXT,
            email TEXT,
            birth_date TEXT,
            admin_notes TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            health_history TEXT,
            medications_in_use TEXT,
            emergency_phone TEXT
        );
        CREATE TABLE appointments (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            patient_id TEXT NOT NULL,
            starts_at TEXT NOT NULL,
            ends_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'scheduled',
            session_price_cents INTEGER NOT NULL DEFAULT 0,
            quick_notes TEXT,
            cancel_reason TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT
        );
        CREATE TABLE record_files (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            patient_id TEXT NOT NULL,
            appointment_id TEXT NOT NULL,
            storage_path TEXT NOT NULL,
            original_name TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            byte_size INTEGER NOT NULL,
            uploaded_at TEXT NOT NULL,
            deleted_at TEXT
        );
        CREATE TABLE audit_logs (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            action TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT,
            ip_address TEXT,
            user_agent TEXT,
            metadata TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        );
    "#;

    /// Builds an unencrypted version-1 backup ZIP around a legacy-schema database.
    async fn build_legacy_backup(dir: &std::path::Path, user_id: &str) -> Vec<u8> {
        use std::io::Write;

        let legacy_db_path = dir.join("legacy-source.db");
        let url = format!("sqlite:{}?mode=rwc", legacy_db_path.to_string_lossy());
        let legacy = sqlx::SqlitePool::connect(&url).await.unwrap();

        for stmt in LEGACY_SCHEMA.split(';').filter(|s| !s.trim().is_empty()) {
            sqlx::query(stmt).execute(&legacy).await.unwrap();
        }

        sqlx::query(
            "INSERT INTO users (id, email, full_name, created_at, updated_at) \
             VALUES (?, 'legado@test.com', 'Usuario Legado', '2024-01-01T10:00:00', '2024-01-01T10:00:00')",
        )
        .bind(user_id)
        .execute(&legacy)
        .await
        .unwrap();

        // Plaintext PII, the way version 1 stored it.
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, phone, email, birth_date, \
             admin_notes, created_at, updated_at) \
             VALUES ('pac-legado', ?, 'Paciente Legado', '11999998888', 'pac@test.com', \
             '1990-05-02', 'nota administrativa', '2024-01-01T10:00:00', '2024-01-01T10:00:00')",
        )
        .bind(user_id)
        .execute(&legacy)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO appointments (id, user_id, patient_id, starts_at, ends_at, \
             created_at, updated_at) \
             VALUES ('apt-legado', ?, 'pac-legado', '2024-02-01T09:00:00', '2024-02-01T10:00:00', \
             '2024-01-01T10:00:00', '2024-01-01T10:00:00')",
        )
        .bind(user_id)
        .execute(&legacy)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO record_files (id, user_id, patient_id, appointment_id, storage_path, \
             original_name, mime_type, byte_size, uploaded_at) \
             VALUES ('arq-legado', ?, 'pac-legado', 'apt-legado', '/tmp/x.pdf', 'laudo.pdf', \
             'application/pdf', 100, '2024-01-01T10:00:00')",
        )
        .bind(user_id)
        .execute(&legacy)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, entity_type, entity_id, ip_address, \
             metadata, created_at) \
             VALUES ('aud-legado', ?, 'file_download', 'record_file', 'arq-legado', \
             'local-device', '{\"k\":1}', '2024-01-03T08:30:00')",
        )
        .bind(user_id)
        .execute(&legacy)
        .await
        .unwrap();

        legacy.close().await;

        let db_bytes = tokio::fs::read(&legacy_db_path).await.unwrap();

        let mut file_hashes = std::collections::BTreeMap::new();
        file_hashes.insert(super::DB_ENTRY.to_string(), super::sha256_hex(&db_bytes));

        let manifest = super::BackupManifest {
            version: super::BACKUP_VERSION_LEGACY,
            created_at: "2024-01-03T08:30:00.000Z".into(),
            user_id: user_id.to_string(),
            app_version: "1.0.0".into(),
            file_hashes,
            encrypted: None,
            kdf: None,
            salt: None,
            pepper: None,
            pepper_fingerprint: None,
        };

        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(super::MANIFEST_ENTRY, options).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        zip.start_file(super::DB_ENTRY, options).unwrap();
        zip.write_all(&db_bytes).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn restores_legacy_v1_backup_matching_columns_by_name() {
        let (dir, db, config) = test_db("backup-legacy-v1").await;
        let user_id = "550e8400-e29b-41d4-a716-4466554400aa";
        crate::crypto::set_pepper(&[7u8; 32]);
        crate::crypto::init_user_crypto(user_id).unwrap();

        let bytes = build_legacy_backup(dir.path(), user_id).await;

        let manifest = super::restore_backup(&db, &config, user_id, &bytes)
            .await
            .expect("um backup da versao 1 deve restaurar");
        assert_eq!(manifest.version, 1);

        // audit_logs was rebuilt with a different column order in migration 11.
        // Matching by name keeps each value in its own column instead of
        // shifting user_id into timestamp and action into user_id.
        let (aud_user, aud_action, aud_entity, aud_ts, aud_details, aud_device): (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT user_id, action, entity_type, timestamp, details, ip_or_device \
             FROM audit_logs WHERE id = 'aud-legado'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(aud_user, user_id, "user_id não deve escorregar de coluna");
        assert_eq!(aud_action, "file_download");
        assert_eq!(aud_entity, "record_file");
        assert_eq!(aud_ts, "2024-01-03T08:30:00", "timestamp vem de created_at");
        assert_eq!(aud_details, "{\"k\":1}", "details vem de metadata");
        assert_eq!(aud_device.as_deref(), Some("local-device"));

        // Columns the legacy schema lacks fall back to the current DEFAULT.
        let status: String = sqlx::query_scalar("SELECT status FROM patients WHERE id = 'pac-legado'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(status, "active");

        let confirmation: String = sqlx::query_scalar(
            "SELECT confirmation_status FROM appointments WHERE id = 'apt-legado'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(confirmation, "unconfirmed");

        let kind: String =
            sqlx::query_scalar("SELECT kind FROM record_files WHERE id = 'arq-legado'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(kind, "session_attachment");

        // Restoring also migrates the legacy plaintext PII into the encrypted blob.
        let (phone, admin_notes, pii): (Option<String>, Option<String>, Option<String>) =
            sqlx::query_as(
                "SELECT phone, admin_notes, pii_encrypted FROM patients WHERE id = 'pac-legado'",
            )
            .fetch_one(&db)
            .await
            .unwrap();
        assert!(phone.is_none(), "telefone em texto claro deve ser removido");
        assert!(admin_notes.is_none(), "notas em texto claro devem ser removidas");
        assert!(pii.is_some(), "PII deve estar cifrada");

        // ...and the data is still readable through the normal read path.
        let patient = crate::features::patients::get_patient_detail(&db, user_id, "pac-legado")
            .await
            .unwrap();
        assert_eq!(patient.phone.as_deref(), Some("11999998888"));
        assert_eq!(patient.email.as_deref(), Some("pac@test.com"));
        assert_eq!(patient.birth_date.as_deref(), Some("1990-05-02"));
        assert_eq!(patient.admin_notes.as_deref(), Some("nota administrativa"));
        drop(dir);
    }

    #[tokio::test]
    async fn rejects_manifest_with_malformed_pepper_instead_of_panicking() {
        assert!(super::hex_decode("abc").is_err(), "hex de tamanho impar");
        assert!(super::hex_decode("zz").is_err(), "digito nao-hex");
        assert!(super::hex_decode("çç").is_err(), "byte multi-byte");
        assert_eq!(super::hex_decode("00ff").unwrap(), vec![0x00, 0xff]);
    }

    #[tokio::test]
    async fn rejects_short_backup_password_on_create() {
        let (_dir, db, config) = test_db("backup-weak-password").await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;

        assert!(
            super::create_backup_with_password(&db, &config, user_id, Some("123"))
                .await
                .is_err(),
            "senha curta deve ser rejeitada"
        );
        assert!(
            super::create_backup_with_password(&db, &config, user_id, Some(""))
                .await
                .is_err(),
            "senha vazia deve ser rejeitada"
        );
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
