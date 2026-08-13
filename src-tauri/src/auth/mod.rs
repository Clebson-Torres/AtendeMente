pub mod auth_service;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;

use crate::audit::{self, AuditAction};
use crate::errors::{ActionResponse, AppError};
use crate::AppState;

pub fn create_auth_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
        .route("/auth/recover", post(recover_handler))
        .route("/auth/reset-password", post(reset_password_handler))
        .route("/auth/lock", post(lock_handler))
        .route("/auth/unlock", post(unlock_handler))
        .route("/auth/onboarding", patch(onboarding_handler))
        .route("/auth/recovery-code/rotate", post(rotate_recovery_code_handler))
        .route("/auth/recovery-code/ack", post(ack_recovery_code_handler))
        .route("/auth/rotate-key", post(rotate_data_key_handler))
        .with_state(state)
}

#[derive(Deserialize)]
struct RotateKeyInput {
    password: String,
    recovery_code: String,
}

#[derive(Deserialize)]
struct RegisterInput {
    email: String,
    password: String,
    full_name: String,
}

#[derive(Deserialize)]
struct LoginInput {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct RecoverInput {
    user_id: Option<String>,
    email: Option<String>,
    recovery_secret: String,
}

#[derive(Deserialize)]
struct ResetPasswordInput {
    reset_token: String,
    new_password: String,
}

#[derive(Deserialize)]
struct UnlockInput {
    password: String,
}

async fn register_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RegisterInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::Register, &input.email)
    .await?;

    let result = auth_service::register(&state.auth_db, &input.email, &input.password, &input.full_name)
        .await
        .map_err(|e| AppError::bad_request(e))?;

    // Create user's app DB and run migrations
    let app_db_path = state.config.user_db_path(&result.user_id);
    let app_db = crate::db::init_database(&app_db_path)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao criar banco de dados: {}", e)))?;

    // Insert user record in their own app DB
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    sqlx::query(
        r#"INSERT INTO users (id, email, full_name, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&result.user_id)
    .bind(&result.email)
    .bind(&result.full_name)
    .bind(&now)
    .bind(&now)
    .execute(&app_db)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao criar usuario no app DB: {}", e)))?;

    // Envelope + chave de dados. O register e o unico momento em que a senha E o
    // codigo de recuperacao existem em claro ao mesmo tempo, entao e aqui que os
    // dois wraps podem nascer juntos.
    crate::crypto::unlock_user_crypto(
        &state.auth_db,
        &result.user_id,
        &input.password,
        Some(&result.recovery_secret),
    )
    .await
    .map_err(|e| AppError::internal(format!("Erro ao iniciar criptografia: {}", e)))?;

    Ok(Json(ActionResponse::success(
        "Conta criada com sucesso!",
        serde_json::json!({
            "user_id": result.user_id,
            "email": result.email,
            "full_name": result.full_name,
            "token": result.token,
            "recovery_secret": result.recovery_secret,
            "onboarding_completed": false,
        }),
    )))
}

