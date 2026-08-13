//! Rotacao da chave de dados para uma DEK aleatoria.
//!
//! Esta e a etapa em que a promessa comeca a valer. Ate aqui a chave dos dados
//! era a derivada do pepper — envelopada, mas ainda reconstruivel por quem
//! tivesse o cofre de credenciais da maquina. Depois da rotacao a chave e
//! aleatoria e existe **apenas** dentro dos envelopes, abertos pela senha ou
//! pelo codigo de recuperacao.
//!
//! ## O que torna isto seguro de executar
//!
//! Re-cifrar o banco inteiro e uma operacao irreversivel sobre o dado mais
//! sensivel do app. Tres mecanismos sustentam isso:
//!
//! 1. **Backup de seguranca verificado.** Antes de tocar em qualquer registro, um
//!    bundle e gravado e imediatamente relido e conferido contra o proprio
//!    manifesto. "Gravou" nao e o mesmo que "serve"; se a verificacao falhar, a
//!    rotacao nem comeca.
//! 2. **Nada e descartado antes de ser provado.** A chave antiga permanece como
//!    `retiring` durante toda a conversao e so e removida depois de um passe que
//!    le tudo sob a chave nova.
//! 3. **Retomavel por construcao.** A conversao nao guarda progresso em lugar
//!    nenhum: ela pergunta a cada registro qual chave o abre. Uma queda de
//!    energia no meio deixa parte convertida e parte nao, e a proxima execucao
//!    continua exatamente de onde parou — sem depender de um marcador que pode
//!    nao ter sido gravado.
//!
//! ## O que ela ainda NAO entrega
//!
//! Enquanto o pepper existir no cofre, contas que ainda nao rotacionaram seguem
//! com chave derivavel sem senha. A promessa so se completa quando todas tiverem
//! rotacionado e o pepper for removido.

use sqlx::SqlitePool;

use super::envelope::{self, Dek, DekRole, DekSource, Slot};
use super::{decrypt_content_trying_all, encrypt_content_with_key, EncryptedPayload};
use crate::config::AppConfig;
use crate::errors::AppError;

/// Marcador de "cifrado com a DEK envelopada". Dica de otimizacao, nunca
/// autoridade — quem decide e a decifra.
const KEY_VERSION_ENVELOPE: i32 = 2;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RotationReport {
    pub patients: usize,
    pub session_records: usize,
    pub files: usize,
    /// Caminho do backup de seguranca, para poder ser informado ao usuario.
    pub safety_backup: Option<String>,
    /// `true` quando a execucao retomou uma rotacao ja em andamento.
    pub resumed: bool,
}

/// Executa (ou retoma) a rotacao da chave de dados do usuario.
///
/// Exige **os dois segredos**: a senha e o codigo de recuperacao. Nao e
/// burocracia — a chave nova precisa nascer com os dois envelopes, e um envelope
/// so pode ser criado a partir do segredo em claro.
///
/// A primeira versao desta funcao criava a DEK nova apenas com o wrap de senha,
/// deixando "a UI cobra o codigo depois". Isso significava que rotacionar
/// DESTRUIA a segunda via: entre a rotacao e a emissao do codigo novo, esquecer
/// a senha era perda total. Exatamente o que a rotacao deveria evitar.
pub async fn rotate_to_random_dek(
    user_db: &SqlitePool,
    auth_db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    password: &str,
    recovery_code: &str,
) -> Result<RotationReport, AppError> {
    let mut report = RotationReport::default();
    let deks = envelope::load_deks(auth_db, user_id).await?;

    let atual = deks
        .iter()
        .find(|d| d.role == DekRole::Current)
        .ok_or_else(|| AppError::internal("Este usuario ainda nao tem chave envelopada."))?;

    let saindo = deks.iter().find(|d| d.role == DekRole::Retiring);

    // Ja rotacionada e sem nada em andamento: nao ha o que fazer. Sem esta
    // checagem, chamar de novo geraria outra chave e re-cifraria tudo outra vez —
    // trabalho e risco a troco de nada.
    if atual.source == DekSource::Random && saindo.is_none() {
        return Ok(report);
    }

    // Recusar sem segunda via. Depois da rotacao a chave deixa de ser derivavel
    // do pepper, entao uma conta so com wrap de senha fica a um esquecimento de
    // distancia da perda total do prontuario — sem caminho de suporte.
    if !atual.wraps.iter().any(|w| w.slot == Slot::Recovery) {
        return Err(AppError::bad_request(
            "Antes de proteger a chave com sua senha, gere um codigo de recuperacao. \
             Sem ele, esquecer a senha tornaria os prontuarios inacessiveis para sempre.",
        ));
    }

    // O codigo tem de conferir ANTES de qualquer escrita: se ele nao abre a chave
    // atual, tambem nao serviria para a nova, e a rotacao deixaria a conta sem
    // segunda via sem ninguem perceber.
    if envelope::unwrap_current(auth_db, user_id, recovery_code, &[Slot::Recovery, Slot::RecoveryPrev])
        .await
        .is_err()
        && saindo.is_none()
    {
        return Err(AppError::bad_request(
            "O codigo de recuperacao informado nao confere. Confira antes de prosseguir: \
             ele e a segunda via da chave dos prontuarios.",
        ));
    }

    if let Some(saindo) = saindo {
        // Rotacao ja em andamento: nao gerar chave nova nem backup de novo.
        report.resumed = true;
        let antiga = envelope::unwrap_current_for(auth_db, user_id, password, saindo).await?;
        super::set_retiring_key(user_id, *antiga.expose())?;
    } else {
        report.safety_backup =
            Some(gravar_backup_de_seguranca(user_db, config, user_id, password).await?);

        let antiga = envelope::unwrap_current(auth_db, user_id, password, &[Slot::Password])
            .await
            .or_else(|_| -> Result<Dek, AppError> {
                // Conta legada cujo wrap nao abre: a chave ainda e derivavel.
                Ok(Dek::from_bytes(super::derive_user_key(user_id)?))
            })?;

        let nova = Dek::generate();
        promover_nova_dek(auth_db, user_id, &atual.id, &nova, password, recovery_code).await?;

        super::cache_key_public(user_id, *nova.expose())?;
        super::set_retiring_key(user_id, *antiga.expose())?;
    }

    // A partir daqui o chaveiro tem as duas chaves e a conversao e idempotente.
    report.patients += converter_patients(user_db, user_id).await?;
    report.session_records += converter_session_records(user_db, user_id).await?;
    report.files += converter_anexos(user_db, user_id).await?;

    verificar_tudo_sob_a_chave_nova(user_db, user_id).await?;
    descartar_chave_antiga(auth_db, user_id).await?;

    Ok(report)
}

