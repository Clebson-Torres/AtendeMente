use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use keyring::Entry;
use rand::RngCore;

const KEYCHAIN_SERVICE: &str = "atendemente";
const KEYCHAIN_ACCOUNT: &str = "master_pepper";
const KEYCHAIN_BACKUP_PASSWORD: &str = "backup_password";
pub const MAX_UPLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;
/// Matches the minimum the Settings UI enforces for manual backups.
pub const MIN_AUTO_BACKUP_PASSWORD_LEN: usize = 12;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub auth_database_url: String,
    pub server_port: u16,
    pub master_pepper: [u8; 32],
    pub storage_dir: PathBuf,
    /// Parent directory of the per-user databases (`<data_dir>/<user_id>/atendemente.db`).
    pub data_dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ConfigFile {
    master_pepper: Option<String>,
    /// Deprecated. The "mobile access" feature was removed; this field is only
    /// still read so an installation that had it enabled can be detected once
    /// and cleaned up (see `cleanup_legacy_mobile_access`). It is never honored
    /// for binding — the server is loopback-only.
    pub mobile_access_enabled: Option<bool>,
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("atendemente")
        .join("config.toml")
}

pub fn load_config_file() -> Result<ConfigFile, Box<dyn std::error::Error>> {
    let path = config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let cfg: ConfigFile = toml::from_str(&content)?;
        Ok(cfg)
    } else {
        Ok(ConfigFile::default())
    }
}

async fn save_config_file_async(cfg: &ConfigFile) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = toml::to_string(cfg)?;
    tokio::fs::write(&path, content).await?;
    Ok(())
}

/// Clears the deprecated `mobile_access_enabled` flag after the one-shot cleanup,
/// so the firewall removal is not retried on every start.
pub async fn clear_legacy_mobile_access_flag() {
    let mut cfg = load_config_file().unwrap_or_default();
    cfg.mobile_access_enabled = None;
    if let Err(e) = save_config_file_async(&cfg).await {
        tracing::error!("[Config] Falha ao limpar flag legada de acesso mobile: {}", e);
    }
}

// ─── Automatic backup password ───────────────────────────────────────────────
//
// Scheduled backups run unattended, so the password has to be stored somewhere
// the process can read. It lives in the OS keyring (same store as the master
// pepper), never in the config file and never in the database.
//
// It is deliberately a *user-chosen password* and not a key derived from the
// pepper: a backup encrypted with machine-local key material would be
// unrecoverable exactly when it is needed most — after the machine is lost or
// wiped. The user must be able to restore on a new machine, which means they
// have to know the secret.

/// The password used to encrypt scheduled backups, if the user configured one.
pub fn load_backup_password() -> Option<String> {
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_BACKUP_PASSWORD)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .filter(|p| !p.is_empty())
}

pub fn save_backup_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_AUTO_BACKUP_PASSWORD_LEN {
        return Err(format!(
            "A senha dos backups automaticos deve ter no minimo {} caracteres.",
            MIN_AUTO_BACKUP_PASSWORD_LEN
        ));
    }
    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_BACKUP_PASSWORD)
        .map_err(|e| format!("Cofre de credenciais indisponivel: {}", e))?;
    entry
        .set_password(password)
        .map_err(|e| format!("Erro ao guardar a senha no cofre: {}", e))
}

pub fn delete_backup_password() -> Result<(), String> {
    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_BACKUP_PASSWORD)
        .map_err(|e| format!("Cofre de credenciais indisponivel: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        // Already absent is the desired end state.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Erro ao remover a senha do cofre: {}", e)),
    }
}

fn load_pepper_from_keychain() -> Option<[u8; 32]> {
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .and_then(|password| {
            BASE64.decode(&password).ok().and_then(|bytes| {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    Some(key)
                } else {
                    None
                }
            })
        })
}

