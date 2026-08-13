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

pub mod envelope;
pub mod rotation;

const KEY_VERSION: i32 = 1;

static MASTER_PEPPER: OnceLock<[u8; 32]> = OnceLock::new();
static USER_KEYS: OnceLock<Mutex<HashMap<String, KeyRing>>> = OnceLock::new();

/// As chaves de dados de um usuario nesta sessao.
///
/// Durante a rotacao as duas coexistem: parte dos registros ainda esta sob a
/// chave que sai, parte ja esta sob a nova. Sem guardar as duas, uma rotacao
/// interrompida — queda de energia, app fechado no meio — deixaria metade do
/// prontuario ilegivel ate o fim da conversao.
#[derive(Clone)]
pub struct KeyRing {
    pub current: [u8; 32],
    /// Chave anterior, presente apenas enquanto a rotacao nao terminou.
    pub retiring: Option<[u8; 32]>,
}

fn user_keys() -> &'static Mutex<HashMap<String, KeyRing>> {
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
///
/// Caminho legado: deriva do pepper, sem participacao da senha. Continua sendo
/// usado pelos testes e pelo bootstrap do envelope, mas os handlers de
/// autenticacao passaram a usar `unlock_user_crypto`.
pub fn init_user_crypto(user_id: &str) -> Result<(), AppError> {
    let key = derive_user_key(user_id)?;
    cache_key(user_id, key)
}

/// Instala a chave de escrita do usuario, descartando a que estava saindo.
pub fn cache_key_public(user_id: &str, key: [u8; 32]) -> Result<(), AppError> {
    cache_key(user_id, key)
}

fn cache_key(user_id: &str, key: [u8; 32]) -> Result<(), AppError> {
    cache_keyring(user_id, KeyRing { current: key, retiring: None })
}

fn cache_keyring(user_id: &str, ring: KeyRing) -> Result<(), AppError> {
    user_keys()
        .lock()
        .map_err(|_| AppError::internal("Erro ao acessar cache de chaves."))?
        .insert(user_id.to_string(), ring);
    Ok(())
}

/// Chaveiro completo do usuario, com a chave que esta saindo se houver.
pub fn load_keyring(user_id: &str) -> Result<KeyRing, AppError> {
    user_keys()
        .lock()
        .map_err(|_| AppError::internal("Erro ao acessar cache de chaves."))?
        .get(user_id)
        .cloned()
        .ok_or_else(|| {
            AppError::unauthorized("Chave de criptografia nao inicializada. Faca login novamente.")
        })
}

/// Registra a chave que esta saindo, para a decifra por tentativa alcanca-la.
pub fn set_retiring_key(user_id: &str, retiring: [u8; 32]) -> Result<(), AppError> {
    let mut ring = load_keyring(user_id)?;
    ring.retiring = Some(retiring);
    cache_keyring(user_id, ring)
}

/// Abre a DEK do usuario com a senha e devolve o objeto, sem cachear.
///
/// Usada por quem precisa da chave para reembrulha-la — trocar senha, rotacionar
/// o codigo de recuperacao — e nao para cifrar dado. Enquanto a DEK for a legada,
/// cai para a derivacao do pepper pelo mesmo motivo explicado em
/// `unlock_user_crypto`: nao transformar a transicao em perda de acesso.
pub async fn unwrap_dek_for_user(
    auth_db: &sqlx::SqlitePool,
    user_id: &str,
    password: &str,
) -> Result<envelope::Dek, AppError> {
    use envelope::{DekRole, DekSource, Slot};

    match envelope::unwrap_current(auth_db, user_id, password, &[Slot::Password]).await {
        Ok(dek) => Ok(dek),
        Err(e) => {
            let deks = envelope::load_deks(auth_db, user_id).await?;
            let legada = deks
                .iter()
                .any(|d| d.role == DekRole::Current && d.source == DekSource::LegacyPepperV1);
            if legada {
                return Ok(envelope::Dek::from_bytes(derive_user_key(user_id)?));
            }
            Err(e)
        }
    }
}

