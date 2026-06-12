use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub auth_database_url: String,
    pub server_port: u16,
    pub master_pepper: [u8; 32],
    pub storage_dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConfigFile {
    master_pepper: Option<String>,
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

fn load_or_generate_pepper() -> [u8; 32] {
    // 1. Try env var
    if let Ok(raw) = std::env::var("MASTER_PEPPER") {
        if let Ok(decoded) = decode_hex_or_base64(&raw) {
            return decoded;
        }
    }

    // 2. Try config file
    let path = config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<ConfigFile>(&content) {
                if let Some(pepper) = cfg.master_pepper {
                    if let Ok(decoded) = decode_hex_or_base64(&pepper) {
                        return decoded;
                    }
                }
            }
        }
    }

    // 3. Generate new pepper
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);

    // Persist to config file
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cfg = ConfigFile {
        master_pepper: Some(BASE64.encode(bytes)),
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
    ) -> PathBuf {
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let uuid = uuid::Uuid::new_v4();
        self.storage_dir.join(format!(
            "{}/{}/{}/{}.{}",
            user_id, patient_id, appointment_id, uuid, ext
        ))
    }
}
