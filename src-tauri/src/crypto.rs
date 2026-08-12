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

/// Quantos registros a re-cifra converteu, por artefato.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReencryptReport {
    pub patients: usize,
    pub session_records: usize,
}

/// Re-cifra todo o conteudo cifrado de um banco da chave derivada de
/// `old_pepper` para a derivada do pepper atual.
///
/// Tres coisas que a versao anterior errava, e que custaram caro:
///
/// 1. Ela selecionava `COALESCE(key_version, 1)` de `patients`, coluna que nao
///    existia. A query falhava com "no such column" e o erro subia depois de o
///    restore ja ter substituido banco e anexos. A migration 14 criou a coluna.
/// 2. Ela cobria **so** `patients`. `session_records` — o prontuario de sessao,
///    o dado mais sensivel do app — ficava cifrado com a chave antiga, sem
///    nenhuma marcacao, silenciosamente ilegivel.
/// 3. Ela gravava fora de transacao, um UPDATE por vez. Uma falha no meio
///    deixava metade das linhas em cada chave.
///
/// Agora roda numa transacao unica: ou o banco inteiro converte, ou nada muda.
/// Os anexos em disco nao passam por aqui — eles nao participam da transacao do
/// SQLite e sao convertidos por quem chama, sobre uma copia em area temporaria.
pub async fn reencrypt_all_pii(
    db: &sqlx::SqlitePool,
    old_pepper: &[u8; 32],
    user_id: &str,
) -> Result<ReencryptReport, AppError> {
    let current_pepper = MASTER_PEPPER
        .get()
        .ok_or_else(|| AppError::internal("Master pepper not initialized."))?;

    if old_pepper == current_pepper {
        return Ok(ReencryptReport::default());
    }

    let old_key = derive_key_inner(user_id, old_pepper)?;
    let new_key = derive_key_inner(user_id, current_pepper)?;

    let mut tx = db
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao iniciar re-cifra: {}", e)))?;

    let mut report = ReencryptReport::default();

    // ── patients.pii_* ──────────────────────────────────────────────────────
    let rows: Vec<(String, String, String, String, i32)> = sqlx::query_as(
        r#"SELECT id, pii_encrypted, pii_iv, pii_auth_tag, COALESCE(key_version, 1)
        FROM patients WHERE user_id = ? AND pii_encrypted IS NOT NULL"#,
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler PII: {}", e)))?;

    for (id, enc, iv, tag, kv) in &rows {
        let payload = EncryptedPayload {
            encrypted_payload: enc.clone(),
            iv: iv.clone(),
            auth_tag: tag.clone(),
            key_version: *kv,
        };
        // Verify-before-discard: se nao abre com a chave antiga, aborta a
        // transacao inteira em vez de gravar lixo sobre dado possivelmente bom.
        let plaintext = decrypt_content_with_key(&payload, &old_key).map_err(|_| {
            AppError::bad_request(format!(
                "Nao foi possivel decifrar a PII do paciente {} com a chave do backup. \
                 Nada foi alterado.",
                id
            ))
        })?;
        let np = encrypt_content_with_key(&plaintext, &new_key)?;
        sqlx::query(
            r#"UPDATE patients
               SET pii_encrypted = ?, pii_iv = ?, pii_auth_tag = ?, key_version = ?
               WHERE id = ?"#,
        )
        .bind(&np.encrypted_payload)
        .bind(&np.iv)
        .bind(&np.auth_tag)
        .bind(np.key_version)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao atualizar PII: {}", e)))?;
        report.patients += 1;
    }

    // ── session_records: o prontuario, que a versao anterior nao tocava ──────
    let recs: Vec<(String, String, String, String, i32)> = sqlx::query_as(
        r#"SELECT id, encrypted_payload, iv, auth_tag, COALESCE(key_version, 1)
        FROM session_records WHERE user_id = ?"#,
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler prontuarios: {}", e)))?;

    for (id, enc, iv, tag, kv) in &recs {
        let payload = EncryptedPayload {
            encrypted_payload: enc.clone(),
            iv: iv.clone(),
            auth_tag: tag.clone(),
            key_version: *kv,
        };
        let plaintext = decrypt_content_with_key(&payload, &old_key).map_err(|_| {
            AppError::bad_request(format!(
                "Nao foi possivel decifrar o prontuario {} com a chave do backup. \
                 Nada foi alterado.",
                id
            ))
        })?;
        let np = encrypt_content_with_key(&plaintext, &new_key)?;
        sqlx::query(
            r#"UPDATE session_records
               SET encrypted_payload = ?, iv = ?, auth_tag = ?, key_version = ?
               WHERE id = ?"#,
        )
        .bind(&np.encrypted_payload)
        .bind(&np.iv)
        .bind(&np.auth_tag)
        .bind(np.key_version)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao atualizar prontuario: {}", e)))?;
        report.session_records += 1;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao concluir re-cifra: {}", e)))?;

    Ok(report)
}

/// Converte um anexo da chave derivada de `old_pepper` para a do pepper atual.
///
/// Vale para os dois formatos que aparecem em backups reais: o bundle feito sem
/// sessao guarda o anexo cifrado com a chave antiga; o feito apos login guarda
/// em texto claro, porque `collect_files` decifra ao gravar no ZIP. O
/// passthrough de `decrypt_file` cobre o segundo caso, e a saida sai cifrada com
/// a chave atual nos dois — inclusive no que entrou em claro.
pub fn reencrypt_file_bytes(
    bytes: &[u8],
    old_pepper: &[u8; 32],
    user_id: &str,
) -> Result<Vec<u8>, AppError> {
    let current_pepper = MASTER_PEPPER
        .get()
        .ok_or_else(|| AppError::internal("Master pepper not initialized."))?;
    if old_pepper == current_pepper {
        return Ok(bytes.to_vec());
    }
    let old_key = derive_key_inner(user_id, old_pepper)?;
    let new_key = derive_key_inner(user_id, current_pepper)?;
    let plaintext = decrypt_file(bytes, &old_key)?;
    encrypt_file(&plaintext, &new_key)
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