/// Carrega a chave de dados **a partir da senha**, pelo envelope.
///
/// Esta e a funcao que os handlers de register/login/unlock usam. Se o usuario
/// ainda nao tem envelope — todo mundo que ja usava o app —, ele e criado aqui,
/// na primeira autenticacao, tendo como DEK a **propria chave legada**.
///
/// Por que a DEK continua sendo a chave legada nesta etapa: trocar por uma chave
/// aleatoria agora criaria um estado misto em que uns usuarios tem chave
/// derivavel do pepper e outros nao, enquanto o caminho de restore de backup
/// ainda deriva do pepper. A rotacao para chave aleatoria e uniforme, e vem na
/// etapa seguinte.
///
/// **Esta etapa nao entrega ganho de seguranca**, e isso e intencional: enquanto
/// `source = legacy_pepper_v1`, a chave continua derivavel do pepper do cofre, e
/// o fallback abaixo depende disso de proposito — e o que garante que ninguem
/// perca acesso durante a transicao. O ganho vem quando a rotacao acontece e o
/// pepper e removido do cofre.
pub async fn unlock_user_crypto(
    auth_db: &sqlx::SqlitePool,
    user_id: &str,
    password: &str,
    recovery_secret: Option<&str>,
) -> Result<(), AppError> {
    use envelope::{DekRole, DekSource, Slot};

    let deks = envelope::load_deks(auth_db, user_id).await?;
    let tem_envelope = deks.iter().any(|d| d.role == DekRole::Current);

    if tem_envelope {
        match envelope::unwrap_current(auth_db, user_id, password, &[Slot::Password]).await {
            Ok(dek) => return cache_key(user_id, *dek.expose()),
            Err(e) => {
                // Enquanto a DEK for a legada, a chave e reconstruivel do pepper.
                // Cair para esse caminho evita transformar um envelope
                // inconsistente em perda de acesso durante a transicao. Quando a
                // rotacao acontecer, `source` deixa de ser legacy e este ramo
                // para de existir — e a senha passa a ser indispensavel.
                let legada = deks
                    .iter()
                    .any(|d| d.role == DekRole::Current && d.source == DekSource::LegacyPepperV1);
                if legada {
                    tracing::warn!(
                        "[Crypto] Nao foi possivel abrir a chave de {} pelo envelope ({}); \
                         usando a derivacao legada do pepper. Isso so funciona porque a chave \
                         ainda nao foi rotacionada.",
                        user_id,
                        e
                    );
                    return init_user_crypto(user_id);
                }
                return Err(e);
            }
        }
    }

    // Primeiro acesso apos a atualizacao: cria o envelope sobre a chave que o
    // usuario ja tem, sem tocar em nenhum dado cifrado.
    let key = derive_user_key(user_id)?;
    let dek = envelope::Dek::from_bytes(key);

    let mut wraps = vec![envelope::wrap_dek(
        &dek,
        password,
        user_id,
        Slot::Password,
        Slot::Password.default_params(),
    )?];

    // O wrap de recuperacao exige o codigo em claro, que so existe no momento do
    // register ou de um reset. Num login comum nao ha como cria-lo: o banco
    // guarda apenas o hash. Para quem vem de versao anterior, ele e criado
    // quando o codigo for rotacionado, na etapa da rotacao de chave.
    if let Some(secret) = recovery_secret {
        wraps.push(envelope::wrap_dek(
            &dek,
            secret,
            user_id,
            Slot::Recovery,
            Slot::Recovery.default_params(),
        )?);
    }

    envelope::store_dek(
        auth_db,
        user_id,
        &dek,
        DekRole::Current,
        DekSource::LegacyPepperV1,
        &wraps,
    )
    .await?;

    tracing::info!(
        "[Crypto] Envelope criado para {} com {} wrap(s). A chave de dados nao mudou.",
        user_id,
        wraps.len()
    );
    cache_key(user_id, key)
}

/// Clear user crypto on logout.
pub fn clear_user_crypto(user_id: &str) {
    if let Ok(mut guard) = user_keys().lock() {
        guard.remove(user_id);
    }
}

