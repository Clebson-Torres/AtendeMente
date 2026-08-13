#[cfg(test)]
mod tests {
    use crate::auth::auth_service;
    use crate::db;

    async fn test_auth_db() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("auth-test.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = db::init_auth_database(&db_url).await.unwrap();
        (dir, pool)
    }

    // ─── Pure function tests (no DB) ─────────────────────────────────────

    #[test]
    fn hash_and_verify_password_ok() {
        let hash = auth_service::hash_password("minha-senha-segura").unwrap();
        assert!(auth_service::verify_password("minha-senha-segura", &hash).unwrap());
    }

    #[test]
    fn hash_and_verify_password_wrong() {
        let hash = auth_service::hash_password("senha-correta").unwrap();
        assert!(!auth_service::verify_password("senha-errada", &hash).unwrap());
    }

    /// O codigo passou de 8 para 16 bytes — 64 para 128 bits.
    ///
    /// 64 bits bastavam enquanto o codigo so servia para redefinir senha, com
    /// rate limit pela rede. Deixam de bastar quando ele protege uma copia da
    /// chave de dados, porque o material embrulhado viaja dentro de todo backup
    /// e o ataque passa a ser offline, com o arquivo em maos.
    #[test]
    fn generate_recovery_secret_format() {
        let secret = auth_service::generate_recovery_secret();
        // 32 hex em 8 grupos de 4, separados por hifen.
        assert_eq!(secret.len(), 39);
        let parts: Vec<&str> = secret.split('-').collect();
        assert_eq!(parts.len(), 8);
        for part in &parts {
            assert_eq!(part.len(), 4);
            assert!(part.chars().all(|c| c.is_ascii_hexdigit()));
        }
        // 16 bytes de entropia.
        assert_eq!(secret.chars().filter(|c| c.is_ascii_hexdigit()).count(), 32);
    }

    #[test]
    fn generate_recovery_secret_unique() {
        let a = auth_service::generate_recovery_secret();
        let b = auth_service::generate_recovery_secret();
        assert_ne!(a, b);
    }

    /// O hash agora e SALGADO, o oposto do que este teste exigia antes.
    ///
    /// Era SHA-256 puro: hashear o mesmo codigo dava sempre o mesmo resultado,
    /// o que para um segredo curto e uma tabela de busca para quem tiver o
    /// banco. Determinismo aqui era a fraqueza, nao a garantia.
    #[test]
    fn hash_recovery_secret_e_salgado_e_verificavel() {
        let codigo = "ABCD-EF01-2345-6789-ABCD-EF01-2345-6789";
        let a = auth_service::hash_recovery_secret(codigo);
        let b = auth_service::hash_recovery_secret(codigo);
        assert_ne!(a, b, "hashes iguais indicariam ausencia de sal");
        assert!(a.starts_with("$argon2id$"));

        // Mas os dois verificam o mesmo codigo.
        assert!(auth_service::verify_recovery_secret(codigo, &a));
        assert!(auth_service::verify_recovery_secret(codigo, &b));
        assert!(!auth_service::verify_recovery_secret("OUTRO-CODIGO", &a));
    }

    /// O usuario pode digitar com ou sem hifens, em qualquer caixa.
    #[test]
    fn codigo_de_recuperacao_e_normalizado_na_digitacao() {
        let codigo = "ABCD-EF01-2345-6789-ABCD-EF01-2345-6789";
        let hash = auth_service::hash_recovery_secret(codigo);

        assert!(auth_service::verify_recovery_secret(codigo, &hash));
        assert!(auth_service::verify_recovery_secret(
            "abcdef0123456789abcdef0123456789",
            &hash
        ));
        assert!(auth_service::verify_recovery_secret(
            "ABCDEF01 2345 6789 ABCDEF0123456789",
            &hash
        ));
    }

    /// Um codigo emitido pela versao anterior tem hash SHA-256 de 64 hex e
    /// precisa continuar valendo — o usuario ja o anotou ou guardou em arquivo.
    #[test]
    fn aceita_hash_sha256_legado() {
        use sha2::{Digest, Sha256};
        let codigo_antigo = "ABCD-EF01-2345-6789";
        let mut h = Sha256::new();
        h.update(codigo_antigo.as_bytes());
        let hash_legado = format!("{:x}", h.finalize());
        assert_eq!(hash_legado.len(), 64);

        assert!(
            auth_service::verify_recovery_secret(codigo_antigo, &hash_legado),
            "codigo emitido pela versao anterior tem de continuar aceito"
        );
        assert!(!auth_service::verify_recovery_secret("ABCD-EF01-2345-0000", &hash_legado));
    }

    #[test]
    fn generate_session_token_roundtrip() {
        let (token, hash) = auth_service::generate_session_token();
        assert_eq!(auth_service::hash_token(&token), hash);
    }

    // ─── Registration & Login edge cases ────────────────────────────────

