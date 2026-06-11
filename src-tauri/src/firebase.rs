use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use jsonwebtoken::{decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// Firebase Auth JWKS endpoint.
/// Discovered via https://securetoken.google.com/<project>/.well-known/openid-configuration
const JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";
const CACHE_TTL_SECS: u64 = 3600;

static KEY_CACHE: OnceLock<Mutex<KeyCache>> = OnceLock::new();

struct KeyCache {
    /// kid -> (base64url n, base64url e)
    keys: HashMap<String, (String, String)>,
    fetched_at: Instant,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FirebaseClaims {
    pub sub: String,
    pub aud: String,
    pub iss: String,
    pub exp: usize,
    pub iat: usize,
    pub auth_time: usize,
    pub email: Option<String>,
    pub name: Option<String>,
    pub email_verified: Option<bool>,
}

#[derive(Deserialize)]
struct JwksResponse {
    keys: Vec<JwkEntry>,
}

#[derive(Deserialize)]
struct JwkEntry {
    kid: String,
    n: String,
    e: String,
    #[serde(default)]
    kty: String,
}

fn cache() -> &'static Mutex<KeyCache> {
    KEY_CACHE.get_or_init(|| {
        Mutex::new(KeyCache {
            keys: HashMap::new(),
            fetched_at: Instant::now(),
        })
    })
}

async fn fetch_keys_uncached() -> Result<HashMap<String, (String, String)>, AppError> {
    let resp = reqwest::get(JWKS_URL)
        .await
        .map_err(|e| AppError::internal(format!("Falha ao conectar no Firebase: {}", e)))?;

    let raw_status = resp.status();
    let body = resp.text().await.map_err(|e| {
        AppError::internal(format!("Falha ao ler resposta do Firebase: {}", e))
    })?;

    let jwks: JwksResponse = serde_json::from_str(&body).map_err(|e| {
        AppError::internal(format!(
            "Resposta inesperada do Firebase ({}): {}... erro: {}",
            raw_status,
            &body.chars().take(200).collect::<String>(),
            e
        ))
    })?;

    let keys: HashMap<String, (String, String)> = jwks
        .keys
        .into_iter()
        .filter(|k| k.kty == "RSA")
        .map(|k| (k.kid, (k.n, k.e)))
        .collect();

    if keys.is_empty() {
        return Err(AppError::internal(
            "Nenhuma chave RSA encontrada no Firebase.",
        ));
    }

    tracing::info!("Loaded {} Firebase JWK keys", keys.len());
    Ok(keys)
}

async fn ensure_keys() -> Result<(), AppError> {
    let needs_refresh = {
        let c = cache().lock().unwrap();
        c.keys.is_empty() || c.fetched_at.elapsed().as_secs() > CACHE_TTL_SECS
    };

    if needs_refresh {
        let keys = fetch_keys_uncached().await?;
        let mut c = cache().lock().unwrap();
        c.keys = keys;
        c.fetched_at = Instant::now();
    }

    Ok(())
}

pub async fn verify_firebase_token(
    token: &str,
    project_id: &str,
) -> Result<FirebaseClaims, AppError> {
    ensure_keys().await?;

    let header = decode_header(token)
        .map_err(|_| AppError::unauthorized("Token JWT mal formatado."))?;

    let kid = header
        .kid
        .ok_or_else(|| AppError::unauthorized("Token sem kid (key ID)."))?;

    let key_entry = {
        let c = cache().lock().unwrap();
        c.keys.get(&kid).cloned()
    };

    let (n, e) = match key_entry {
        Some(entry) => entry,
        _ => {
            // Force refresh and try again
            let keys = fetch_keys_uncached().await?;
            let (n, e) = keys.get(&kid).cloned().ok_or_else(|| {
                AppError::unauthorized(
                    "Chave publica do Firebase nao encontrada para este token.",
                )
            })?;
            let mut c = cache().lock().unwrap();
            c.keys = keys;
            c.fetched_at = Instant::now();
            (n, e)
        }
    };

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[format!("https://securetoken.google.com/{}", project_id)]);
    validation.set_audience(&[project_id]);
    validation.set_required_spec_claims(&["sub", "aud", "iss", "exp", "iat", "auth_time"]);

    let key = DecodingKey::from_rsa_components(&n, &e)
        .map_err(|_| AppError::internal("Falha ao construir chave RSA."))?;

    let token_data = jsonwebtoken::decode::<FirebaseClaims>(token, &key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                AppError::unauthorized("Token expirado. Faca login novamente.")
            }
            jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                AppError::unauthorized("Token nao emitido pelo Firebase esperado.")
            }
            jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                AppError::unauthorized("Token nao pertence a este projeto.")
            }
            _ => AppError::unauthorized("Token invalido ou violado."),
        })?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_and_parse_firebase_keys() {
        let keys = fetch_keys_uncached().await.unwrap();
        assert!(!keys.is_empty(), "Should have at least one key");
        for (kid, (n, e)) in &keys {
            assert!(!n.is_empty(), "n should not be empty for kid {}", kid);
            assert!(!e.is_empty(), "e should not be empty for kid {}", kid);
            let result = DecodingKey::from_rsa_components(n, e);
            assert!(
                result.is_ok(),
                "from_rsa_components should succeed for kid {}",
                kid
            );
        }
        tracing::info!(
            "Successfully parsed {} Firebase JWK keys",
            keys.len()
        );
    }
}