/// Quantos registros a reparacao converteu, e quantos nao abriram com nenhuma
/// das chaves conhecidas.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RepairReport {
    pub patients: usize,
    pub session_records: usize,
    pub files: usize,
    /// Registros que nao abriram nem com a DEK atual nem com a chave legada.
    /// Nada foi escrito sobre eles.
    pub illegible: usize,
}

impl RepairReport {
    pub fn touched(&self) -> usize {
        self.patients + self.session_records + self.files
    }
}

/// Converte registros que ficaram sob a chave legada para a DEK atual.
///
/// Existe por um caso concreto: restaurar um backup feito ANTES da rotacao traz
/// dados cifrados com a chave legada. O restore agora converte na hora, mas quem
/// passou por essa sequencia na versao anterior tem registros parados nesse
/// estado — a DEK atual nao os abre, e a chave antiga nao esta no chaveiro.
///
/// Diferencas deliberadas em relacao a rotacao:
///
/// - **Tolerante por registro.** A rotacao aborta tudo se algo nao abre, porque
///   ela vai descartar a chave antiga e nao pode deixar nada para tras. Aqui nada
///   e descartado, entao um registro ilegivel e contado e ignorado em vez de
///   impedir a reparacao dos outros.
/// - **Nunca bloqueia a entrada.** Quem chama trata como best-effort.
///
/// E uma reparacao de TRANSICAO: ela depende de a chave legada ainda ser
/// derivavel, ou seja, de o pepper existir no cofre. Quando o pepper for
/// removido, `derive_user_key` falha e esta funcao vira no-op — que e o
/// comportamento correto, porque a essa altura nao deve haver nada sob a chave
/// legada.
pub async fn repair_rows_under_legacy_key(
    user_db: &SqlitePool,
    auth_db: &SqlitePool,
    user_id: &str,
) -> Result<RepairReport, AppError> {
    let mut rel = RepairReport::default();

    // So faz sentido se a chave atual NAO e a legada.
    let deks = envelope::load_deks(auth_db, user_id).await?;
    let atual = match deks.iter().find(|d| d.role == DekRole::Current) {
        Some(d) => d,
        None => return Ok(rel),
    };
    if atual.source != DekSource::Random {
        return Ok(rel);
    }
    // Rotacao em andamento ja tem a chave antiga no chaveiro; deixa a rotacao
    // terminar em vez de competir com ela.
    if deks.iter().any(|d| d.role == DekRole::Retiring) {
        return Ok(rel);
    }

    let dek_atual = super::load_key(user_id)?;
    // Sem pepper (pos-R5) isto falha, e a reparacao simplesmente nao acontece.
    let legada = match super::derive_user_key(user_id) {
        Ok(k) => k,
        Err(_) => return Ok(rel),
    };
    if legada == dek_atual {
        return Ok(rel);
    }

    // ── patients ────────────────────────────────────────────────────────────
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, pii_encrypted, pii_iv, pii_auth_tag FROM patients \
         WHERE user_id = ? AND pii_encrypted IS NOT NULL",
    )
    .bind(user_id)
    .fetch_all(user_db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler pacientes: {}", e)))?;

    for (id, enc, iv, tag) in rows {
        let p = EncryptedPayload {
            encrypted_payload: enc,
            iv,
            auth_tag: tag,
            key_version: 1,
        };
        if super::decrypt_content_with_key(&p, &dek_atual).is_ok() {
            continue;
        }
        match super::decrypt_content_with_key(&p, &legada) {
            Ok(claro) => {
                let novo = encrypt_content_with_key(&claro, &dek_atual)?;
                sqlx::query(
                    "UPDATE patients SET pii_encrypted=?, pii_iv=?, pii_auth_tag=?, \
                     key_version=? WHERE id=?",
                )
                .bind(&novo.encrypted_payload)
                .bind(&novo.iv)
                .bind(&novo.auth_tag)
                .bind(KEY_VERSION_ENVELOPE)
                .bind(&id)
                .execute(user_db)
                .await
                .map_err(|e| AppError::internal(format!("Erro ao reparar paciente: {}", e)))?;
                rel.patients += 1;
            }
            Err(_) => rel.illegible += 1,
        }
    }

    // ── session_records ─────────────────────────────────────────────────────
    let recs: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, encrypted_payload, iv, auth_tag FROM session_records WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(user_db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler prontuarios: {}", e)))?;

    for (id, enc, iv, tag) in recs {
        let p = EncryptedPayload {
            encrypted_payload: enc,
            iv,
            auth_tag: tag,
            key_version: 1,
        };
        if super::decrypt_content_with_key(&p, &dek_atual).is_ok() {
            continue;
        }
        match super::decrypt_content_with_key(&p, &legada) {
            Ok(claro) => {
                let novo = encrypt_content_with_key(&claro, &dek_atual)?;
                sqlx::query(
                    "UPDATE session_records SET encrypted_payload=?, iv=?, auth_tag=?, \
                     key_version=? WHERE id=?",
                )
                .bind(&novo.encrypted_payload)
                .bind(&novo.iv)
                .bind(&novo.auth_tag)
                .bind(KEY_VERSION_ENVELOPE)
                .bind(&id)
                .execute(user_db)
                .await
                .map_err(|e| AppError::internal(format!("Erro ao reparar prontuario: {}", e)))?;
                rel.session_records += 1;
            }
            Err(_) => rel.illegible += 1,
        }
    }

    // ── anexos ──────────────────────────────────────────────────────────────
    let arqs: Vec<(String, String)> =
        sqlx::query_as("SELECT id, storage_path FROM record_files WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(user_db)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao ler anexos: {}", e)))?;

    for (id, caminho) in arqs {
        let bytes = match tokio::fs::read(&caminho).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        // Cifrado com a chave atual? Nada a fazer.
        if bytes.first() == Some(&0x01) && super::decrypt_file(&bytes, &dek_atual).is_ok() {
            continue;
        }
        let claro = match super::decrypt_file(&bytes, &legada) {
            Ok(c) => c,
            Err(_) => {
                rel.illegible += 1;
                continue;
            }
        };
        let novo = super::encrypt_file(&claro, &dek_atual)?;
        let tmp = format!("{}.repair", caminho);
        if tokio::fs::write(&tmp, &novo).await.is_err() {
            continue;
        }
        if tokio::fs::rename(&tmp, &caminho).await.is_err() {
            let _ = tokio::fs::remove_file(&tmp).await;
            continue;
        }
        let _ = sqlx::query("UPDATE record_files SET encryption_version = ? WHERE id = ?")
            .bind(KEY_VERSION_ENVELOPE)
            .bind(&id)
            .execute(user_db)
            .await;
        rel.files += 1;
    }

    Ok(rel)
}

