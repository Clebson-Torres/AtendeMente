//! Criptografia envelopada da chave de dados.
//!
//! O problema que isto resolve: hoje a chave AES sai de
//! `HKDF(salt = pepper_do_cofre, ikm = user_id)`. A senha do usuario nao
//! participa, entao qualquer processo rodando na conta do sistema operacional
//! le o pepper do cofre, deriva a chave e abre todo o prontuario sem saber a
//! senha. Isso foi demonstrado na pratica, com o servidor desligado.
//!
//! No modelo envelopado existe uma **DEK** (Data Encryption Key) aleatoria por
//! usuario, que nunca e persistida em claro. Ela e guardada embrulhada por
//! **KEKs** derivadas de segredos que so o usuario tem:
//!
//! ```text
//! DEK  = 32 bytes CSPRNG          <- cifra patients.pii_*, session_records, anexos
//! KEK  = Argon2id(segredo, salt)  <- desembrulha a DEK
//! wrap = AES-256-GCM(KEK, DEK, aad)
//! ```
//!
//! Duas decisoes que valem explicacao:
//!
//! **Os parametros do Argon2id sao gravados por linha, nunca lidos de
//! constante.** Eles sobem com o tempo, e um wrap criado ha dois anos precisa
//! continuar abrindo com os parametros com que foi criado. Ler de constante
//! transformaria um aumento de custo em perda de acesso.
//!
//! **O AAD amarra o wrap ao usuario e ao slot.** Sem isso, alguem com acesso de
//! escrita ao banco poderia mover o wrap de recuperacao de um usuario para o
//! slot de senha de outro. Com o AAD, a decifra falha.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::errors::AppError;

/// Parametros do Argon2id para embrulhar a DEK.
///
/// Bem acima do `Argon2::default()` (19 MiB / t=2) que o projeto usa para
/// verificar senha: aquilo e dimensionado para verificacao online, onde o
/// atacante precisa falar com o servidor. Aqui o atacante **tem o arquivo** e
/// ataca offline, entao paga-se mais por tentativa.
///
/// `p = 1` e fixo de proposito: a saida do Argon2 depende do paralelismo, e
/// deixa-lo variar com a contagem de nucleos da maquina produziria wraps que
/// nao abrem em outro computador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// Para o slot da senha da conta: ~64 MiB, digitada a cada login.
    pub const PASSWORD: KdfParams = KdfParams { m_cost: 65_536, t_cost: 3, p_cost: 1 };

    /// Para o slot do codigo de recuperacao: mais caro, porque roda raramente e
    /// o segredo e menor que uma senha escolhida por humano.
    pub const RECOVERY: KdfParams = KdfParams { m_cost: 131_072, t_cost: 4, p_cost: 1 };
}

/// Slot de wrap. O nome vira parte do AAD, entao trocar estas strings invalida
/// os wraps existentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Password,
    Recovery,
    /// Wrap de recuperacao anterior, mantido ate o usuario confirmar que anotou
    /// o codigo novo. Sem ele, um reset em que o usuario perde o codigo emitido
    /// e esquece a senha e perda total, sem caminho de suporte.
    RecoveryPrev,
}

impl Slot {
    pub fn as_str(&self) -> &'static str {
        match self {
            Slot::Password => "password",
            Slot::Recovery => "recovery",
            Slot::RecoveryPrev => "recovery_prev",
        }
    }

    pub fn parse(s: &str) -> Option<Slot> {
        match s {
            "password" => Some(Slot::Password),
            "recovery" => Some(Slot::Recovery),
            "recovery_prev" => Some(Slot::RecoveryPrev),
            _ => None,
        }
    }

    /// Parametros padrao ao criar um wrap novo neste slot.
    pub fn default_params(&self) -> KdfParams {
        match self {
            Slot::Password => KdfParams::PASSWORD,
            Slot::Recovery | Slot::RecoveryPrev => KdfParams::RECOVERY,
        }
    }
}