fn save_pepper_to_keychain(pepper: &[u8; 32]) -> bool {
    let encoded = BASE64.encode(pepper);
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()
        .and_then(|entry| entry.set_password(&encoded).ok())
        .is_some()
}

fn load_or_generate_pepper() -> [u8; 32] {
    // 1. Env var override — process-local, NEVER persisted.
    //
    // This used to call `save_pepper_to_keychain`, so a throwaway value meant for
    // CI/E2E silently replaced the machine's real pepper. Losing the pepper makes
    // every encrypted record on that machine unreadable, and nothing warned about
    // it. An override is an override: it applies to this process and to nothing
    // else. To change the stored pepper, remove the keyring entry and let the app
    // generate a new one.
    if let Ok(raw) = std::env::var("MASTER_PEPPER") {
        if let Ok(decoded) = decode_hex_or_base64(&raw) {
            tracing::warn!(
                "[Config] Usando MASTER_PEPPER do ambiente (apenas neste processo; \
                 o pepper armazenado no cofre nao foi alterado)."
            );
            return decoded;
        }
        // Definida mas invalida: abortar, nunca degradar para o pepper do cofre.
        //
        // Cair para o cofre transforma um ambiente que se pretendia isolado em
        // producao sem avisar. Aconteceu de verdade durante o desenvolvimento:
        // um base64 de 31 bytes num servidor de teste fez o processo ler o
        // pepper real da maquina. Naquele caso nada foi gravado, mas se o cofre
        // estivesse vazio o passo 4 teria gerado e persistido um pepper novo —
        // e um pepper novo sobre dados existentes os torna ilegiveis para sempre.
        //
        // Quem define MASTER_PEPPER esta declarando isolamento. Se o valor nao
        // serve, a resposta certa e parar, nao adivinhar.
        panic!(
            "MASTER_PEPPER esta definida mas e invalida (esperado 32 bytes em hex ou base64; \
             recebido {} caracteres). Corrija o valor ou remova a variavel — seguir com o \
             pepper do cofre transformaria este ambiente isolado em producao.",
            raw.chars().count()
        );
    }

    // 2. Try keychain (OS-native secure storage)
    if let Some(pepper) = load_pepper_from_keychain() {
        return pepper;
    }

    // 3. Try config file (legacy fallback + migration source)
    let path = config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<ConfigFile>(&content) {
                if let Some(pepper_str) = cfg.master_pepper {
                    if let Ok(pepper) = decode_hex_or_base64(&pepper_str) {
                        // Migrate to keychain (keep config.toml as backup)
                        save_pepper_to_keychain(&pepper);
                        return pepper;
                    }
                }
            }
        }
    }

    // 4. Generate new pepper (first run on this machine)
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);

    // Persist to keychain (preferred)
    if save_pepper_to_keychain(&bytes) {
        return bytes;
    }

    // Keychain unavailable — fall back to config file
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::error!("[Config] Falha ao criar diretorio de config: {}", e);
        }
    }
    let cfg = ConfigFile {
        master_pepper: Some(BASE64.encode(bytes)),
        mobile_access_enabled: None,
    };
    if let Ok(content) = toml::to_string(&cfg) {
        if let Err(e) = std::fs::write(&path, content) {
            tracing::error!("[Config] Falha ao persistir pepper no config.toml: {}", e);
        }
    }

    bytes
}

fn decode_hex_or_base64(raw: &str) -> Result<[u8; 32], ()> {
    // Try base64 first
    if let Ok(decoded) = BASE64.decode(raw) {
        if decoded.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            return Ok(key);
        }
    }
    // Try hex
    if let Ok(decoded) = hex_decode(raw) {
        if decoded.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            return Ok(key);
        }
    }
    Err(())
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