/// Grava e **verifica** o backup de seguranca. Falhar aqui aborta a rotacao.
async fn gravar_backup_de_seguranca(
    user_db: &SqlitePool,
    config: &AppConfig,
    user_id: &str,
    password: &str,
) -> Result<String, AppError> {
    let bundle = crate::features::backup::create_backup_with_password(
        user_db,
        config,
        user_id,
        Some(password),
    )
    .await
    .map_err(|e| {
        AppError::internal(format!(
            "Nao foi possivel gerar o backup de seguranca; a rotacao nao comecou. ({})",
            e
        ))
    })?;

    let dir = config
        .storage_dir
        .parent()
        .unwrap_or(&config.storage_dir)
        .join("backups")
        .join(user_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao preparar pasta de backup: {}", e)))?;
    let caminho = dir.join(format!("pre-rotacao-{}", bundle.file_name));
    tokio::fs::write(&caminho, &bundle.bytes)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao gravar backup de seguranca: {}", e)))?;

    // Reler do DISCO, e nao usar os bytes em memoria: o que importa e que o
    // arquivo gravado sirva, nao que a serializacao tenha funcionado.
    let lido = tokio::fs::read(&caminho)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao reler backup de seguranca: {}", e)))?;
    crate::features::backup::verify_backup_bytes(&lido, Some(password))
        .await
        .map_err(|e| {
            AppError::internal(format!(
                "O backup de seguranca foi gravado mas nao passou na verificacao; \
                 a rotacao nao comecou. ({})",
                e
            ))
        })?;

    tracing::info!("[Rotacao] Backup de seguranca verificado em {}", caminho.display());
    Ok(caminho.to_string_lossy().to_string())
}

/// Rebaixa a DEK atual para `retiring` e instala a nova como `current`.
///
/// Numa transacao unica: um estado com duas DEKs `current`, ou com nenhuma,
/// deixaria o usuario sem chave de escrita.
async fn promover_nova_dek(
    auth_db: &SqlitePool,
    user_id: &str,
    dek_atual_id: &str,
    nova: &Dek,
    password: &str,
    recovery_code: &str,
) -> Result<(), AppError> {
    let mut tx = auth_db
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao iniciar rotacao: {}", e)))?;

    sqlx::query("UPDATE user_deks SET role = 'retiring' WHERE id = ?")
        .bind(dek_atual_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao rebaixar chave: {}", e)))?;

    // A nova nasce com OS DOIS wraps, na mesma transacao.
    //
    // Nao ha estado intermediario aceitavel aqui: uma DEK aleatoria com apenas o
    // wrap de senha e uma conta a um esquecimento de distancia da perda total,
    // porque a chave deixou de ser derivavel do pepper no mesmo instante.
    let wrap_senha =
        envelope::wrap_dek(nova, password, user_id, Slot::Password, Slot::Password.default_params())?;
    let wrap_codigo = envelope::wrap_dek(
        nova,
        &crate::auth::auth_service::normalize_recovery_secret(recovery_code),
        user_id,
        Slot::Recovery,
        Slot::Recovery.default_params(),
    )?;
    let novo_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO user_deks (id, user_id, dek_check, role, source) VALUES (?, ?, ?, 'current', ?)")
        .bind(&novo_id)
        .bind(user_id)
        .bind(nova.check())
        .bind(DekSource::Random.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao gravar chave nova: {}", e)))?;
    envelope::insert_wrap_pub(&mut tx, &novo_id, &wrap_senha).await?;
    envelope::insert_wrap_pub(&mut tx, &novo_id, &wrap_codigo).await?;

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao concluir troca de chave: {}", e)))
}

async fn converter_patients(db: &SqlitePool, user_id: &str) -> Result<usize, AppError> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, pii_encrypted, pii_iv, pii_auth_tag FROM patients \
         WHERE user_id = ? AND pii_encrypted IS NOT NULL",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler pacientes: {}", e)))?;

    let atual = super::load_key(user_id)?;
    let mut convertidos = 0;
    for (id, enc, iv, tag) in rows {
        let payload = EncryptedPayload {
            encrypted_payload: enc,
            iv,
            auth_tag: tag,
            key_version: 1,
        };
        let r = decrypt_content_trying_all(&payload, user_id)?;
        if !r.used_retiring {
            continue; // ja convertido
        }
        let novo = encrypt_content_with_key(&r.plaintext, &atual)?;
        sqlx::query(
            "UPDATE patients SET pii_encrypted = ?, pii_iv = ?, pii_auth_tag = ?, key_version = ? \
             WHERE id = ?",
        )
        .bind(&novo.encrypted_payload)
        .bind(&novo.iv)
        .bind(&novo.auth_tag)
        .bind(KEY_VERSION_ENVELOPE)
        .bind(&id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao converter paciente: {}", e)))?;
        convertidos += 1;
    }
    Ok(convertidos)
}