/// A chave de dados. Zera a memoria ao sair de escopo.
///
/// Nao implementa `Clone`, `Debug` nem `Serialize` de proposito: a DEK nunca
/// deve ser copiada por descuido, aparecer num log ou virar JSON.
pub struct Dek(Zeroizing<[u8; 32]>);

impl Dek {
    pub fn generate() -> Dek {
        let mut k = [0u8; 32];
        OsRng.fill_bytes(&mut k);
        Dek(Zeroizing::new(k))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Dek {
        Dek(Zeroizing::new(bytes))
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    /// Identificador publico da DEK: SHA-256 truncado, em base64.
    ///
    /// Serve para responder "esta e a mesma chave?" sem expor material. Como a
    /// DEK e aleatoria de 256 bits, nao ha risco de dicionario.
    pub fn check(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.0.as_ref());
        BASE64.encode(&h.finalize()[..16])
    }
}

/// Uma DEK embrulhada, como vai para o banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedDek {
    pub slot: Slot,
    pub kdf: String,
    pub params: KdfParams,
    pub salt: String,
    pub nonce: String,
    pub wrapped: String,
    pub aad_label: String,
}

/// String de dado autenticado adicional. Amarra o wrap ao usuario e ao slot.
pub fn aad_label(user_id: &str, slot: Slot) -> String {
    format!("atendemente:dek:v2|{}|{}", user_id, slot.as_str())
}

fn derive_kek(secret: &str, salt: &[u8], p: KdfParams) -> Result<Zeroizing<[u8; 32]>, AppError> {
    let params = Params::new(p.m_cost, p.t_cost, p.p_cost, Some(32))
        .map_err(|e| AppError::internal(format!("Parametros de KDF invalidos: {}", e)))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut kek = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(secret.as_bytes(), salt, kek.as_mut())
        .map_err(|e| AppError::internal(format!("Erro ao derivar chave: {}", e)))?;
    Ok(kek)
}

/// Embrulha a DEK com um segredo do usuario.
pub fn wrap_dek(
    dek: &Dek,
    secret: &str,
    user_id: &str,
    slot: Slot,
    params: KdfParams,
) -> Result<WrappedDek, AppError> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);

    let kek = derive_kek(secret, &salt, params)?;
    let cipher = Aes256Gcm::new_from_slice(kek.as_ref())
        .map_err(|_| AppError::internal("Falha ao criar cifra."))?;
    let aad = aad_label(user_id, slot);
    let wrapped = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload { msg: dek.expose(), aad: aad.as_bytes() },
        )
        .map_err(|_| AppError::internal("Falha ao embrulhar a chave."))?;

    Ok(WrappedDek {
        slot,
        kdf: "argon2id".to_string(),
        params,
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce_bytes),
        wrapped: BASE64.encode(&wrapped),
        aad_label: aad,
    })
}

/// Desembrulha a DEK.
///
/// Falha se o segredo estiver errado, se o wrap tiver sido movido para outro
/// slot ou outro usuario (o AAD nao bate), ou se o material estiver corrompido.
/// Nao distingue os casos de proposito: a mensagem nao deve dizer ao atacante
/// qual parte ele acertou.
pub fn unwrap_dek(w: &WrappedDek, secret: &str, user_id: &str) -> Result<Dek, AppError> {
    let salt = BASE64
        .decode(&w.salt)
        .map_err(|_| AppError::internal("Salt do envelope invalido."))?;
    let nonce = BASE64
        .decode(&w.nonce)
        .map_err(|_| AppError::internal("Nonce do envelope invalido."))?;
    if nonce.len() != 12 {
        return Err(AppError::internal("Nonce do envelope com tamanho invalido."));
    }
    let wrapped = BASE64
        .decode(&w.wrapped)
        .map_err(|_| AppError::internal("Envelope invalido."))?;

    // Os parametros vem da linha, nao de constante — e o que permite abrir wraps
    // criados com custo menor depois de o padrao subir.
    let kek = derive_kek(secret, &salt, w.params)?;
    let cipher = Aes256Gcm::new_from_slice(kek.as_ref())
        .map_err(|_| AppError::internal("Falha ao criar cifra."))?;

    // O AAD e recalculado a partir do user_id de quem esta pedindo, e nao lido
    // do banco: se a linha foi movida, a decifra falha.
    let aad = aad_label(user_id, w.slot);
    let plain = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload { msg: wrapped.as_ref(), aad: aad.as_bytes() },
        )
        .map_err(|_| AppError::unauthorized("Nao foi possivel abrir a chave com este segredo."))?;

    if plain.len() != 32 {
        return Err(AppError::internal("Chave desembrulhada com tamanho invalido."));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&plain);
    Ok(Dek::from_bytes(k))
}