/// A chave com que dado NOVO deve ser cifrado.
pub fn load_key(user_id: &str) -> Result<[u8; 32], AppError> {
    Ok(load_keyring(user_id)?.current)
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

/// Resultado de uma decifra que pode ter usado a chave antiga.
pub struct DecryptOutcome {
    pub plaintext: String,
    /// `true` quando so a chave que esta saindo abriu — ou seja, este registro
    /// ainda nao foi convertido.
    pub used_retiring: bool,
}

/// Decifra tentando a chave atual e, em seguida, a que esta saindo.
///
/// **A autoridade sobre qual chave abre um registro e a propria decifra, nunca
/// uma coluna de versao.** Duas razoes concretas:
///
/// - Uma rotacao interrompida deixa parte das linhas em cada chave, e nada
///   garante que o marcador tenha sido gravado antes da queda. Confiar nele
///   transformaria uma interrupcao em dado ilegivel.
/// - Uma linha marcada como convertida fica obsoleta na rotacao seguinte, e duas
///   maquinas restaurando o mesmo backup geram DEKs diferentes para o mesmo
///   usuario. O marcador serve como dica de otimizacao; a verdade e o AES-GCM,
///   que so autentica com a chave certa.
pub fn decrypt_content_trying_all(
    payload: &EncryptedPayload,
    user_id: &str,
) -> Result<DecryptOutcome, AppError> {
    let ring = load_keyring(user_id)?;
    if let Ok(plaintext) = decrypt_content_with_key(payload, &ring.current) {
        return Ok(DecryptOutcome { plaintext, used_retiring: false });
    }
    if let Some(old) = ring.retiring {
        let plaintext = decrypt_content_with_key(payload, &old)?;
        return Ok(DecryptOutcome { plaintext, used_retiring: true });
    }
    // Sem chave antiga, o erro da chave atual e o erro real.
    decrypt_content_with_key(payload, &ring.current)
        .map(|plaintext| DecryptOutcome { plaintext, used_retiring: false })
}

/// Decrypt content using the authenticated user's key.
pub fn decrypt_content(payload: &EncryptedPayload, user_id: &str) -> Result<String, AppError> {
    Ok(decrypt_content_trying_all(payload, user_id)?.plaintext)
}

/// Decifra um arquivo tentando a chave atual e depois a que esta saindo.
///
/// Anexos sao o unico artefato fora da transacao do SQLite: o arquivo e trocado
/// por rename e a linha e atualizada em seguida, entao uma queda entre os dois
/// passos deixa arquivo e marcador discordando. Tentar as duas chaves faz o
/// desencontro deixar de importar.
pub fn decrypt_file_trying_all(data: &[u8], user_id: &str) -> Result<Vec<u8>, AppError> {
    let ring = load_keyring(user_id)?;
    if let Ok(plain) = decrypt_file(data, &ring.current) {
        return Ok(plain);
    }
    if let Some(old) = ring.retiring {
        return decrypt_file(data, &old);
    }
    decrypt_file(data, &ring.current)
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
    reencrypt_between_keys(db, &old_key, &new_key, user_id).await
}

/// Chave legada derivada de um pepper especifico.
pub fn legacy_key_from_pepper(user_id: &str, pepper: &[u8; 32]) -> Result<[u8; 32], AppError> {
    derive_key_inner(user_id, pepper)
}

/// Re-cifra o conteudo de um banco de `old_key` para `new_key`.
///
/// **Decide por decifra, nao por marcador nem por comparacao de pepper.** Para
/// cada registro: se ja abre com `new_key`, esta convertido e e ignorado; se abre
/// com `old_key`, e convertido; se nenhuma abre, aborta a transacao inteira.
///
/// Isso torna a operacao idempotente e resistente a interrupcao, e conserta um
/// bug concreto no restore: a decisao era `old_pepper == current_pepper`, entao
/// restaurar um backup pre-rotacao **na mesma maquina** nao convertia nada. Numa
/// conta ja rotacionada — DEK aleatoria, sem chave antiga no chaveiro — os dados
/// voltavam cifrados com a chave legada e o app nao conseguia le-los. Depois que
/// o envelope existe, "o pepper mudou?" e a pergunta errada; a pergunta e "isto
/// abre com a chave com que eu gravo hoje?".
pub async fn reencrypt_between_keys(
    db: &sqlx::SqlitePool,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
    user_id: &str,
) -> Result<ReencryptReport, AppError> {
    if old_key == new_key {
        return Ok(ReencryptReport::default());
    }
    let (old_key, new_key) = (*old_key, *new_key);

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
        // Ja convertido? Nada a fazer — e o que torna a conversao idempotente e
        // retomavel depois de uma interrupcao.
        if decrypt_content_with_key(&payload, &new_key).is_ok() {
            continue;
        }
        // Verify-before-discard: se nao abre com nenhuma das duas, aborta a
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
        if decrypt_content_with_key(&payload, &new_key).is_ok() {
            continue;
        }
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
/// Converte um anexo de `old_key` para `new_key`.
///
/// Decide por decifra, como o resto: se o conteudo ja abre com a chave alvo, e
/// devolvido sem alteracao. Isso cobre os dois formatos que aparecem em backups
/// reais — cifrado com a chave antiga (bundle feito sem sessao) e em texto claro
/// (bundle pos-login, porque `collect_files` decifra ao gravar no ZIP) — e a
/// saida sai cifrada com a chave alvo nos dois casos.
pub fn reencrypt_file_between_keys(
    bytes: &[u8],
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<Vec<u8>, AppError> {
    if old_key == new_key {
        return Ok(bytes.to_vec());
    }
    // Ja esta sob a chave alvo? Nada a fazer. O teste do primeiro byte separa
    // "abriu porque estava cifrado com esta chave" de "abriu porque o
    // passthrough devolveu texto claro intacto".
    if bytes.first() == Some(&0x01) && decrypt_file(bytes, new_key).is_ok() {
        return Ok(bytes.to_vec());
    }
    let plaintext = decrypt_file(bytes, old_key)?;
    encrypt_file(&plaintext, new_key)
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

    // ── Chaveiro e decifra por tentativa ────────────────────────────────────

    /// O cenario que a rotacao precisa sobreviver: metade dos registros sob a
    /// chave nova, metade ainda sob a antiga. Sem tentar as duas, uma queda de
    /// energia no meio da conversao deixaria parte do prontuario ilegivel.
    #[test]
    fn le_registro_que_ainda_esta_sob_a_chave_antiga() {
        let uid = "u-rotacao";
        let antiga = [0x11u8; 32];
        let nova = [0x22u8; 32];

        // Registro gravado antes da rotacao.
        let antigo = encrypt_content_with_key("prontuario nao convertido", &antiga).unwrap();
        // Registro gravado depois.
        let novo = encrypt_content_with_key("prontuario ja convertido", &nova).unwrap();

        cache_keyring(uid, KeyRing { current: nova, retiring: Some(antiga) }).unwrap();

        let r = decrypt_content_trying_all(&novo, uid).unwrap();
        assert_eq!(r.plaintext, "prontuario ja convertido");
        assert!(!r.used_retiring);

        let r = decrypt_content_trying_all(&antigo, uid).unwrap();
        assert_eq!(r.plaintext, "prontuario nao convertido");
        assert!(
            r.used_retiring,
            "precisa sinalizar que veio da chave antiga, e o que dispara a correcao da linha"
        );

        clear_user_crypto(uid);
    }

    /// Sem chave antiga registrada, nada muda: o erro continua sendo o da chave
    /// atual, e nao um erro generico que esconderia a causa.
    #[test]
    fn sem_chave_antiga_o_erro_e_o_da_chave_atual() {
        let uid = "u-sem-antiga";
        let alheio = encrypt_content_with_key("dado de outra chave", &[0x33u8; 32]).unwrap();
        cache_keyring(uid, KeyRing { current: [0x44u8; 32], retiring: None }).unwrap();

        assert!(decrypt_content_trying_all(&alheio, uid).is_err());
        clear_user_crypto(uid);
    }

    /// Anexos sao o unico artefato fora da transacao do SQLite: o arquivo e
    /// trocado por rename e a linha e atualizada depois, entao uma queda entre
    /// os dois passos deixa arquivo e marcador discordando.
    #[test]
    fn anexo_abre_com_qualquer_uma_das_duas_chaves() {
        let uid = "u-anexo-rotacao";
        let antiga = [0x55u8; 32];
        let nova = [0x66u8; 32];
        let conteudo = b"%PDF-1.4 conteudo ficticio";

        let sob_antiga = encrypt_file(conteudo, &antiga).unwrap();
        let sob_nova = encrypt_file(conteudo, &nova).unwrap();

        cache_keyring(uid, KeyRing { current: nova, retiring: Some(antiga) }).unwrap();

        assert_eq!(decrypt_file_trying_all(&sob_nova, uid).unwrap(), conteudo);
        assert_eq!(decrypt_file_trying_all(&sob_antiga, uid).unwrap(), conteudo);

        // E o passthrough de arquivo em texto claro continua valendo — e o
        // estado de todo anexo vindo de backup.
        let em_claro = conteudo.to_vec();
        assert_eq!(decrypt_file_trying_all(&em_claro, uid).unwrap(), conteudo);

        clear_user_crypto(uid);
    }

    #[test]
    fn set_retiring_key_preserva_a_chave_atual() {
        let uid = "u-set-retiring";
        cache_key(uid, [0x77u8; 32]).unwrap();
        assert!(load_keyring(uid).unwrap().retiring.is_none());

        set_retiring_key(uid, [0x88u8; 32]).unwrap();
        let ring = load_keyring(uid).unwrap();
        assert_eq!(ring.current, [0x77u8; 32], "a chave de escrita nao pode mudar");
        assert_eq!(ring.retiring, Some([0x88u8; 32]));

        clear_user_crypto(uid);
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