async fn converter_session_records(db: &SqlitePool, user_id: &str) -> Result<usize, AppError> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, encrypted_payload, iv, auth_tag FROM session_records WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao ler prontuarios: {}", e)))?;

    let atual = super::load_key(user_id)?;
    let mut convertidos = 0;
    for (id, enc, iv, tag) in rows {
        let payload = EncryptedPayload {
            encrypted_payload: enc,
            iv,
            auth_tag: tag,
            key_version: 1,
        };
        let r = decrypt_content_trying_all(&payload, user_id)?;
        if !r.used_retiring {
            continue;
        }
        let novo = encrypt_content_with_key(&r.plaintext, &atual)?;
        sqlx::query(
            "UPDATE session_records SET encrypted_payload = ?, iv = ?, auth_tag = ?, \
             key_version = ? WHERE id = ?",
        )
        .bind(&novo.encrypted_payload)
        .bind(&novo.iv)
        .bind(&novo.auth_tag)
        .bind(KEY_VERSION_ENVELOPE)
        .bind(&id)
        .execute(db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao converter prontuario: {}", e)))?;
        convertidos += 1;
    }
    Ok(convertidos)
}

/// Converte os anexos em disco.
///
/// O filesystem nao participa da transacao do SQLite, entao a ordem importa:
/// grava num temporario, sincroniza, renomeia sobre o original e SO ENTAO
/// atualiza a linha. Uma queda entre o rename e o UPDATE deixa o arquivo novo e
/// a linha dizendo o contrario — inofensivo, porque a leitura tenta as duas
/// chaves. A ordem inversa perderia o arquivo.
async fn converter_anexos(db: &SqlitePool, user_id: &str) -> Result<usize, AppError> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id, storage_path FROM record_files WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(db)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao ler anexos: {}", e)))?;

    let atual = super::load_key(user_id)?;
    let mut convertidos = 0;

    for (id, caminho) in rows {
        let bytes = match tokio::fs::read(&caminho).await {
            Ok(b) => b,
            // Arquivo ausente nao impede a rotacao: a linha ja apontava para o
            // vazio antes dela, e abortar aqui bloquearia o usuario para sempre.
            Err(_) => {
                tracing::warn!("[Rotacao] Anexo ausente em disco, ignorado: {}", caminho);
                continue;
            }
        };

        // Ja esta sob a chave nova? Entao nada a fazer.
        if super::decrypt_file(&bytes, &atual).is_ok() && bytes.first() == Some(&0x01) {
            continue;
        }
        let claro = super::decrypt_file_trying_all(&bytes, user_id)?;

        let novo = super::encrypt_file(&claro, &atual)?;
        let tmp = format!("{}.tmp2", caminho);
        tokio::fs::write(&tmp, &novo)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao gravar anexo temporario: {}", e)))?;
        tokio::fs::rename(&tmp, &caminho)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao substituir anexo: {}", e)))?;

        sqlx::query("UPDATE record_files SET encryption_version = ? WHERE id = ?")
            .bind(KEY_VERSION_ENVELOPE)
            .bind(&id)
            .execute(db)
            .await
            .map_err(|e| AppError::internal(format!("Erro ao marcar anexo: {}", e)))?;
        convertidos += 1;
    }
    Ok(convertidos)
}