async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LoginInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::Login, &input.email)
    .await?;

    let result = match auth_service::login(&state.auth_db, &input.email, &input.password).await {
        Ok(r) => {
            record_session_event(
                &state.auth_db,
                &r.user_id,
                AuditAction::LoginSucceeded,
                Some(&r.user_id),
                serde_json::json!({}),
            )
            .await;
            r
        }
        Err(e) => {
            // O e-mail digitado so e registrado quando corresponde a uma conta
            // existente. Nesse caso ele ja esta em `auth_users` e o log nao expoe
            // nada novo; serve para a pessoa ver tentativas contra a propria conta.
            //
            // Quando nao corresponde, pode ser o endereco de um terceiro digitado
            // por engano — e guardar isso criaria dado pessoal sobre alguem que
            // nem usa o sistema, sem nenhum ganho de auditoria. O dominio basta
            // para distinguir varredura de erro de digitacao.
            let conhecido =
                auth_service::find_user_id_by_email(&state.auth_db, &input.email).await.is_ok();
            let dominio = input.email.rsplit('@').next().unwrap_or("").to_string();
            let detalhe = if conhecido {
                serde_json::json!({"email": input.email, "conta_existente": true})
            } else {
                serde_json::json!({"conta_existente": false, "dominio": dominio})
            };
            let user_id = if conhecido {
                format!("unknown:{}", input.email)
            } else {
                "unknown".to_string()
            };
            record_session_event(
                &state.auth_db,
                &user_id,
                AuditAction::LoginFailed,
                None,
                detalhe,
            )
            .await;
            return Err(AppError::unauthorized(e));
        }
    };

    // A chave de dados passa a vir da senha, pelo envelope. Para quem vem de
    // versao anterior, o envelope e criado aqui na primeira autenticacao, tendo
    // como DEK a propria chave que o usuario ja tinha — nenhum dado e re-cifrado.
    crate::crypto::unlock_user_crypto(&state.auth_db, &result.user_id, &input.password, None)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao iniciar criptografia: {}", e)))?;

    // Open user's app DB
    let user_db = state.get_or_open_user_db(&result.user_id).await?;

    // Now that the key is loaded, sweep any patient PII still sitting in the
    // legacy plaintext columns. Best-effort: never block a login over it.
    if let Err(e) =
        crate::features::patients::migrate_plaintext_pii(&user_db, &result.user_id).await
    {
        tracing::warn!("[Auth] Falha ao migrar PII em texto claro no login: {}", e);
    }

    // Reparacao de transicao: converte registros que ficaram sob a chave legada.
    // Best-effort de proposito — um problema aqui nao pode impedir a entrada.
    reparar_chave_legada(&state, &user_db, &result.user_id).await;

    Ok(Json(ActionResponse::success(
        "Login realizado com sucesso!",
        serde_json::json!({
            "user_id": result.user_id,
            "email": result.email,
            "full_name": result.full_name,
            "token": result.token,
            "onboarding_completed": result.onboarding_completed,
        }),
    )))
}

async fn logout_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let user_id = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map(|(uid, _, _)| uid)
        .unwrap_or_default();
    auth_service::logout(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::internal(e))?;
    if !user_id.is_empty() {
        record_session_event(
            &state.auth_db,
            &user_id,
            AuditAction::Logout,
            Some(&user_id),
            serde_json::json!({}),
        )
        .await;
    }
    if !user_id.is_empty() {
        // A chave de dados tem de sair da memoria no logout, nao so no lock.
        //
        // Antes, apenas `lock_handler` limpava o cache. Depois de um logout a
        // chave AES continuava viva no processo, e isso vazava para o backup
        // agendado: `collect_files` gravava os anexos decifrados no ZIP se
        // alguem tivesse logado desde o boot, e cifrados se nao — o mesmo
        // comando produzia dois formatos diferentes.
        crate::crypto::clear_user_crypto(&user_id);
        state.clear_user_db_for_user(&user_id).await;
    }
    Ok(Json(ActionResponse::<()>::success_empty("Sessão encerrada.")))
}

async fn me_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, email, full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    let onboarding_completed = auth_service::get_onboarding_status(&state.auth_db, &user_id)
        .await
        .unwrap_or(false);

    // Re-open user's app DB (useful after page refresh)
    state.get_or_open_user_db(&user_id).await?;

    // A sessao pode estar valida sem a chave de dados estar carregada: o token
    // vive no sessionStorage e sobrevive a um F5, e o cache de chaves morre com
    // o processo do backend. Nesse estado o app respondia 200 "logado" e a tela
    // abria com os campos de PII vazios, como se o paciente nao tivesse
    // telefone nem historico — e uma edicao a partir dali gravaria o vazio.
    //
    // Reportar `locked` deixa o frontend pedir a senha em vez de mostrar dado
    // ausente como se fosse dado real.
    let locked = crate::crypto::load_key(&user_id).is_err();

    // Falta o wrap de recuperacao?
    //
    // Quem vem de versao anterior tem codigo de recuperacao valido mas nenhum
    // wrap, porque um wrap so nasce do segredo em claro e num login comum o
    // banco so tem o hash. Enquanto for assim, essa conta nao tem segunda via
    // da chave: se a senha for esquecida depois da rotacao, o prontuario fica
    // inacessivel. A UI usa isto para pedir a emissao de um codigo novo.
    use crate::crypto::envelope::{DekRole, DekSource, Slot};
    let deks = crate::crypto::envelope::load_deks(&state.auth_db, &user_id)
        .await
        .unwrap_or_default();
    let atual = deks.iter().find(|d| d.role == DekRole::Current);

    let recovery_wrap_missing = atual
        .map(|d| !d.wraps.iter().any(|w| w.slot == Slot::Recovery))
        // Sem envelope ainda (primeiro acesso): nao ha o que cobrar.
        .unwrap_or(false);

    // A chave ainda e a derivada do pepper, ou ha rotacao pela metade?
    //
    // Enquanto for assim, quem tem a conta do sistema operacional abre os
    // prontuarios sem saber a senha. A UI usa isto para oferecer a rotacao.
    let key_rotation_pending = atual.map(|d| d.source == DekSource::LegacyPepperV1).unwrap_or(false)
        || deks.iter().any(|d| d.role == DekRole::Retiring);

    Ok(Json(ActionResponse::success(
        "",
        serde_json::json!({
            "user_id": user_id,
            "email": email,
            "full_name": full_name,
            "onboarding_completed": onboarding_completed,
            "locked": locked,
            "recovery_wrap_missing": recovery_wrap_missing,
            "key_rotation_pending": key_rotation_pending,
        }),
    )))
}

