use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use keyring::Entry;
use rand::RngCore;

const KEYCHAIN_SERVICE: &str = "atendemente";
const KEYCHAIN_ACCOUNT: &str = "master_pepper";
pub const MAX_UPLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub auth_database_url: String,
    pub server_port: u16,
    pub master_pepper: [u8; 32],
    pub storage_dir: PathBuf,
    pub mobile_access_enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ConfigFile {
    master_pepper: Option<String>,
    mobile_access_enabled: Option<bool>,
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

fn load_config_file() -> Result<ConfigFile, Box<dyn std::error::Error>> {
    let path = config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let cfg: ConfigFile = toml::from_str(&content)?;
        Ok(cfg)
    } else {
        Ok(ConfigFile::default())
    }
}

fn save_config_file(cfg: &ConfigFile) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string(cfg)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn set_mobile_access_enabled(enabled: bool) {
    let mut cfg = load_config_file().unwrap_or_default();
    cfg.mobile_access_enabled = Some(enabled);
    let _ = save_config_file(&cfg);
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
    // 1. Try env var (highest priority — for CI/E2E)
    if let Ok(raw) = std::env::var("MASTER_PEPPER") {
        if let Ok(decoded) = decode_hex_or_base64(&raw) {
            save_pepper_to_keychain(&decoded);
            return decoded;
        }
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
        let _ = std::fs::create_dir_all(parent);
    }
    let cfg = ConfigFile {
        master_pepper: Some(BASE64.encode(bytes)),
        mobile_access_enabled: None,
    };
    if let Ok(content) = toml::to_string(&cfg) {
        let _ = std::fs::write(&path, content);
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

        let mobile_from_env = std::env::var("MOBILE_ACCESS_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok());
        let mobile_from_file = load_config_file()
            .ok()
            .and_then(|f| f.mobile_access_enabled);

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
            mobile_access_enabled: mobile_from_env.or(mobile_from_file).unwrap_or(false),
        }
    }

    pub fn user_db_path(&self, user_id: &str) -> String {
        let config_dir = PathBuf::from(
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".into()),
        )
        .join(".config")
        .join("atendemente");
        let dir = config_dir.join("data").join(user_id);
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