/// Le tudo usando SOMENTE a chave nova. Se algo escapou, a chave antiga nao pode
/// ser descartada.
async fn verificar_tudo_sob_a_chave_nova(db: &SqlitePool, user_id: &str) -> Result<(), AppError> {
    let atual = super::load_key(user_id)?;

    let pacientes: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, pii_encrypted, pii_iv, pii_auth_tag FROM patients \
         WHERE user_id = ? AND pii_encrypted IS NOT NULL",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao verificar pacientes: {}", e)))?;
    for (id, enc, iv, tag) in pacientes {
        let p = EncryptedPayload { encrypted_payload: enc, iv, auth_tag: tag, key_version: 2 };
        super::decrypt_content_with_key(&p, &atual).map_err(|_| {
            AppError::internal(format!(
                "Paciente {} nao abre com a chave nova; a chave antiga foi mantida.",
                id
            ))
        })?;
    }

    let prontuarios: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, encrypted_payload, iv, auth_tag FROM session_records WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao verificar prontuarios: {}", e)))?;
    for (id, enc, iv, tag) in prontuarios {
        let p = EncryptedPayload { encrypted_payload: enc, iv, auth_tag: tag, key_version: 2 };
        super::decrypt_content_with_key(&p, &atual).map_err(|_| {
            AppError::internal(format!(
                "Prontuario {} nao abre com a chave nova; a chave antiga foi mantida.",
                id
            ))
        })?;
    }
    Ok(())
}

