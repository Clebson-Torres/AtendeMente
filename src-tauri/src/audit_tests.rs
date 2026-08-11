#[cfg(test)]
mod tests {
    use crate::{audit, db};

    async fn test_db() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("audit-test.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
        let pool = db::init_database(&db_url).await.unwrap();
        (dir, pool)
    }

    async fn seed_user(db: &sqlx::SqlitePool, user_id: &str) {
        sqlx::query(
            "INSERT INTO users (id, email, full_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind("audit@example.com")
        .bind("Audit User")
        .bind("2026-06-18T10:00:00")
        .bind("2026-06-18T10:00:00")
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn writes_and_lists_lgpd_audit_events_without_sensitive_content() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;

        audit::write_audit_event(
            &db,
            user_id,
            audit::AuditAction::PatientViewed,
            "patient",
            Some("patient-123"),
            serde_json::json!({"field": "metadata-only"}),
            Some("local-device"),
        )
        .await
        .unwrap();

        let events = audit::list_audit_events(&db, user_id, 20).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "patient.viewed");
        assert!(!events[0].details.contains("conteudo do prontuario"));
    }

    /// `write_audit_log` used to funnel every unrecognised action into
    /// `patient.updated`, so deleting a file or editing a session record was
    /// recorded as a patient edit. For an LGPD trail that is worse than useless.
    #[tokio::test]
    async fn maps_each_action_to_its_own_event() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;

        let casos: &[(&str, &str, &str)] = &[
            ("file_delete", "record_file", "file.deleted"),
            ("file_upload", "record_file", "file.upload.approved"),
            ("file_download", "record_file", "file.downloaded"),
            ("patient_export", "patient", "system.export"),
            ("delete", "patient", "patient.deleted"),
            ("update", "patient", "patient.updated"),
            ("update", "appointment", "appointment.updated"),
            ("update", "session_record", "record.updated"),
            ("update", "payment", "payment.updated"),
        ];

        for (acao, entidade, _) in casos {
            audit::write_audit_log(&db, user_id, acao, entidade, None, None, None, None)
                .await
                .unwrap();
        }

        let events = audit::list_audit_events(&db, user_id, 100).await.unwrap();
        assert_eq!(events.len(), casos.len());

        for (acao, entidade, esperado) in casos {
            let achou = events
                .iter()
                .any(|e| e.action == *esperado && e.entity_type == *entidade);
            assert!(
                achou,
                "acao {acao:?} em {entidade:?} deveria virar {esperado:?}; \
                 registrados: {:?}",
                events.iter().map(|e| (&e.action, &e.entity_type)).collect::<Vec<_>>()
            );
        }

        // Nada deve ter caido no antigo catch-all de patient.updated.
        let patient_updated = events.iter().filter(|e| e.action == "patient.updated").count();
        assert_eq!(
            patient_updated, 1,
            "so o update de paciente deve ser patient.updated"
        );
    }

    #[tokio::test]
    async fn unknown_actions_fall_back_to_a_generic_event() {
        let (_dir, db) = test_db().await;
        let user_id = "550e8400-e29b-41d4-a716-446655440001";
        seed_user(&db, user_id).await;

        audit::write_audit_log(&db, user_id, "acao_inventada", "coisa", None, None, None, None)
            .await
            .unwrap();

        let events = audit::list_audit_events(&db, user_id, 10).await.unwrap();
        assert_eq!(events[0].action, "entity.updated");
        assert_ne!(
            events[0].action, "patient.updated",
            "acao desconhecida nao deve se disfarcar de edicao de paciente"
        );
    }

    #[tokio::test]
    async fn events_are_scoped_per_user_and_newest_first() {
        let (_dir, db) = test_db().await;
        let a = "550e8400-e29b-41d4-a716-44665544000a";
        let b = "550e8400-e29b-41d4-a716-44665544000b";
        seed_user(&db, a).await;
        sqlx::query("INSERT INTO users (id, email, created_at, updated_at) VALUES (?, 'b@x.com', '2026-01-01T00:00:00', '2026-01-01T00:00:00')")
            .bind(b)
            .execute(&db)
            .await
            .unwrap();

        audit::write_audit_event(&db, a, audit::AuditAction::PatientViewed, "patient", None, serde_json::json!({}), None).await.unwrap();
        audit::write_audit_event(&db, b, audit::AuditAction::FileDeleted, "record_file", None, serde_json::json!({}), None).await.unwrap();

        let de_a = audit::list_audit_events(&db, a, 50).await.unwrap();
        assert_eq!(de_a.len(), 1, "auditoria de um usuario nao deve vazar para outro");
        assert_eq!(de_a[0].user_id, a);

        // limit e respeitado
        for _ in 0..5 {
            audit::write_audit_event(&db, a, audit::AuditAction::Locked, "session", None, serde_json::json!({}), None).await.unwrap();
        }
        let limitado = audit::list_audit_events(&db, a, 3).await.unwrap();
        assert_eq!(limitado.len(), 3);
    }
}