    #[tokio::test]
    async fn register_returns_onboarding_completed_false() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::register(&db, "novo@test.com", "senha12345", "Novo Usuario")
            .await
            .unwrap();

        assert!(!result.onboarding_completed);
    }

    #[tokio::test]
    async fn register_returns_error_for_duplicate_email() {
        let (_dir, db) = test_auth_db().await;
        auth_service::register(&db, "dup@test.com", "senha12345", "First")
            .await
            .unwrap();

        let result = auth_service::register(&db, "dup@test.com", "outrasenha", "Second").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn login_returns_onboarding_completed_false_for_new_user() {
        let (_dir, db) = test_auth_db().await;
        auth_service::register(&db, "login@test.com", "senha12345", "Login User")
            .await
            .unwrap();

        let result = auth_service::login(&db, "login@test.com", "senha12345")
            .await
            .unwrap();

        assert!(!result.onboarding_completed);
    }

    #[tokio::test]
    async fn login_returns_error_for_wrong_password() {
        let (_dir, db) = test_auth_db().await;
        auth_service::register(&db, "wrong-login@test.com", "senha12345", "Wrong Login")
            .await
            .unwrap();

        let result = auth_service::login(&db, "wrong-login@test.com", "senha-errada").await;
        assert!(result.is_err());
    }

    // ─── Onboarding ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn set_onboarding_completed_marks_user() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "complete@test.com", "senha12345", "Complete User")
            .await
            .unwrap();

        let status = auth_service::get_onboarding_status(&db, &reg.user_id)
            .await
            .unwrap();
        assert!(!status);

        auth_service::set_onboarding_completed(&db, &reg.user_id)
            .await
            .unwrap();

        let status = auth_service::get_onboarding_status(&db, &reg.user_id)
            .await
            .unwrap();
        assert!(status);
    }

    #[tokio::test]
    async fn login_returns_onboarding_completed_true_after_completion() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "after@test.com", "senha12345", "After User")
            .await
            .unwrap();

        auth_service::set_onboarding_completed(&db, &reg.user_id)
            .await
            .unwrap();

        let result = auth_service::login(&db, "after@test.com", "senha12345")
            .await
            .unwrap();

        assert!(result.onboarding_completed);
    }

    #[tokio::test]
    async fn get_onboarding_status_returns_false_for_nonexistent_user() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::get_onboarding_status(&db, "nonexistent-id").await;
        assert!(result.is_err());
    }

    // ─── Session validation ─────────────────────────────────────────────

    #[tokio::test]
    async fn validate_session_returns_user_info_for_valid_session() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "validate@test.com", "senha12345", "Validate User")
            .await
            .unwrap();

        let result = auth_service::validate_session(&db, &reg.token).await;
        assert!(result.is_ok());
        let (uid, email, name) = result.unwrap();
        assert_eq!(uid, reg.user_id);
        assert_eq!(email, reg.email);
        assert_eq!(name, reg.full_name);
    }

    #[tokio::test]
    async fn validate_session_returns_error_for_expired_session() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "expired@test.com", "senha12345", "Expired User")
            .await
            .unwrap();

        sqlx::query("UPDATE sessions SET expires_at = '2020-01-01T00:00:00' WHERE token_hash = ?")
            .bind(&auth_service::hash_token(&reg.token))
            .execute(&db)
            .await
            .unwrap();

        let result = auth_service::validate_session(&db, &reg.token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expirada"));
    }

    #[tokio::test]
    async fn validate_session_returns_error_for_nonexistent_token() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::validate_session(&db, "token-inexistente").await;
        assert!(result.is_err());
    }

    // ─── Logout ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn logout_deletes_session() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "logout@test.com", "senha12345", "Logout User")
            .await
            .unwrap();

        assert!(auth_service::validate_session(&db, &reg.token).await.is_ok());

        auth_service::logout(&db, &reg.token).await.unwrap();

        assert!(auth_service::validate_session(&db, &reg.token).await.is_err());
    }

    #[tokio::test]
    async fn logout_with_nonexistent_token_does_not_error() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::logout(&db, "token-inexistente").await;
        assert!(result.is_ok());
    }

    // ─── Password verification ──────────────────────────────────────────

    #[tokio::test]
    async fn verify_user_password_returns_true_for_correct_password() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "verify@test.com", "senha12345", "Verify User")
            .await
            .unwrap();

        assert!(auth_service::verify_user_password(&db, &reg.user_id, "senha12345")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn verify_user_password_returns_false_for_wrong_password() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "verify-wrong@test.com", "senha12345", "Verify Wrong")
            .await
            .unwrap();

        assert!(!auth_service::verify_user_password(&db, &reg.user_id, "senha-errada")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn verify_user_password_returns_error_for_nonexistent_user() {
        let (_dir, db) = test_auth_db().await;

        let result = auth_service::verify_user_password(&db, "id-inexistente", "senha12345").await;
        assert!(result.is_err());
    }

    // ─── Email lookup ───────────────────────────────────────────────────

    #[tokio::test]
    async fn find_user_id_by_email_returns_id_for_existing_user() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "findbyemail@test.com", "senha12345", "Find Email")
            .await
            .unwrap();

        let found_id = auth_service::find_user_id_by_email(&db, "findbyemail@test.com")
            .await
            .unwrap();

        assert_eq!(found_id, reg.user_id);
    }

    #[tokio::test]
    async fn find_user_id_by_email_returns_error_for_nonexistent_email() {
        let (_dir, db) = test_auth_db().await;

        let result = auth_service::find_user_id_by_email(&db, "naoexiste@test.com").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Nenhuma conta encontrada"));
    }

    #[tokio::test]
    async fn find_user_id_by_email_is_case_insensitive() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "CaseEmail@test.com", "senha12345", "Case User")
            .await
            .unwrap();

        let found_id = auth_service::find_user_id_by_email(&db, "caseemail@TEST.COM")
            .await
            .unwrap();

        assert_eq!(found_id, reg.user_id);
    }

    // ─── Recovery ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn recover_with_secret_works_with_email_lookup() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "recover-email@test.com", "senha12345", "Recover Email")
            .await
            .unwrap();

        let found_id = auth_service::find_user_id_by_email(&db, "recover-email@test.com")
            .await
            .unwrap();

        let result = auth_service::recover_with_secret(&db, &found_id, &reg.recovery_secret)
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap().reset_token.is_empty());
    }

    #[tokio::test]
    async fn recover_with_secret_works_with_user_id_directly() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "recover-file@test.com", "senha12345", "File User")
            .await
            .unwrap();

        let result = auth_service::recover_with_secret(&db, &reg.user_id, &reg.recovery_secret)
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap().reset_token.is_empty());
    }

    #[tokio::test]
    async fn recover_with_secret_rejects_wrong_secret() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "recover-wrong@test.com", "senha12345", "Wrong User")
            .await
            .unwrap();

        let result = auth_service::recover_with_secret(
            &db,
            &reg.user_id,
            "0000-0000-0000-0000",
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("inválida"));
    }

    // ─── Reset password ─────────────────────────────────────────────────

    #[tokio::test]
    async fn reset_password_changes_password_and_rotates_recovery_secret() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "reset-pwd@test.com", "senha12345", "Reset User")
            .await
            .unwrap();

        let recovery = auth_service::recover_with_secret(&db, &reg.user_id, &reg.recovery_secret)
            .await
            .unwrap();

        let reset = auth_service::reset_password(&db, &recovery.reset_token, "nova-senha-67890")
            .await
            .unwrap();
        let new_secret = reset.recovery_secret;
        assert_eq!(reset.user_id, reg.user_id);

        let login_new = auth_service::login(&db, "reset-pwd@test.com", "nova-senha-67890").await;
        assert!(login_new.is_ok());

        let login_old = auth_service::login(&db, "reset-pwd@test.com", "senha12345").await;
        assert!(login_old.is_err());

        // The code that authorised the reset is spent — reusing it must fail.
        let reused = auth_service::recover_with_secret(&db, &reg.user_id, &reg.recovery_secret).await;
        assert!(reused.is_err());

        // ...and the replacement issued by the reset works.
        assert_ne!(new_secret, reg.recovery_secret);
        let with_new = auth_service::recover_with_secret(&db, &reg.user_id, &new_secret).await;
        assert!(with_new.is_ok());
    }

    #[tokio::test]
    async fn reset_password_issues_a_usable_replacement_code_each_time() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "reset-twice@test.com", "senha12345", "Twice User")
            .await
            .unwrap();

        let mut secret = reg.recovery_secret.clone();
        for i in 0..3 {
            let recovery = auth_service::recover_with_secret(&db, &reg.user_id, &secret)
                .await
                .unwrap_or_else(|e| panic!("rodada {i} deveria aceitar o codigo atual: {e}"));
            secret = auth_service::reset_password(
                &db,
                &recovery.reset_token,
                &format!("senha-nova-{i}-abcdef"),
            )
            .await
            .unwrap()
            .recovery_secret;
        }

        assert!(
            auth_service::login(&db, "reset-twice@test.com", "senha-nova-2-abcdef")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn reset_password_rejects_invalid_token() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::reset_password(&db, "token-invalido", "nova-senha-67890").await;
        assert!(result.is_err());
    }

    // ─── Register with invalid input ────────────────────────────────────

    #[tokio::test]
    async fn register_rejects_short_password() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::register(&db, "short@test.com", "1234567", "Short Pwd").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn register_rejects_empty_name() {
        let (_dir, db) = test_auth_db().await;
        let result = auth_service::register(&db, "empty@test.com", "senha12345", "  ").await;
        assert!(result.is_err());
    }
}