/// Emite um codigo de recuperacao novo e cria o wrap correspondente.
///
/// Este endpoint existe por um motivo concreto: quem vem de versao anterior tem
/// um codigo de recuperacao valido, mas **nenhum wrap** — porque um wrap so pode
/// ser criado a partir do segredo em claro, e num login comum o banco so tem o
/// hash. Sem wrap de recuperacao, rotacionar a chave de dados transformaria
/// "esqueci a senha" em perda definitiva do prontuario.
///
/// Exige a senha, e nao apenas a sessao: e a senha que abre a DEK para poder
/// embrulha-la de novo. O codigo anterior e invalidado por completo — hash e
/// wrap. Manter o hash antigo valido sem o wrap correspondente criaria uma
/// armadilha: o codigo autenticaria e ainda assim nao abriria os dados.
///
/// Isso e aceitavel aqui porque a pessoa acabou de digitar a senha, ou seja
/// ainda a tem. No fluxo de reset, onde ela nao tem a senha, o codigo anterior e
/// preservado.
async fn rotate_recovery_code_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<UnlockInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(AppError::unauthorized)?;

    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::RecoveryRotate, &user_id)
    .await?;

    if !auth_service::verify_user_password(&state.auth_db, &user_id, &input.password)
        .await
        .map_err(AppError::internal)?
    {
        return Err(AppError::unauthorized("Senha incorreta."));
    }

    // A senha e o que abre a DEK. Sem ela nao ha o que embrulhar.
    let dek = crate::crypto::unwrap_dek_for_user(&state.auth_db, &user_id, &input.password).await?;

    let novo_codigo = auth_service::generate_recovery_secret();
    let novo_hash = auth_service::hash_recovery_secret(&novo_codigo);

    let mut tx = state
        .auth_db
        .begin()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao iniciar rotacao: {}", e)))?;

    // Hash e wrap na MESMA transacao: dessincronizar os dois produz um codigo que
    // abre a chave mas e recusado na entrada, ou o contrario.
    sqlx::query(
        "UPDATE auth_users SET recovery_secret_hash = ?, recovery_secret_hash_prev = NULL \
         WHERE id = ?",
    )
    .bind(&novo_hash)
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao gravar codigo: {}", e)))?;

    sqlx::query(
        "DELETE FROM dek_wraps WHERE slot = 'recovery_prev' AND dek_id IN \
         (SELECT id FROM user_deks WHERE user_id = ?)",
    )
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal(format!("Erro ao limpar envelope anterior: {}", e)))?;

    crate::crypto::envelope::set_recovery_wrap(&mut tx, &user_id, &dek, &novo_codigo).await?;

    tx.commit()
        .await
        .map_err(|e| AppError::internal(format!("Erro ao concluir rotacao: {}", e)))?;

    record_session_event(
        &state.auth_db,
        &user_id,
        AuditAction::PasswordReset,
        Some(&user_id),
        serde_json::json!({"action": "recovery_code_rotated"}),
    )
    .await;

    Ok(Json(ActionResponse::success(
        "Novo código de recuperação gerado. Guarde-o: o anterior deixou de valer.",
        serde_json::json!({ "user_id": user_id, "recovery_secret": novo_codigo }),
    )))
}

