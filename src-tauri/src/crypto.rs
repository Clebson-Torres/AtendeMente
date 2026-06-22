use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::errors::AppError;

const KEY_VERSION: i32 = 1;

static MASTER_PEPPER: OnceLock<[u8; 32]> = OnceLock::new();
static USER_KEYS: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();

fn user_keys() -> &'static Mutex<HashMap<String, [u8; 32]>> {
    USER_KEYS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedPayload {
    pub encrypted_payload: String,
    pub iv: String,
    pub auth_tag: String,
    pub key_version: i32,
}

/// Set the master pepper once at startup.
pub fn set_pepper(pepper: &[u8; 32]) {
    let _ = MASTER_PEPPER.set(*pepper);
}

/// Derive a 32-byte AES key from the user's ID and the master pepper (as salt).
pub fn derive_user_key(user_id: &str) -> Result<[u8; 32], AppError> {
    let pepper = MASTER_PEPPER
        .get()
        .ok_or_else(|| AppError::internal("Master pepper not initialized."))?;
    derive_key_inner(user_id, pepper)
}

fn derive_key_inner(user_id: &str, pepper: &[u8; 32]) -> Result<[u8; 32], AppError> {
    let hk = Hkdf::<Sha256>::new(Some(pepper), user_id.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(&[], &mut okm)
        .map_err(|_| AppError::internal("HKDF expand falhou."))?;
    Ok(okm)
}

/// Initialize user crypto on login — derives and caches the key.
pub fn init_user_crypto(user_id: &str) -> Result<(), AppError> {
    let key = derive_user_key(user_id)?;
    user_keys()
        .lock()
        .map_err(|_| AppError::internal("Erro ao acessar cache de chaves."))?
        .insert(user_id.to_string(), key);
    Ok(())
}

/// Clear user crypto on logout.
pub fn clear_user_crypto(user_id: &str) {
    if let Ok(mut guard) = user_keys().lock() {
        guard.remove(user_id);
    }
}

pub fn load_key(user_id: &str) -> Result<[u8; 32], AppError> {
    user_keys()
        .lock()
        .map_err(|_| AppError::internal("Erro ao acessar cache de chaves."))?
        .get(user_id)
        .copied()
        .ok_or_else(|| {
            AppError::unauthorized(
                "Chave de criptografia nao inicializada. Faca login novamente.",
            )
        })
}

pub fn encrypt_content_with_key(content: &str, key: &[u8; 32]) -> Result<EncryptedPayload, AppError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::internal("Failed to create cipher."))?;

    let mut iv = [0u8; 12];
    OsRng.fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    let ciphertext: Vec<u8> = cipher
        .encrypt(nonce, content.as_bytes())
        .map_err(|e| AppError::internal(format!("Encryption failed: {}", e)))?;

    let tag_start = ciphertext.len().saturating_sub(16);
    let (encrypted_data, auth_tag) = ciphertext.split_at(tag_start);

    Ok(EncryptedPayload {
        encrypted_payload: BASE64.encode(encrypted_data),
        iv: BASE64.encode(iv),
        auth_tag: BASE64.encode(auth_tag),
        key_version: KEY_VERSION,
    })
}

pub fn decrypt_content_with_key(payload: &EncryptedPayload, key: &[u8; 32]) -> Result<String, AppError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::internal("Failed to create cipher."))?;

    let iv = BASE64
        .decode(&payload.iv)
        .map_err(|_| AppError::bad_request("Invalid IV encoding."))?;
    let nonce = Nonce::from_slice(&iv);

    let mut ciphertext = BASE64
        .decode(&payload.encrypted_payload)
        .map_err(|_| AppError::bad_request("Invalid payload encoding."))?;
    let auth_tag = BASE64
        .decode(&payload.auth_tag)
        .map_err(|_| AppError::bad_request("Invalid auth tag encoding."))?;
    ciphertext.extend_from_slice(&auth_tag);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| AppError::bad_request("Decryption failed. Data may be tampered."))?;

    String::from_utf8(plaintext)
        .map_err(|_| AppError::internal("Decrypted data is not valid UTF-8."))
}

/// Encrypt content using the authenticated user's key.
pub fn encrypt_content(content: &str, user_id: &str) -> Result<EncryptedPayload, AppError> {
    let key = load_key(user_id)?;
    encrypt_content_with_key(content, &key)
}

/// Decrypt content using the authenticated user's key.
pub fn decrypt_content(payload: &EncryptedPayload, user_id: &str) -> Result<String, AppError> {
    let key = load_key(user_id)?;
    decrypt_content_with_key(payload, &key)
}

pub fn pepper_fingerprint() -> Result<String, AppError> {
    let pepper = MASTER_PEPPER
        .get()
        .ok_or_else(|| AppError::internal("Master pepper not initialized."))?;
    let mut hasher = Sha256::new();
    hasher.update(pepper);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<[u8; 32], AppError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::internal(format!("Erro ao derivar chave: {}", e)))?;
    Ok(key)
}

pub fn encrypt_file(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, AppError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::internal("Failed to create cipher."))?;

    let mut iv = [0u8; 12];
    OsRng.fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| AppError::internal(format!("Encryption failed: {}", e)))?;

    let mut result = Vec::with_capacity(1 + 12 + ciphertext.len());
    result.push(0x01);
    result.extend_from_slice(&iv);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt_file(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, AppError> {
    if data.first() != Some(&0x01) || data.len() < 29 {
        return Ok(data.to_vec());
    }
    let (_, rest) = data.split_at(1);
    let (iv_bytes, ciphertext) = rest.split_at(12);
    let nonce = Nonce::from_slice(iv_bytes);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| AppError::internal("Failed to create cipher."))?;
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::bad_request("Falha ao descriptografar arquivo."))
}

