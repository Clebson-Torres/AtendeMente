use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

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

/// Derive a 32-byte AES key from the user's Firebase UID and the master pepper (as salt).
pub fn derive_user_key(firebase_uid: &str) -> Result<[u8; 32], AppError> {
    let pepper = MASTER_PEPPER
        .get()
        .ok_or_else(|| AppError::internal("Master pepper not initialized."))?;
    Ok(derive_key_inner(firebase_uid, pepper))
}

fn derive_key_inner(firebase_uid: &str, pepper: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(pepper), firebase_uid.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(&[], &mut okm).expect("HKDF expand should not fail");
    okm
}

/// Initialize user crypto on login — derives and caches the key.
pub fn init_user_crypto(user_id: &str) -> Result<(), AppError> {
    let key = derive_user_key(user_id)?;
    user_keys()
        .lock()
        .unwrap()
        .insert(user_id.to_string(), key);
    Ok(())
}

/// Clear user crypto on logout.
pub fn clear_user_crypto(user_id: &str) {
    user_keys().lock().unwrap().remove(user_id);
}

fn load_key(user_id: &str) -> Result<[u8; 32], AppError> {
    user_keys()
        .lock()
        .unwrap()
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
        let k1 = derive_key_inner("user-123", &pepper);
        let k2 = derive_key_inner("user-123", &pepper);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_user_key_different_users() {
        let pepper = [0xabu8; 32];
        let k1 = derive_key_inner("user-123", &pepper);
        let k2 = derive_key_inner("user-456", &pepper);
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
}