/// Troca a chave de dados por uma aleatoria e re-cifra o acervo.
///
/// E a operacao que faz a senha passar a ser indispensavel: a chave deixa de ser
/// derivavel do pepper do cofre e passa a existir apenas dentro dos envelopes.
///
/// Exige os DOIS segredos porque a chave nova precisa nascer com os dois
/// envelopes, e um envelope so pode ser criado a partir do segredo em claro. Um
/// unico envelope significaria que esquecer aquele segredo apaga o prontuario.
///
/// Roda de forma sincrona. Para o alvo do app — um consultorio individual, com
/// centenas de registros — a conversao leva a ordem de milissegundos por
/// registro; se algum dia houver base grande o suficiente para incomodar, o
/// caminho e reportar progresso, nao paralelizar: a rotacao e retomavel, mas
/// duas rotacoes concorrentes brigariam pelo mesmo chaveiro.
async fn rotate_data_key_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<RotateKeyInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _e, _f) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(AppError::unauthorized)?;

    // Cada tentativa gera um backup completo e pode re-cifrar todo o acervo.
    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::RotateKey, &user_id)
        .await?;

    if !auth_service::verify_user_password(&state.auth_db, &user_id, &input.password)
        .await
        .map_err(AppError::internal)?
    {
        return Err(AppError::unauthorized("Senha incorreta."));
    }

    let user_db = state.get_or_open_user_db(&user_id).await?;
    let relatorio = crate::crypto::rotation::rotate_to_random_dek(
        &user_db,
        &state.auth_db,
        &state.config,
        &user_id,
        &input.password,
        &input.recovery_code,
    )
    .await?;

    record_session_event(
        &state.auth_db,
        &user_id,
        AuditAction::PasswordReset,
        Some(&user_id),
        serde_json::json!({
            "action": "data_key_rotated",
            "patients": relatorio.patients,
            "session_records": relatorio.session_records,
            "files": relatorio.files,
            "resumed": relatorio.resumed,
        }),
    )
    .await;

    Ok(Json(ActionResponse::success(
        "Chave protegida pela sua senha.",
        serde_json::json!({
            "patients": relatorio.patients,
            "session_records": relatorio.session_records,
            "files": relatorio.files,
            "resumed": relatorio.resumed,
            "safety_backup": relatorio.safety_backup,
        }),
    )))
}

/// Confirma que o usuario guardou o codigo atual, descartando o anterior.
async fn ack_recovery_code_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _e, _f) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(AppError::unauthorized)?;

    sqlx::query("UPDATE auth_users SET recovery_secret_hash_prev = NULL WHERE id = ?")
        .bind(&user_id)
        .execute(&state.auth_db)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao confirmar codigo: {}", e)))?;
    crate::crypto::envelope::drop_previous_recovery_wrap(&state.auth_db, &user_id).await?;

    Ok(Json(ActionResponse::<()>::success_empty(
        "Código confirmado. O anterior foi descartado.",
    )))
}

/// Converte registros parados sob a chave legada, e registra o resultado.
///
/// Chamada no login e no unlock. Best-effort: qualquer falha aqui vira aviso no
/// log, nunca erro para o usuario — o custo de bloquear a entrada e maior que o
/// de adiar a reparacao para a proxima autenticacao.
async fn reparar_chave_legada(state: &Arc<AppState>, user_db: &sqlx::SqlitePool, user_id: &str) {
    match crate::crypto::rotation::repair_rows_under_legacy_key(user_db, &state.auth_db, user_id)
        .await
    {
        Ok(r) if r.touched() > 0 || r.illegible > 0 => {
            tracing::info!(
                "[Auth] Reparacao de chave: {} paciente(s), {} prontuario(s) e {} anexo(s)                  reconvertidos para a chave atual; {} ilegivel(is) mantido(s) intacto(s).",
                r.patients,
                r.session_records,
                r.files,
                r.illegible
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("[Auth] Reparacao de chave nao pode ser concluida: {}", e),
    }
}

async fn recover_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RecoverInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    let user_id = match (&input.user_id, &input.email) {
        (Some(uid), _) if !uid.is_empty() => uid.clone(),
        (_, Some(email)) if !email.is_empty() => {
            auth_service::find_user_id_by_email(&state.auth_db, email)
                .await
                .map_err(|e| AppError::not_found(e))?
        }
        _ => return Err(AppError::bad_request("Informe user_id ou email.")),
    };

    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::PasswordReset, &user_id)
    .await?;

    let result = auth_service::recover_with_secret(
        &state.auth_db,
        &user_id,
        &input.recovery_secret,
    )
    .await
    .map_err(|e| AppError::unauthorized(e))?;

    Ok(Json(ActionResponse::success(
        "Chave verificada. Crie uma nova senha.",
        serde_json::json!({
            "reset_token": result.reset_token,
        }),
    )))
}