impl AppConfig {
    pub fn from_env() -> Self {
        let home = || -> String {
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".into())
        };

        // MOBILE_ACCESS_ENABLED is deliberately no longer read: honoring it would
        // let an environment variable re-expose the API on the network.
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| format!("sqlite:{}/.config/atendemente/atendemente.db?mode=rwc", home())),
            auth_database_url: std::env::var("AUTH_DATABASE_URL")
                .unwrap_or_else(|_| format!("sqlite:{}/.config/atendemente/auth.db?mode=rwc", home())),
            server_port: std::env::var("SERVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3001),
            master_pepper: load_or_generate_pepper(),
            storage_dir: std::env::var("STORAGE_DIR")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(home()).join(".config").join("atendemente").join("uploads")),
            // Where each user's database lives. Overridable so tests can be
            // isolated — see `user_db_path`.
            data_dir: std::env::var("DATA_DIR")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(home()).join(".config").join("atendemente").join("data")),
        }
    }

    /// Scratch directory for backup/restore temporaries.
    ///
    /// Deliberately *not* the system temp dir: these files hold the patient
    /// database in plaintext, and the app's own config directory is easier to
    /// reason about (and to clean up) than a shared location.
    pub fn temp_dir(&self) -> Result<PathBuf, crate::errors::AppError> {
        let dir = self
            .storage_dir
            .parent()
            .map(|p| p.join("tmp"))
            .unwrap_or_else(|| PathBuf::from(".").join("tmp"));
        std::fs::create_dir_all(&dir).map_err(|e| {
            crate::errors::AppError::internal(format!(
                "Erro ao criar diretorio temporario: {}",
                e
            ))
        })?;
        Ok(dir)
    }

    /// Path to a user's database, under `data_dir`.
    ///
    /// This used to read `HOME`/`USERPROFILE` directly, ignoring every other
    /// isolation knob. The e2e suite sets `DATABASE_URL`, `AUTH_DATABASE_URL` and
    /// `STORAGE_DIR` to a temp folder but had no way to redirect this, so each run
    /// left a real directory of fixture patient data in the production config
    /// folder — hundreds of them accumulated. Honor `DATA_DIR` instead.
    pub fn user_db_path(&self, user_id: &str) -> String {
        let dir = self.data_dir.join(user_id);
        let _ = std::fs::create_dir_all(&dir);
        format!("sqlite:{}/atendemente.db?mode=rwc", dir.display())
    }

    pub fn storage_path_for(
        &self,
        user_id: &str,
        patient_id: &str,
        appointment_id: &str,
        filename: &str,
    ) -> Result<PathBuf, crate::errors::AppError> {
        uuid::Uuid::parse_str(user_id)
            .map_err(|_| crate::errors::AppError::bad_request("user_id inválido."))?;
        uuid::Uuid::parse_str(patient_id)
            .map_err(|_| crate::errors::AppError::bad_request("patient_id inválido."))?;
        uuid::Uuid::parse_str(appointment_id)
            .map_err(|_| crate::errors::AppError::bad_request("appointment_id inválido."))?;

        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let uuid = uuid::Uuid::new_v4();
        Ok(self.storage_dir.join(format!(
            "{}/{}/{}/{}.{}",
            user_id, patient_id, appointment_id, uuid, ext
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_hex_or_base64, AppConfig, ConfigFile};
    use std::path::PathBuf;

    fn config_with_data_dir(data_dir: PathBuf) -> AppConfig {
        AppConfig {
            database_url: String::new(),
            auth_database_url: String::new(),
            server_port: 3001,
            master_pepper: [0u8; 32],
            storage_dir: PathBuf::from("uploads"),
            data_dir,
        }
    }

    /// Per-user databases must live under `data_dir`, not under a hardcoded
    /// `$HOME/.config/atendemente/data`. Reading HOME directly is what made the
    /// e2e suite write fixture patient data into the production config folder.
    #[test]
    fn user_db_path_follows_data_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = config_with_data_dir(tmp.path().join("isolado"));
        let uid = "550e8400-e29b-41d4-a716-446655440001";

        let url = cfg.user_db_path(uid);

        let expected = tmp.path().join("isolado").join(uid);
        assert!(
            url.contains(&expected.display().to_string()),
            "esperava o caminho sob data_dir; obtido: {url}"
        );
        assert!(url.starts_with("sqlite:") && url.ends_with("atendemente.db?mode=rwc"));
        assert!(expected.exists(), "o diretorio do usuario deve ser criado");
    }

    #[test]
    fn user_db_path_isolates_users_from_each_other() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = config_with_data_dir(tmp.path().to_path_buf());
        let a = cfg.user_db_path("550e8400-e29b-41d4-a716-44665544000a");
        let b = cfg.user_db_path("550e8400-e29b-41d4-a716-44665544000b");
        assert_ne!(a, b);
    }

    #[test]
    fn decodes_pepper_from_hex_and_base64() {
        let hex = "00".repeat(32);
        assert_eq!(decode_hex_or_base64(&hex).unwrap(), [0u8; 32]);

        let b64 = "q83nH1cS0Zk9vYt7pXeR2mLbA5wJ4gQfN6uD8iO0sT8=";
        assert!(decode_hex_or_base64(b64).is_ok());

        // Comprimento errado e lixo devem ser rejeitados, para que uma
        // MASTER_PEPPER invalida nao vire uma chave silenciosamente truncada.
        assert!(decode_hex_or_base64("00").is_err());
        assert!(decode_hex_or_base64("nao-e-hex-nem-base64!!").is_err());
        assert!(decode_hex_or_base64("").is_err());
    }

    /// A config.toml written by a version that had the mobile-access toggle must
    /// still parse after the feature was removed — otherwise upgrading would
    /// fail to read the file that also holds the fallback master pepper.
    #[test]
    fn parses_legacy_config_with_mobile_access_flag() {
        let legacy = r#"
            master_pepper = "q83nH1cS0Zk9vYt7pXeR2mLbA5wJ4gQfN6uD8iO0sT8="
            mobile_access_enabled = true
        "#;
        let cfg: ConfigFile = toml::from_str(legacy).expect("config legado deve parsear");
        assert_eq!(cfg.mobile_access_enabled, Some(true));
    }

    #[test]
    fn parses_config_without_the_flag() {
        let current = r#"master_pepper = "q83nH1cS0Zk9vYt7pXeR2mLbA5wJ4gQfN6uD8iO0sT8=""#;
        let cfg: ConfigFile = toml::from_str(current).expect("config atual deve parsear");
        assert_eq!(cfg.mobile_access_enabled, None);
    }

    /// Unknown keys must not break parsing either: a user could be downgrading,
    /// or a future field could be added and then dropped.
    #[test]
    fn tolerates_unknown_keys() {
        let cfg: ConfigFile = toml::from_str("alguma_chave_futura = 42")
            .expect("chave desconhecida nao deve quebrar o parse");
        assert!(cfg.mobile_access_enabled.is_none());
    }

    /// Clearing the flag must round-trip: the pepper has to survive, or an
    /// installation that relies on the config-file fallback loses its key.
    #[test]
    fn clearing_the_flag_preserves_the_pepper() {
        let legacy = r#"
            master_pepper = "q83nH1cS0Zk9vYt7pXeR2mLbA5wJ4gQfN6uD8iO0sT8="
            mobile_access_enabled = true
        "#;
        let mut cfg: ConfigFile = toml::from_str(legacy).unwrap();
        cfg.mobile_access_enabled = None;

        let written = toml::to_string(&cfg).unwrap();
        let reparsed: ConfigFile = toml::from_str(&written).unwrap();

        assert_eq!(reparsed.mobile_access_enabled, None);
        assert!(
            written.contains("q83nH1cS0Zk9vYt7pXeR2mLbA5wJ4gQfN6uD8iO0sT8="),
            "o pepper deve sobreviver a limpeza da flag; escrito: {written}"
        );
    }
}