// ─── Persistencia (banco de autenticacao) ───────────────────────────────────

/// Papel de uma DEK. Durante a rotacao as duas coexistem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DekRole {
    Current,
    Retiring,
}

impl DekRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            DekRole::Current => "current",
            DekRole::Retiring => "retiring",
        }
    }
}

/// De onde a DEK veio. `LegacyPepperV1` marca a chave derivada do pepper,
/// registrada como ponto de partida da migracao — e o que permite saber que
/// aquele usuario ainda nao rotacionou para uma chave propria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DekSource {
    Random,
    LegacyPepperV1,
}

impl DekSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DekSource::Random => "random",
            DekSource::LegacyPepperV1 => "legacy_pepper_v1",
        }
    }
}

/// Grava uma DEK e seus wraps numa transacao unica.
///
/// Transacao unica nao e detalhe: uma DEK sem nenhum wrap valido e uma chave
/// que ninguem consegue abrir, e os dados cifrados sob ela ficam perdidos para
/// sempre. Ou entram a linha da DEK e todos os wraps, ou nao entra nada.
pub async fn store_dek(
    auth_db: &sqlx::SqlitePool,
    user_id: &str,
    dek: &Dek,
    role: DekRole,
    source: DekSource,
    wraps: &[WrappedDek],
) -> Result<String, AppError> {
    if wraps.is_empty() {
        return Err(AppError::internal(
            "Recusando gravar uma DEK sem nenhum wrap: seria uma chave inabrivel.",
        ));
    }

    let dek_id = uuid::Uuid::new_v4().to_string();
    let mut tx = auth_db
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao iniciar gravacao da chave: {}", e)))?;

    sqlx::query(
        "INSERT INTO user_deks (id, user_id, dek_check, role, source) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&dek_id)
    .bind(user_id)
    .bind(dek.check())
    .bind(role.as_str())
    .bind(source.as_str())
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao gravar chave: {}", e)))?;

    for w in wraps {
        insert_wrap(&mut tx, &dek_id, w).await?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao concluir gravacao da chave: {}", e)))?;
    Ok(dek_id)
}

async fn insert_wrap(
    tx: &mut sqlx::SqliteConnection,
    dek_id: &str,
    w: &WrappedDek,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT OR REPLACE INTO dek_wraps
           (dek_id, slot, kdf, m_cost, t_cost, p_cost, salt, nonce, wrapped, aad_label)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(dek_id)
    .bind(w.slot.as_str())
    .bind(&w.kdf)
    .bind(w.params.m_cost as i64)
    .bind(w.params.t_cost as i64)
    .bind(w.params.p_cost as i64)
    .bind(&w.salt)
    .bind(&w.nonce)
    .bind(&w.wrapped)
    .bind(&w.aad_label)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao gravar envelope: {}", e)))?;
    Ok(())
}

/// Uma linha de `dek_wraps` como o SQLite a devolve.
/// Nomear o tipo evita uma tupla de nove campos no meio da query.
type WrapRow = (String, String, i64, i64, i64, String, String, String, String);

/// Uma DEK do usuario, com seus wraps, como esta no banco.
pub struct StoredDek {
    pub id: String,
    pub dek_check: String,
    pub role: DekRole,
    pub source: DekSource,
    pub wraps: Vec<WrappedDek>,
}

/// Le as DEKs do usuario, da mais atual para a que esta saindo.
pub async fn load_deks(
    auth_db: &sqlx::SqlitePool,
    user_id: &str,
) -> Result<Vec<StoredDek>, AppError> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        r#"SELECT id, dek_check, role, source FROM user_deks
           WHERE user_id = ? ORDER BY (role = 'current') DESC, created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(auth_db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler chaves: {}", e)))?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, dek_check, role, source) in rows {
        let wraps: Vec<WrapRow> =
            sqlx::query_as(
                r#"SELECT slot, kdf, m_cost, t_cost, p_cost, salt, nonce, wrapped, aad_label
                   FROM dek_wraps WHERE dek_id = ?"#,
            )
            .bind(&id)
            .fetch_all(auth_db)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao ler envelopes: {}", e)))?;

        out.push(StoredDek {
            id,
            dek_check,
            role: if role == "current" { DekRole::Current } else { DekRole::Retiring },
            source: if source == "legacy_pepper_v1" {
                DekSource::LegacyPepperV1
            } else {
                DekSource::Random
            },
            wraps: wraps
                .into_iter()
                .filter_map(|(slot, kdf, m, t, p, salt, nonce, wrapped, aad_label)| {
                    Some(WrappedDek {
                        slot: Slot::parse(&slot)?,
                        kdf,
                        params: KdfParams {
                            m_cost: m as u32,
                            t_cost: t as u32,
                            p_cost: p as u32,
                        },
                        salt,
                        nonce,
                        wrapped,
                        aad_label,
                    })
                })
                .collect(),
        });
    }
    Ok(out)
}

/// Abre a DEK atual do usuario com um segredo, tentando os slots informados.
///
/// `slots` existe porque o codigo de recuperacao vale para dois: apos um reset,
/// `recovery_prev` continua aceito ate o usuario confirmar que anotou o novo.
pub async fn unwrap_current(
    auth_db: &sqlx::SqlitePool,
    user_id: &str,
    secret: &str,
    slots: &[Slot],
) -> Result<Dek, AppError> {
    let deks = load_deks(auth_db, user_id).await?;
    let atual = deks
        .iter()
        .find(|d| d.role == DekRole::Current)
        .ok_or_else(|| AppError::unauthorized("Este usuario ainda nao tem chave envelopada."))?;

    for slot in slots {
        if let Some(w) = atual.wraps.iter().find(|w| w.slot == *slot) {
            if let Ok(dek) = unwrap_dek(w, secret, user_id) {
                // O `dek_check` gravado tem de bater com a chave que saiu: se
                // divergir, a linha foi adulterada e seguir usaria a chave errada
                // para cifrar dado novo.
                if dek.check() != atual.dek_check {
                    return Err(AppError::internal(
                        "A chave aberta nao corresponde ao registro. Envelope inconsistente.",
                    ));
                }
                return Ok(dek);
            }
        }
    }
    Err(AppError::unauthorized(
        "Nao foi possivel abrir a chave com este segredo.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID: &str = "550e8400-e29b-41d4-a716-446655440001";
    const OUTRO_UID: &str = "550e8400-e29b-41d4-a716-446655440002";

    /// Parametros baratos: os de producao levam ~300 ms cada, e os testes fazem
    /// dezenas de derivacoes.
    const RAPIDO: KdfParams = KdfParams { m_cost: 64, t_cost: 1, p_cost: 1 };

    #[test]
    fn embrulha_e_desembrulha_a_mesma_chave() {
        let dek = Dek::generate();
        let w = wrap_dek(&dek, "senha-correta", UID, Slot::Password, RAPIDO).unwrap();
        let aberta = unwrap_dek(&w, "senha-correta", UID).unwrap();
        assert_eq!(aberta.expose(), dek.expose());
        assert_eq!(aberta.check(), dek.check());
    }

    #[test]
    fn segredo_errado_nao_abre() {
        let dek = Dek::generate();
        let w = wrap_dek(&dek, "senha-correta", UID, Slot::Password, RAPIDO).unwrap();
        assert!(unwrap_dek(&w, "senha-errada", UID).is_err());
    }

    /// O AAD e o que impede mover um wrap de lugar. Sem ele, quem tivesse
    /// escrita no banco poderia promover o wrap de recuperacao a wrap de senha,
    /// ou levar o wrap de um usuario para a linha de outro.
    #[test]
    fn wrap_movido_para_outro_slot_nao_abre() {
        let dek = Dek::generate();
        let w = wrap_dek(&dek, "segredo", UID, Slot::Recovery, RAPIDO).unwrap();

        let mut falsificado = w.clone();
        falsificado.slot = Slot::Password;
        assert!(
            unwrap_dek(&falsificado, "segredo", UID).is_err(),
            "mover o wrap entre slots tem de invalidar a decifra"
        );
    }

    #[test]
    fn wrap_de_outro_usuario_nao_abre() {
        let dek = Dek::generate();
        let w = wrap_dek(&dek, "segredo", UID, Slot::Password, RAPIDO).unwrap();
        assert!(
            unwrap_dek(&w, "segredo", OUTRO_UID).is_err(),
            "o wrap nao pode abrir sob outro user_id"
        );
    }

    /// Os parametros tem de sair da linha. Se o codigo lesse de constante, subir
    /// o custo padrao tornaria ilegivel todo wrap criado antes.
    #[test]
    fn abre_wrap_criado_com_parametros_menores() {
        let dek = Dek::generate();
        let antigos = KdfParams { m_cost: 32, t_cost: 1, p_cost: 1 };
        let w = wrap_dek(&dek, "segredo", UID, Slot::Password, antigos).unwrap();

        assert_ne!(w.params, KdfParams::PASSWORD, "o teste precisa de params diferentes do padrao");
        let aberta = unwrap_dek(&w, "segredo", UID).unwrap();
        assert_eq!(aberta.expose(), dek.expose());
    }

    #[test]
    fn cada_wrap_usa_salt_e_nonce_proprios() {
        let dek = Dek::generate();
        let a = wrap_dek(&dek, "segredo", UID, Slot::Password, RAPIDO).unwrap();
        let b = wrap_dek(&dek, "segredo", UID, Slot::Password, RAPIDO).unwrap();
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.wrapped, b.wrapped, "o mesmo segredo nao pode gerar o mesmo ciphertext");
        // Mas os dois abrem a mesma DEK.
        assert_eq!(unwrap_dek(&a, "segredo", UID).unwrap().check(), dek.check());
        assert_eq!(unwrap_dek(&b, "segredo", UID).unwrap().check(), dek.check());
    }

    #[test]
    fn check_identifica_a_chave_sem_revelar_material() {
        let a = Dek::generate();
        let b = Dek::generate();
        assert_ne!(a.check(), b.check());
        assert_eq!(a.check(), Dek::from_bytes(*a.expose()).check());
        // 16 bytes em base64 = 24 caracteres, e nada do material aparece.
        assert_eq!(a.check().len(), 24);
        assert!(!BASE64.decode(a.check()).unwrap().starts_with(&a.expose()[..4]));
    }

    #[test]
    fn envelope_corrompido_nao_abre() {
        let dek = Dek::generate();
        let mut w = wrap_dek(&dek, "segredo", UID, Slot::Password, RAPIDO).unwrap();
        let mut bytes = BASE64.decode(&w.wrapped).unwrap();
        bytes[0] ^= 0xFF;
        w.wrapped = BASE64.encode(&bytes);
        assert!(unwrap_dek(&w, "segredo", UID).is_err());
    }

    // ── Persistencia ────────────────────────────────────────────────────────

    async fn auth_db_de_teste() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.db");
        let url = format!("sqlite:{}?mode=rwc", path.to_string_lossy());
        let pool = crate::db::init_auth_database(&url).await.unwrap();
        sqlx::query(
            "INSERT INTO auth_users (id, email, password_hash, recovery_secret_hash, full_name) \
             VALUES (?, 'e@e.invalid', 'hash', 'rec', 'Nome')",
        )
        .bind(UID)
        .execute(&pool)
        .await
        .unwrap();
        (dir, pool)
    }

    #[tokio::test]
    async fn grava_e_abre_a_dek_pelos_dois_slots() {
        let (_dir, db) = auth_db_de_teste().await;
        let dek = Dek::generate();
        let wraps = vec![
            wrap_dek(&dek, "senha-da-conta", UID, Slot::Password, RAPIDO).unwrap(),
            wrap_dek(&dek, "CODIGO-DE-RECUPERACAO", UID, Slot::Recovery, RAPIDO).unwrap(),
        ];
        store_dek(&db, UID, &dek, DekRole::Current, DekSource::Random, &wraps)
            .await
            .unwrap();

        let pela_senha = unwrap_current(&db, UID, "senha-da-conta", &[Slot::Password])
            .await
            .unwrap();
        assert_eq!(pela_senha.check(), dek.check());

        let pelo_codigo = unwrap_current(&db, UID, "CODIGO-DE-RECUPERACAO", &[Slot::Recovery])
            .await
            .unwrap();
        assert_eq!(pelo_codigo.check(), dek.check(), "esquecer a senha nao pode perder os dados");

        assert!(
            unwrap_current(&db, UID, "senha-errada", &[Slot::Password]).await.is_err()
        );
        // O segredo certo no slot errado nao abre.
        assert!(
            unwrap_current(&db, UID, "senha-da-conta", &[Slot::Recovery]).await.is_err()
        );
    }

    /// Uma DEK sem wrap e uma chave que ninguem abre, e os dados cifrados sob
    /// ela ficam perdidos. Gravar isso tem de ser recusado, nao aceito.
    #[tokio::test]
    async fn recusa_gravar_dek_sem_nenhum_wrap() {
        let (_dir, db) = auth_db_de_teste().await;
        let dek = Dek::generate();
        assert!(
            store_dek(&db, UID, &dek, DekRole::Current, DekSource::Random, &[])
                .await
                .is_err()
        );
        assert!(load_deks(&db, UID).await.unwrap().is_empty(), "nada pode ter sido gravado");
    }

    #[tokio::test]
    async fn os_parametros_do_kdf_voltam_do_banco_e_nao_da_constante() {
        let (_dir, db) = auth_db_de_teste().await;
        let dek = Dek::generate();
        let antigos = KdfParams { m_cost: 48, t_cost: 2, p_cost: 1 };
        let w = wrap_dek(&dek, "segredo", UID, Slot::Password, antigos).unwrap();
        store_dek(&db, UID, &dek, DekRole::Current, DekSource::Random, &[w]).await.unwrap();

        let lidos = load_deks(&db, UID).await.unwrap();
        assert_eq!(lidos[0].wraps[0].params, antigos);
        assert_ne!(lidos[0].wraps[0].params, KdfParams::PASSWORD);
        // E o wrap continua abrindo com os parametros dele.
        assert_eq!(
            unwrap_current(&db, UID, "segredo", &[Slot::Password]).await.unwrap().check(),
            dek.check()
        );
    }

    /// `dek_check` divergente significa envelope adulterado. Seguir em frente
    /// cifraria dado novo com uma chave que nao e a registrada.
    #[tokio::test]
    async fn dek_check_divergente_e_recusado() {
        let (_dir, db) = auth_db_de_teste().await;
        let dek = Dek::generate();
        let w = wrap_dek(&dek, "segredo", UID, Slot::Password, RAPIDO).unwrap();
        store_dek(&db, UID, &dek, DekRole::Current, DekSource::Random, &[w]).await.unwrap();

        sqlx::query("UPDATE user_deks SET dek_check = 'outro-valor-qualquer' WHERE user_id = ?")
            .bind(UID)
            .execute(&db)
            .await
            .unwrap();

        assert!(unwrap_current(&db, UID, "segredo", &[Slot::Password]).await.is_err());
    }

    #[tokio::test]
    async fn so_pode_existir_uma_dek_current_por_usuario() {
        let (_dir, db) = auth_db_de_teste().await;
        let a = Dek::generate();
        let wa = wrap_dek(&a, "s", UID, Slot::Password, RAPIDO).unwrap();
        store_dek(&db, UID, &a, DekRole::Current, DekSource::LegacyPepperV1, &[wa])
            .await
            .unwrap();

        let b = Dek::generate();
        let wb = wrap_dek(&b, "s", UID, Slot::Password, RAPIDO).unwrap();
        assert!(
            store_dek(&db, UID, &b, DekRole::Current, DekSource::Random, &[wb.clone()])
                .await
                .is_err(),
            "o indice unico parcial deve impedir duas DEKs 'current'"
        );

        // Como 'retiring', conviver e o esperado durante a rotacao.
        sqlx::query("UPDATE user_deks SET role = 'retiring' WHERE user_id = ?")
            .bind(UID)
            .execute(&db)
            .await
            .unwrap();
        store_dek(&db, UID, &b, DekRole::Current, DekSource::Random, &[wb])
            .await
            .unwrap();
        let deks = load_deks(&db, UID).await.unwrap();
        assert_eq!(deks.len(), 2);
        assert_eq!(deks[0].role, DekRole::Current, "a atual vem primeiro");
        assert_eq!(deks[0].source, DekSource::Random);
        assert_eq!(deks[1].role, DekRole::Retiring);
        assert_eq!(deks[1].source, DekSource::LegacyPepperV1);
    }

    // ── Bootstrap: o caminho de quem ja usava o app ─────────────────────────

    #[tokio::test]
    async fn bootstrap_cria_envelope_sem_mudar_a_chave_de_dados() {
        let (_dir, db) = auth_db_de_teste().await;
        crate::crypto::set_pepper(&[0x5au8; 32]);

        // Chave que o usuario "ja tinha", derivada do pepper.
        let legada = crate::crypto::derive_user_key(UID).unwrap();
        assert!(load_deks(&db, UID).await.unwrap().is_empty(), "comeca sem envelope");

        crate::crypto::unlock_user_crypto(&db, UID, "senha-do-usuario", None)
            .await
            .unwrap();

        // A chave em uso e EXATAMENTE a antiga: nenhum dado precisou ser
        // re-cifrado nesta etapa.
        assert_eq!(
            crate::crypto::load_key(UID).unwrap(),
            legada,
            "a chave de dados nao pode mudar no bootstrap"
        );

        let deks = load_deks(&db, UID).await.unwrap();
        assert_eq!(deks.len(), 1);
        assert_eq!(deks[0].role, DekRole::Current);
        assert_eq!(
            deks[0].source,
            DekSource::LegacyPepperV1,
            "tem de ficar marcada como legada, e o que permite a rotacao depois"
        );
        // Sem o codigo de recuperacao em claro, so o wrap de senha nasce.
        assert_eq!(deks[0].wraps.len(), 1);
        assert_eq!(deks[0].wraps[0].slot, Slot::Password);

        // E a senha agora abre a chave pelo envelope.
        assert_eq!(
            unwrap_current(&db, UID, "senha-do-usuario", &[Slot::Password])
                .await
                .unwrap()
                .expose(),
            &legada
        );
    }

    /// Um segundo login nao pode criar um envelope novo — o indice unico parcial
    /// recusaria uma segunda DEK 'current', e o login quebraria.
    #[tokio::test]
    async fn bootstrap_e_idempotente_entre_logins() {
        let (_dir, db) = auth_db_de_teste().await;
        crate::crypto::set_pepper(&[0x5au8; 32]);

        crate::crypto::unlock_user_crypto(&db, UID, "senha", None).await.unwrap();
        let primeiro = load_deks(&db, UID).await.unwrap();

        crate::crypto::unlock_user_crypto(&db, UID, "senha", None).await.unwrap();
        crate::crypto::unlock_user_crypto(&db, UID, "senha", None).await.unwrap();

        let depois = load_deks(&db, UID).await.unwrap();
        assert_eq!(depois.len(), 1, "nao pode acumular DEKs a cada login");
        assert_eq!(depois[0].id, primeiro[0].id, "e tem de ser a mesma linha");
    }

    /// Com o codigo de recuperacao em maos — o caso do register — os dois wraps
    /// nascem juntos, e esquecer a senha deixa de ser perda de dados.
    #[tokio::test]
    async fn com_codigo_de_recuperacao_nascem_os_dois_wraps() {
        let (_dir, db) = auth_db_de_teste().await;
        crate::crypto::set_pepper(&[0x5au8; 32]);

        crate::crypto::unlock_user_crypto(&db, UID, "senha", Some("AAAA-BBBB-CCCC-DDDD"))
            .await
            .unwrap();

        let deks = load_deks(&db, UID).await.unwrap();
        assert_eq!(deks[0].wraps.len(), 2);
        let slots: Vec<_> = deks[0].wraps.iter().map(|w| w.slot).collect();
        assert!(slots.contains(&Slot::Password));
        assert!(slots.contains(&Slot::Recovery));

        // O codigo abre a chave sem a senha.
        assert!(
            unwrap_current(&db, UID, "AAAA-BBBB-CCCC-DDDD", &[Slot::Recovery])
                .await
                .is_ok()
        );
    }

    /// Nesta etapa a senha errada NAO bloqueia o acesso, e isso e deliberado:
    /// enquanto a DEK e a legada, o pepper ainda a reconstroi, e recusar aqui
    /// transformaria um envelope inconsistente em perda de acesso durante a
    /// transicao. O teste registra esse contrato para que a mudanca de
    /// comportamento na rotacao seja uma decisao consciente, e nao um acidente.
    #[tokio::test]
    async fn enquanto_a_dek_e_legada_o_pepper_ainda_e_o_fallback() {
        let (_dir, db) = auth_db_de_teste().await;
        crate::crypto::set_pepper(&[0x5au8; 32]);
        let legada = crate::crypto::derive_user_key(UID).unwrap();

        crate::crypto::unlock_user_crypto(&db, UID, "senha-certa", None).await.unwrap();
        crate::crypto::clear_user_crypto(UID);

        crate::crypto::unlock_user_crypto(&db, UID, "senha-ERRADA", None)
            .await
            .expect("com DEK legada, o fallback do pepper mantem o acesso");
        assert_eq!(crate::crypto::load_key(UID).unwrap(), legada);

        // Mas se a DEK for aleatoria (pos-rotacao), o pepper nao serve e a senha
        // errada passa a bloquear de verdade.
        sqlx::query("UPDATE user_deks SET source = 'random' WHERE user_id = ?")
            .bind(UID)
            .execute(&db)
            .await
            .unwrap();
        crate::crypto::clear_user_crypto(UID);
        assert!(
            crate::crypto::unlock_user_crypto(&db, UID, "senha-ERRADA", None).await.is_err(),
            "com DEK aleatoria, a senha passa a ser indispensavel"
        );
        // E a senha certa continua abrindo.
        assert!(
            crate::crypto::unlock_user_crypto(&db, UID, "senha-certa", None).await.is_ok()
        );
    }

    /// Nao e um teste de correcao: mede quanto o usuario espera no login.
    /// `cargo test --release --lib custo_do_kdf -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn custo_do_kdf() {
        let dek = Dek::generate();
        for (nome, p) in [("senha", KdfParams::PASSWORD), ("recuperacao", KdfParams::RECOVERY)] {
            let t = std::time::Instant::now();
            let w = wrap_dek(&dek, "segredo-de-teste", UID, Slot::Password, p).unwrap();
            let embrulhar = t.elapsed();
            let t = std::time::Instant::now();
            unwrap_dek(&w, "segredo-de-teste", UID).unwrap();
            println!(
                "  {:12} m={:>7} KiB t={}  embrulhar {:>7.0?}  desembrulhar {:>7.0?}",
                nome, p.m_cost, p.t_cost, embrulhar, t.elapsed()
            );
        }
    }

    #[test]
    fn aad_label_tem_o_formato_esperado() {
        assert_eq!(
            aad_label(UID, Slot::Password),
            format!("atendemente:dek:v2|{}|password", UID)
        );
        assert_eq!(Slot::parse("recovery_prev"), Some(Slot::RecoveryPrev));
        assert_eq!(Slot::parse("inexistente"), None);
    }
}