async fn reset_password_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ResetPasswordInput>,
) -> Result<Json<ActionResponse<serde_json::Value>>, AppError> {
    // O `/auth/recover` e limitado a 3/15min, mas este passo nao tinha limite:
    // o reset_token e um UUID de 5 minutos e, sem limite, era atacavel por forca bruta.
    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::PasswordReset, &input.reset_token)
    .await?;

    let result =
        auth_service::reset_password(&state.auth_db, &input.reset_token, &input.new_password)
            .await
            .map_err(|e| AppError::bad_request(e))?;

    record_session_event(
        &state.auth_db,
        &result.user_id,
        AuditAction::PasswordReset,
        Some(&result.user_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(ActionResponse::success(
        "Senha redefinida com sucesso! Faca login novamente.",
        serde_json::json!({
            // The code used for this reset is now spent; these replace it, in the
            // same shape as the recovery file written at registration.
            "user_id": result.user_id,
            "recovery_secret": result.recovery_secret,
        }),
    )))
}

async fn lock_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    crate::crypto::clear_user_crypto(&user_id);
    state.clear_user_db_for_user(&user_id).await;

    record_session_event(
        &state.auth_db,
        &user_id,
        AuditAction::Locked,
        Some(&user_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(ActionResponse::<()>::success_empty("Tela bloqueada.")))
}

async fn unlock_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<UnlockInput>,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    // O desbloqueio nao tinha limite nenhum no servidor. A unica protecao era um
    // contador em `useRef` no LockScreen, que zera a cada refresh da pagina —
    // ou seja, com um token valido em maos dava para testar senhas sem limite,
    // e o unlock e justamente o que devolve a chave de dados. O login sempre
    // teve 5/10min; nao faz sentido a porta de tras ser mais franca que a porta
    // da frente.
    crate::rate_limit::enforce(&state.auth_db, crate::rate_limit::Scope::Unlock, &user_id)
        .await?;

    let password_valid = auth_service::verify_user_password(&state.auth_db, &user_id, &input.password)
        .await
        .map_err(|e| AppError::internal(e))?;

    if !password_valid {
        record_session_event(
            &state.auth_db,
            &user_id,
            AuditAction::LoginFailed,
            Some(&user_id),
            serde_json::json!({"reason": "unlock_wrong_password"}),
        )
        .await;
        return Err(AppError::unauthorized("Senha incorreta."));
    }

    // O unlock ja verificou a senha logo acima; aqui ela e usada de novo, agora
    // para abrir a DEK pelo envelope. Mesmo caminho do login.
    crate::crypto::unlock_user_crypto(&state.auth_db, &user_id, &input.password, None)
        .await
        .map_err(|e| AppError::internal(format!("Erro ao reiniciar criptografia: {}", e)))?;

    let user_db = state.get_or_open_user_db(&user_id).await?;

    if let Err(e) = crate::features::patients::migrate_plaintext_pii(&user_db, &user_id).await {
        tracing::warn!("[Auth] Falha ao migrar PII em texto claro no unlock: {}", e);
    }

    reparar_chave_legada(&state, &user_db, &user_id).await;

    record_session_event(
        &state.auth_db,
        &user_id,
        AuditAction::Unlocked,
        Some(&user_id),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(ActionResponse::<()>::success_empty("Tela desbloqueada.")))
}

async fn onboarding_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse<()>>, AppError> {
    let token = extract_bearer_token(&headers)?;
    let (user_id, _email, _full_name) = auth_service::validate_session(&state.auth_db, &token)
        .await
        .map_err(|e| AppError::unauthorized(e))?;

    auth_service::set_onboarding_completed(&state.auth_db, &user_id)
        .await
        .map_err(|e| AppError::internal(e))?;

    Ok(Json(ActionResponse::<()>::success_empty(
        "Onboarding concluido.",
    )))
}

/// Records a session-scoped audit event, logging (rather than discarding) any
/// failure. These writes used to be dropped with `let _ = ...`, which is how a
/// missing `audit_logs` table in the auth database went unnoticed.
async fn record_session_event(
    db: &sqlx::SqlitePool,
    user_id: &str,
    action: AuditAction,
    entity_id: Option<&str>,
    details: serde_json::Value,
) {
    if let Err(e) =
        audit::write_audit_event(db, user_id, action, "session", entity_id, details, None).await
    {
        tracing::error!(
            "[Auth] Falha ao registrar evento de auditoria '{}': {}",
            action.as_str(),
            e
        );
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Token nao informado."))?;

    auth_header
        .strip_prefix("Bearer ")
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::unauthorized("Formato invalido. Use: Bearer <token>."))
}