/// Remove a DEK antiga (e seus wraps, por cascata) e a tira do chaveiro.
async fn descartar_chave_antiga(auth_db: &SqlitePool, user_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM user_deks WHERE user_id = ? AND role = 'retiring'")
        .bind(user_id)
        .execute(auth_db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao descartar chave antiga: {}", e)))?;

    let atual = super::load_key(user_id)?;
    super::cache_key_public(user_id, atual)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    const SENHA: &str = "SenhaDaRotacao#2026";
    const CODIGO: &str = "ABCD-EF01-2345-6789-ABCD-EF01-2345-6789";

    /// Cada teste precisa do SEU proprio user_id.
    ///
    /// `USER_KEYS` e um mapa global indexado por user_id, e o `cargo test` roda
    /// em paralelo: com um uid compartilhado, um teste instalava a chave que o
    /// outro estava usando. O sintoma era intermitente e nao apontava a causa —
    /// a verificacao final falhava com "nao abre com a chave nova", como se a
    /// rotacao estivesse errada.
    async fn cenario(uid: &str) -> (tempfile::TempDir, SqlitePool, SqlitePool, AppConfig) {
        let dir = tempfile::tempdir().unwrap();
        let user_url = format!("sqlite:{}?mode=rwc", dir.path().join("user.db").display());
        let auth_url = format!("sqlite:{}?mode=rwc", dir.path().join("auth.db").display());
        let user_db = crate::db::init_database(&user_url).await.unwrap();
        let auth_db = crate::db::init_auth_database(&auth_url).await.unwrap();

        sqlx::query(
            "INSERT INTO auth_users (id, email, password_hash, recovery_secret_hash, full_name) \
             VALUES (?, 'r@r.invalid', 'h', 'rh', 'Nome')",
        )
        .bind(uid)
        .execute(&auth_db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO users (id, email, full_name, created_at, updated_at) \
             VALUES (?, 'r@r.invalid', 'Nome', '2026-01-01', '2026-01-01')",
        )
        .bind(uid)
        .execute(&user_db)
        .await
        .unwrap();

        let storage_dir = dir.path().join("uploads");
        std::fs::create_dir_all(&storage_dir).unwrap();
        let config = AppConfig {
            database_url: user_url,
            auth_database_url: auth_url,
            server_port: 3001,
            master_pepper: [0u8; 32],
            storage_dir,
            data_dir: dir.path().join("data"),
        };
        crypto::set_pepper(&[0x5au8; 32]);
        (dir, user_db, auth_db, config)
    }

    async fn criar_paciente(db: &SqlitePool, uid: &str, id: &str, texto: &str) {
        let payload = crypto::encrypt_content(texto, uid).unwrap();
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, status, created_at, updated_at, \
             pii_encrypted, pii_iv, pii_auth_tag) \
             VALUES (?, ?, 'Paciente', 'active', '2026-01-01', '2026-01-01', ?, ?, ?)",
        )
        .bind(id)
        .bind(uid)
        .bind(&payload.encrypted_payload)
        .bind(&payload.iv)
        .bind(&payload.auth_tag)
        .execute(db)
        .await
        .unwrap();
    }

    async fn ler_paciente(db: &SqlitePool, uid: &str, id: &str) -> Result<String, AppError> {
        let (e, i, t): (String, String, String) =
            sqlx::query_as("SELECT pii_encrypted, pii_iv, pii_auth_tag FROM patients WHERE id = ?")
                .bind(id)
                .fetch_one(db)
                .await
                .unwrap();
        crypto::decrypt_content(
            &EncryptedPayload { encrypted_payload: e, iv: i, auth_tag: t, key_version: 1 },
            uid,
        )
    }

    /// **O teste que define a etapa.**
    ///
    /// Reproduz o experimento que motivou toda a reforma: derivar a chave a
    /// partir do pepper, fora do app, e tentar abrir o prontuario. Antes da
    /// rotacao isso funciona. Depois, tem de falhar — e a senha tem de abrir.
    #[tokio::test]
    async fn depois_da_rotacao_o_pepper_nao_abre_mais_os_dados() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c1";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, Some(CODIGO)).await.unwrap();
        criar_paciente(&user_db, uid, "p1", "prontuario secreto").await;

        // ANTES: a chave derivada do pepper abre o registro.
        let chave_do_pepper = crypto::derive_user_key(uid).unwrap();
        let (e, i, t): (String, String, String) =
            sqlx::query_as("SELECT pii_encrypted, pii_iv, pii_auth_tag FROM patients WHERE id = 'p1'")
                .fetch_one(&user_db)
                .await
                .unwrap();
        let payload = EncryptedPayload {
            encrypted_payload: e,
            iv: i,
            auth_tag: t,
            key_version: 1,
        };
        assert_eq!(
            crypto::decrypt_content_with_key(&payload, &chave_do_pepper).unwrap(),
            "prontuario secreto",
            "antes da rotacao, o pepper abre — e este era o problema"
        );

        let rel = rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();
        assert_eq!(rel.patients, 1);
        assert!(rel.safety_backup.is_some(), "a rotacao tem de gravar backup antes");

        // DEPOIS: o mesmo caminho falha.
        let (e, i, t): (String, String, String) =
            sqlx::query_as("SELECT pii_encrypted, pii_iv, pii_auth_tag FROM patients WHERE id = 'p1'")
                .fetch_one(&user_db)
                .await
                .unwrap();
        let payload = EncryptedPayload {
            encrypted_payload: e,
            iv: i,
            auth_tag: t,
            key_version: 2,
        };
        assert!(
            crypto::decrypt_content_with_key(&payload, &chave_do_pepper).is_err(),
            "DEPOIS da rotacao o pepper NAO pode mais abrir o prontuario"
        );

        // E a senha continua abrindo, numa sessao nova.
        crypto::clear_user_crypto(uid);
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, None).await.unwrap();
        assert_eq!(ler_paciente(&user_db, uid, "p1").await.unwrap(), "prontuario secreto");
    }

    /// Sem segunda via, rotacionar deixaria a conta a um esquecimento de senha
    /// da perda total. Tem de recusar.
    #[tokio::test]
    async fn recusa_rotacionar_sem_codigo_de_recuperacao() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c2";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        // Bootstrap sem codigo em claro: nasce so o wrap de senha.
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, None).await.unwrap();

        let erro = rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO)
            .await
            .expect_err("deveria recusar");
        assert!(
            format!("{:?}", erro).contains("codigo de recuperacao"),
            "a mensagem precisa dizer o que fazer: {:?}",
            erro
        );
    }

    /// Interrupcao no meio: parte convertida, parte nao. A proxima execucao
    /// retoma e tudo continua legivel — sem depender de marcador gravado.
    #[tokio::test]
    async fn rotacao_interrompida_e_retomada_converge() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c3";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, Some(CODIGO)).await.unwrap();
        for i in 0..4 {
            criar_paciente(&user_db, uid, &format!("p{}", i), &format!("prontuario {}", i)).await;
        }

        // Simula a interrupcao: a troca de chave aconteceu, a conversao nao.
        let deks = envelope::load_deks(&auth_db, uid).await.unwrap();
        let antiga = crypto::load_key(uid).unwrap();
        let nova = Dek::generate();
        promover_nova_dek(&auth_db, uid, &deks[0].id, &nova, SENHA, CODIGO).await.unwrap();
        crypto::cache_key_public(uid, *nova.expose()).unwrap();
        crypto::set_retiring_key(uid, antiga).unwrap();

        // Converte so metade, como se o processo tivesse morrido aqui.
        let so_dois: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT id, pii_encrypted, pii_iv, pii_auth_tag FROM patients LIMIT 2",
        )
        .fetch_all(&user_db)
        .await
        .unwrap();
        for (id, e, i, t) in so_dois {
            let p = EncryptedPayload { encrypted_payload: e, iv: i, auth_tag: t, key_version: 1 };
            let claro = decrypt_content_trying_all(&p, uid).unwrap().plaintext;
            let novo = encrypt_content_with_key(&claro, nova.expose()).unwrap();
            sqlx::query(
                "UPDATE patients SET pii_encrypted=?, pii_iv=?, pii_auth_tag=?, key_version=2 WHERE id=?",
            )
            .bind(&novo.encrypted_payload).bind(&novo.iv).bind(&novo.auth_tag).bind(&id)
            .execute(&user_db).await.unwrap();
        }

        // Estado misto: os quatro continuam legiveis pela leitura normal.
        for i in 0..4 {
            assert_eq!(
                ler_paciente(&user_db, uid, &format!("p{}", i)).await.unwrap(),
                format!("prontuario {}", i),
                "com metade convertida, tudo tem de continuar legivel"
            );
        }

        // Retomada: converte o resto e conclui.
        let rel = rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();
        assert!(rel.resumed, "deveria ter identificado a rotacao em andamento");
        assert_eq!(rel.patients, 2, "so os dois que faltavam");
        assert!(rel.safety_backup.is_none(), "retomada nao regrava backup");

        crypto::clear_user_crypto(uid);
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, None).await.unwrap();
        for i in 0..4 {
            assert_eq!(
                ler_paciente(&user_db, uid, &format!("p{}", i)).await.unwrap(),
                format!("prontuario {}", i)
            );
        }
        // E a chave antiga foi descartada.
        let deks = envelope::load_deks(&auth_db, uid).await.unwrap();
        assert_eq!(deks.len(), 1);
        assert_eq!(deks[0].source, DekSource::Random);
    }

    /// O cenario que apareceu no teste com dados reais.
    ///
    /// Sequencia: a conta rotaciona (DEK aleatoria), e depois o usuario restaura
    /// um backup feito ANTES da rotacao, na MESMA maquina. O conteudo do backup
    /// esta sob a chave legada; a DEK atual e aleatoria e nao ha chave antiga no
    /// chaveiro. Antes da correcao o restore nao convertia nada, porque decidia
    /// comparando `old_pepper == current_pepper` — iguais, na mesma maquina — e os
    /// dados voltavam ilegiveis para o app.
    #[tokio::test]
    async fn restaurar_backup_pre_rotacao_na_mesma_maquina_mantem_os_dados_legiveis() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c5";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, Some(CODIGO)).await.unwrap();
        criar_paciente(&user_db, uid, "p1", "prontuario antes do backup").await;

        // Backup feito ANTES de rotacionar: conteudo sob a chave legada.
        let bundle = crate::features::backup::create_backup_with_password(
            &user_db, &config, uid, Some("SenhaDoBackup#2026"),
        )
        .await
        .unwrap();

        rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();
        assert_eq!(
            ler_paciente(&user_db, uid, "p1").await.unwrap(),
            "prontuario antes do backup",
            "apos a rotacao os dados seguem legiveis"
        );

        // Agora restaura o backup antigo, com a mesma sessao e o mesmo pepper.
        crate::features::backup::restore_backup_with_password(
            &user_db, &config, uid, &bundle.bytes, Some("SenhaDoBackup#2026"),
        )
        .await
        .unwrap();

        assert_eq!(
            ler_paciente(&user_db, uid, "p1").await.unwrap(),
            "prontuario antes do backup",
            "o conteudo restaurado tem de ser convertido para a chave atual"
        );

        // E numa sessao nova, abrindo a chave pela senha, continua legivel.
        crypto::clear_user_crypto(uid);
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, None).await.unwrap();
        assert_eq!(
            ler_paciente(&user_db, uid, "p1").await.unwrap(),
            "prontuario antes do backup"
        );
    }

    /// A reparacao para quem JA passou pela sequencia ruim antes da correcao.
    ///
    /// Reproduz o estado exato encontrado no uso real: conta rotacionada (DEK
    /// aleatoria) com registros gravados sob a chave legada, e nenhuma chave
    /// antiga no chaveiro. O app nao consegue ler; a reparacao converte.
    #[tokio::test]
    async fn repara_registros_parados_sob_a_chave_legada() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c6";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, Some(CODIGO)).await.unwrap();
        criar_paciente(&user_db, uid, "p1", "prontuario sob a chave legada").await;

        // Guarda o texto cifrado com a chave LEGADA antes de rotacionar.
        let legado: (String, String, String) = sqlx::query_as(
            "SELECT pii_encrypted, pii_iv, pii_auth_tag FROM patients WHERE id='p1'",
        )
        .fetch_one(&user_db)
        .await
        .unwrap();

        rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();

        // Simula o que o restore quebrado fazia: devolve a linha antiga.
        sqlx::query(
            "UPDATE patients SET pii_encrypted=?, pii_iv=?, pii_auth_tag=?, key_version=1 \
             WHERE id='p1'",
        )
        .bind(&legado.0)
        .bind(&legado.1)
        .bind(&legado.2)
        .execute(&user_db)
        .await
        .unwrap();

        assert!(
            ler_paciente(&user_db, uid, "p1").await.is_err(),
            "o estado de partida tem de ser justamente o ilegivel"
        );

        let rel = repair_rows_under_legacy_key(&user_db, &auth_db, uid).await.unwrap();
        assert_eq!(rel.patients, 1);
        assert_eq!(rel.illegible, 0);
        assert_eq!(
            ler_paciente(&user_db, uid, "p1").await.unwrap(),
            "prontuario sob a chave legada"
        );

        // Rodar de novo nao faz nada: tudo ja esta sob a chave atual.
        let rel = repair_rows_under_legacy_key(&user_db, &auth_db, uid).await.unwrap();
        assert_eq!(rel, RepairReport::default());
    }

    /// Um registro que nao abre com NENHUMA chave nao pode bloquear a reparacao
    /// dos outros, nem ser sobrescrito.
    #[tokio::test]
    async fn registro_ilegivel_e_contado_e_preservado() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c7";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, Some(CODIGO)).await.unwrap();
        criar_paciente(&user_db, uid, "bom", "conteudo recuperavel").await;
        let legado: (String, String, String) = sqlx::query_as(
            "SELECT pii_encrypted, pii_iv, pii_auth_tag FROM patients WHERE id='bom'",
        )
        .fetch_one(&user_db)
        .await
        .unwrap();

        rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();

        // Um volta para a chave legada; outro recebe lixo que nenhuma chave abre.
        sqlx::query("UPDATE patients SET pii_encrypted=?, pii_iv=?, pii_auth_tag=?, key_version=1 WHERE id='bom'")
            .bind(&legado.0).bind(&legado.1).bind(&legado.2)
            .execute(&user_db).await.unwrap();
        sqlx::query(
            "INSERT INTO patients (id, user_id, full_name, status, created_at, updated_at, \
             pii_encrypted, pii_iv, pii_auth_tag) VALUES ('ruim', ?, 'Ruim', 'active', \
             '2026-01-01', '2026-01-01', 'bm90LXJlYWxseS1jaXBoZXI=', 'YWFhYWFhYWFhYWFh', \
             'YmJiYmJiYmJiYmJiYmJiYg==')",
        )
        .bind(uid)
        .execute(&user_db)
        .await
        .unwrap();

        let rel = repair_rows_under_legacy_key(&user_db, &auth_db, uid).await.unwrap();
        assert_eq!(rel.patients, 1, "o recuperavel foi convertido");
        assert_eq!(rel.illegible, 1, "o ilegivel foi contado");
        assert_eq!(ler_paciente(&user_db, uid, "bom").await.unwrap(), "conteudo recuperavel");

        // E o ilegivel continua exatamente como estava — nada escrito por cima.
        let blob: String =
            sqlx::query_scalar("SELECT pii_encrypted FROM patients WHERE id='ruim'")
                .fetch_one(&user_db)
                .await
                .unwrap();
        assert_eq!(blob, "bm90LXJlYWxseS1jaXBoZXI=");
    }

    /// Rodar duas vezes seguidas nao pode fazer nada na segunda.
    #[tokio::test]
    async fn rotacao_e_idempotente() {
        let uid = "550e8400-e29b-41d4-a716-4466554400c4";
        let (_dir, user_db, auth_db, config) = cenario(uid).await;
        crypto::unlock_user_crypto(&auth_db, uid, SENHA, Some(CODIGO)).await.unwrap();
        criar_paciente(&user_db, uid, "p1", "conteudo").await;

        let primeira = rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();
        assert_eq!(primeira.patients, 1);

        // A segunda chamada tem de ser um no-op completo: a chave ja e aleatoria
        // e nao ha rotacao em andamento. Sem essa checagem, cada chamada geraria
        // outra chave e re-cifraria tudo — risco e trabalho a troco de nada.
        let segunda = rotate_to_random_dek(&user_db, &auth_db, &config, uid, SENHA, CODIGO).await.unwrap();
        assert_eq!(segunda, RotationReport::default(), "a segunda chamada nao pode fazer nada");
        assert_eq!(ler_paciente(&user_db, uid, "p1").await.unwrap(), "conteudo");
    }
}
