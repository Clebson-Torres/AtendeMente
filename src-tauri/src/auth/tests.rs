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

    #[test]
    fn generate_recovery_secret_format() {
        let secret = auth_service::generate_recovery_secret();
        assert_eq!(secret.len(), 19);
        let parts: Vec<&str> = secret.split('-').collect();
        assert_eq!(parts.len(), 4);
        for part in &parts {
            assert_eq!(part.len(), 4);
            assert!(part.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn generate_recovery_secret_unique() {
        let a = auth_service::generate_recovery_secret();
        let b = auth_service::generate_recovery_secret();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_recovery_secret_deterministic() {
        let a = auth_service::hash_recovery_secret("ABCD-EFGH-IJKL-MNOP");
        let b = auth_service::hash_recovery_secret("ABCD-EFGH-IJKL-MNOP");
        assert_eq!(a, b);
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
    async fn reset_password_changes_password_and_keeps_recovery_secret() {
        let (_dir, db) = test_auth_db().await;
        let reg = auth_service::register(&db, "reset-pwd@test.com", "senha12345", "Reset User")
            .await
            .unwrap();

        let recovery = auth_service::recover_with_secret(&db, &reg.user_id, &reg.recovery_secret)
            .await
            .unwrap();

        auth_service::reset_password(&db, &recovery.reset_token, "nova-senha-67890")
            .await
            .unwrap();

        let login_new = auth_service::login(&db, "reset-pwd@test.com", "nova-senha-67890").await;
        assert!(login_new.is_ok());

        let login_old = auth_service::login(&db, "reset-pwd@test.com", "senha12345").await;
        assert!(login_old.is_err());

        let recovery2 = auth_service::recover_with_secret(&db, &reg.user_id, &reg.recovery_secret)
            .await;
        assert!(recovery2.is_ok());
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