pub async fn reencrypt_all_pii(
    db: &sqlx::SqlitePool,
    old_pepper: &[u8; 32],
    user_id: &str,
) -> Result<(), AppError> {
    let current_pepper = MASTER_PEPPER
        .get()
        .ok_or_else(|| AppError::internal("Master pepper not initialized."))?;

    if old_pepper == current_pepper {
        return Ok(());
    }

    let rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
        r#"SELECT id, pii_encrypted, pii_iv, pii_auth_tag, COALESCE(key_version, 1)
        FROM patients WHERE user_id = ? AND pii_encrypted IS NOT NULL"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler PII: {}", e)))?;

    let old_key = derive_key_inner(user_id, old_pepper)?;
    let new_key = derive_key_inner(user_id, current_pepper)?;

    for (id, enc, iv, tag, kv) in &rows {
        let payload = EncryptedPayload {
            encrypted_payload: enc.clone(),
            iv: iv.clone(),
            auth_tag: tag.clone(),
            key_version: *kv,
        };
        let plaintext = decrypt_content_with_key(&payload, &old_key)?;
        let new_payload = encrypt_content_with_key(&plaintext, &new_key)?;
        sqlx::query(
            r#"UPDATE patients SET pii_encrypted = ?, pii_iv = ?, pii_auth_tag = ? WHERE id = ?"#,
        )
        .bind(&new_payload.encrypted_payload)
        .bind(&new_payload.iv)
        .bind(&new_payload.auth_tag)
        .bind(id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao atualizar PII: {}", e)))?;
    }

    Ok(())
}

pub fn get_pepper() -> Option<&'static [u8; 32]> {
    MASTER_PEPPER.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0u8; 32]
    }

    fn wrong_key() -> [u8; 32] {
        [2u8; 32]
    }

    #[test]
    fn test_derive_user_key_deterministic() {
        let pepper = [0xabu8; 32];
        let k1 = derive_key_inner("user-123", &pepper).unwrap();
        let k2 = derive_key_inner("user-123", &pepper).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_user_key_different_users() {
        let pepper = [0xabu8; 32];
        let k1 = derive_key_inner("user-123", &pepper).unwrap();
        let k2 = derive_key_inner("user-456", &pepper).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let content = "Paciente relatou melhora significativa nos sintomas de ansiedade.";
        let encrypted = encrypt_content_with_key(content, &key).unwrap();
        let decrypted = decrypt_content_with_key(&encrypted, &key).unwrap();
        assert_eq!(content, decrypted);
    }

    #[test]
    fn test_encrypt_different_iv() {
        let key = test_key();
        let content = "Mesmo texto";
        let e1 = encrypt_content_with_key(content, &key).unwrap();
        let e2 = encrypt_content_with_key(content, &key).unwrap();
        assert_ne!(e1.iv, e2.iv);
        assert_ne!(e1.encrypted_payload, e2.encrypted_payload);
    }

    #[test]
    fn test_decrypt_tampered_payload_fails() {
        let key = test_key();
        let content = "Texto secreto";
        let mut encrypted = encrypt_content_with_key(content, &key).unwrap();

        let original = encrypted.encrypted_payload.clone();
        encrypted.encrypted_payload = format!("x{}", &original[1..]);

        assert!(decrypt_content_with_key(&encrypted, &key).is_err());
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key = test_key();
        let bad_key = wrong_key();
        let encrypted = encrypt_content_with_key("dados", &key).unwrap();
        assert!(decrypt_content_with_key(&encrypted, &bad_key).is_err());
    }

    #[test]
    fn test_encrypt_file_roundtrip() {
        let key = test_key();
        let data = b"Hello, encrypted file!";
        let encrypted = encrypt_file(data, &key).unwrap();
        assert_eq!(encrypted.first(), Some(&0x01));
        assert!(encrypted.len() > data.len());
        let decrypted = decrypt_file(&encrypted, &key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_decrypt_file_legacy() {
        let key = test_key();
        let legacy = b"This is a legacy plaintext file";
        let result = decrypt_file(legacy, &key).unwrap();
        assert_eq!(result, legacy);
    }

    #[test]
    fn test_decrypt_file_too_short() {
        let key = test_key();
        let short = vec![0x01, 0x02, 0x03];
        let result = decrypt_file(&short, &key).unwrap();
        assert_eq!(result, short);
    }

    #[test]
    fn test_pepper_fingerprint() {
        set_pepper(&[0xabu8; 32]);
        let fp = pepper_fingerprint().unwrap();
        assert_eq!(fp.len(), 64);
        // Same pepper = same fingerprint
        let fp2 = pepper_fingerprint().unwrap();
        assert_eq!(fp, fp2);
    }

    #[test]
    fn test_derive_key_from_password() {
        let key1 = derive_key_from_password("minha-senha", b"0123456789abcdef").unwrap();
        assert_eq!(key1.len(), 32);
        // Same password + salt = same key
        let key2 = derive_key_from_password("minha-senha", b"0123456789abcdef").unwrap();
        assert_eq!(key1, key2);
        // Different salt = different key
        let key3 = derive_key_from_password("minha-senha", b"fedcba9876543210").unwrap();
        assert_ne!(key1, key3);
        // Different password = different key
        let key4 = derive_key_from_password("outra-senha", b"0123456789abcdef").unwrap();
        assert_ne!(key1, key4);
    }
}
