use serde::{Deserialize, Serialize};
use validator::Validate;

// ─── Limites de tamanho dos campos de entrada ────────────────────────────────
//
// A validacao de forma existia so no Zod, no cliente. Uma chamada direta a API
// aceitava um `admin_notes` de megabytes — o unico teto era o `DefaultBodyLimit`
// global de 20 MB. Estes limites sao generosos de proposito: o objetivo e barrar
// abuso, nao policiar o texto que a psicologa escreve.
//
// Toda regra abaixo precisa de `message`: `errors.rs` descarta mensagens vazias e
// so inclui o campo se sobrou alguma, entao um `length(max = ...)` sem `message`
// devolve 422 com `errors: {}` e o cliente nao sabe o que falhou.
/// Nome, e-mail, telefone, numero de prontuario: campos de uma linha.
const CURTO: u64 = 200;
/// Data em ISO ou BR, status, metodo de pagamento, frequencia de recorrencia.
const MINUSCULO: u64 = 40;
/// Identificadores UUID.
const ID: u64 = 64;
/// Historico clinico, medicacoes, observacoes: texto livre longo.
const LONGO: u64 = 20_000;
/// Prontuario de sessao — o campo mais extenso do sistema.
const PRONTUARIO: u64 = 200_000;
/// Nome de arquivo de anexo.
const NOME_ARQUIVO: u64 = 255;

// ─── Users ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub two_factor_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Patients ───────────────────────────────────────────────────────────────

/// DB row with optional encrypted PII columns.
/// Used internally for queries; callers convert to `Patient` or `PatientListItem`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PatientRow {
    pub id: String,
    pub user_id: String,
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub status: String,
    pub emergency_phone: Option<String>,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub admin_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub pii_encrypted: Option<String>,
    pub pii_iv: Option<String>,
    pub pii_auth_tag: Option<String>,
}

/// Decrypted PII fields stored inside the encrypted blob.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientPii {
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub emergency_phone: Option<String>,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub admin_notes: Option<String>,
}

/// A row in the patient_search_tokens index.
///
/// `token_text` guarda HMAC-SHA256 truncado, nunca o valor. Ate a migration 15 aqui
/// ficavam telefone e e-mail em texto claro.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SearchToken {
    pub user_id: String,
    pub patient_id: String,
    pub token_type: String,
    pub token_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    pub id: String,
    pub user_id: String,
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub status: String,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub emergency_phone: Option<String>,
    pub admin_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientListItem {
    pub id: String,
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreatePatientInput {
    #[validate(length(min = 3, max = "CURTO", message = "Informe o nome completo (min. 3 caracteres)."))]
    pub full_name: String,
    #[validate(length(max = "CURTO", message = "Numero de prontuario longo demais."))]
    pub chart_number: Option<String>,
    #[validate(length(max = "CURTO", message = "Telefone longo demais."))]
    pub phone: Option<String>,
    #[validate(length(max = "CURTO", message = "E-mail longo demais."))]
    pub email: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Data de nascimento invalida."))]
    pub birth_date: Option<String>,
    #[validate(length(max = "LONGO", message = "Historico de saude longo demais."))]
    pub health_history: Option<String>,
    #[validate(length(max = "LONGO", message = "Lista de medicacoes longa demais."))]
    pub medications_in_use: Option<String>,
    #[validate(length(max = "CURTO", message = "Telefone de emergencia longo demais."))]
    pub emergency_phone: Option<String>,
    #[validate(length(max = "LONGO", message = "Observacoes longas demais."))]
    pub admin_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdatePatientInput {
    #[validate(length(min = 3, max = "CURTO", message = "Informe o nome completo (min. 3 caracteres)."))]
    pub full_name: String,
    #[validate(length(max = "CURTO", message = "Numero de prontuario longo demais."))]
    pub chart_number: Option<String>,
    #[validate(length(max = "CURTO", message = "Telefone longo demais."))]
    pub phone: Option<String>,
    #[validate(length(max = "CURTO", message = "E-mail longo demais."))]
    pub email: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Data de nascimento invalida."))]
    pub birth_date: Option<String>,
    #[validate(length(max = "LONGO", message = "Historico de saude longo demais."))]
    pub health_history: Option<String>,
    #[validate(length(max = "LONGO", message = "Lista de medicacoes longa demais."))]
    pub medications_in_use: Option<String>,
    #[validate(length(max = "CURTO", message = "Telefone de emergencia longo demais."))]
    pub emergency_phone: Option<String>,
    #[validate(length(max = "LONGO", message = "Observacoes longas demais."))]
    pub admin_notes: Option<String>,
}

// ─── Appointments ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Appointment {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub series_id: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    pub status: String,
    pub confirmation_status: String,
    pub session_price_cents: i64,
    pub quick_notes: Option<String>,
    pub cancel_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub patient_id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub status: String,
    pub confirmation_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppointmentDetail {
    pub id: String,
    pub patient_id: String,
    pub patient_name: String,
    pub starts_at: String,
    pub ends_at: String,
    pub series_id: Option<String>,
    pub status: String,
    pub confirmation_status: String,
    pub session_price_cents: i64,
    pub quick_notes: Option<String>,
    pub cancel_reason: Option<String>,
    pub payment_id: Option<String>,
    pub payment_status: Option<String>,
    pub payment_method: Option<String>,
    pub amount_received_cents: Option<i64>,
    pub paid_at: Option<String>,
    pub payment_notes: Option<String>,
    pub record_id: Option<String>,
    pub encrypted_payload: Option<String>,
    pub iv: Option<String>,
    pub auth_tag: Option<String>,
    pub key_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateAppointmentInput {
    #[validate(length(min = 1, max = "ID", message = "Selecione um paciente."))]
    pub patient_id: String,
    #[validate(length(min = 1, max = "MINUSCULO", message = "Informe o inicio do atendimento."))]
    pub starts_at: String,
    #[validate(length(min = 1, max = "MINUSCULO", message = "Informe o fim do atendimento."))]
    pub ends_at: String,
    #[validate(length(max = "MINUSCULO", message = "Status invalido."))]
    pub status: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Status de confirmacao invalido."))]
    pub confirmation_status: Option<String>,
    #[validate(range(min = 0, max = 100_000_000, message = "Valor da sessao fora da faixa."))]
    pub session_price_cents: Option<i64>,
    #[validate(length(max = "LONGO", message = "Anotacoes longas demais."))]
    pub quick_notes: Option<String>,
    #[validate(length(max = "LONGO", message = "Motivo do cancelamento longo demais."))]
    pub cancel_reason: Option<String>,
    // Recurrence
    #[validate(length(max = "MINUSCULO", message = "Frequencia de recorrencia invalida."))]
    pub recurrence_frequency: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Modo de termino da recorrencia invalido."))]
    pub recurrence_end_mode: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Data final da recorrencia invalida."))]
    pub recurrence_until_date: Option<String>,
    // O gerador ja trunca em 52, mas rejeitar aqui e mais honesto que aceitar 9999
    // silenciosamente e devolver 52 sessoes.
    #[validate(range(min = 2, max = 52, message = "A recorrencia aceita de 2 a 52 sessoes."))]
    pub recurrence_occurrences: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateAppointmentInput {
    #[validate(length(max = "ID", message = "Paciente invalido."))]
    pub patient_id: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Inicio do atendimento invalido."))]
    pub starts_at: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Fim do atendimento invalido."))]
    pub ends_at: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Status invalido."))]
    pub status: Option<String>,
    #[validate(length(max = "MINUSCULO", message = "Status de confirmacao invalido."))]
    pub confirmation_status: Option<String>,
    #[validate(range(min = 0, max = 100_000_000, message = "Valor da sessao fora da faixa."))]
    pub session_price_cents: Option<i64>,
    /// Absent = leave unchanged, `null` = clear, value = set.
    /// A plain `Option` could not express "clear", so an explicit `null` was
    /// silently treated as "unchanged".
    // `Option<Option<String>>` nao aceita os validadores padrao do validator 0.19,
    // entao o limite destes dois vem de `custom`.
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    #[validate(custom(function = "texto_longo_opcional"))]
    pub quick_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    #[validate(custom(function = "texto_longo_opcional"))]
    pub cancel_reason: Option<Option<String>>,
}

/// Limite de tamanho para os campos "ausente / nulo / valor".
///
/// O derive desembrulha os dois niveis de `Option` antes de chamar, entao so o caso
/// com valor chega aqui — ausente e nulo nunca sao validados.
fn texto_longo_opcional(valor: &str) -> Result<(), validator::ValidationError> {
    if valor.chars().count() as u64 > LONGO {
        let mut e = validator::ValidationError::new("length");
        e.message = Some("Texto longo demais.".into());
        return Err(e);
    }
    Ok(())
}

/// Distinguishes a missing JSON field (`None`) from an explicit `null`
/// (`Some(None)`). `#[serde(default)]` supplies `None` when the key is absent;
/// this runs only when the key is present.
fn deserialize_nullable_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

// ─── Recurring Series ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecurringSeries {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub frequency: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub occurrences_count: Option<i32>,
    pub start_time: String,
    pub end_time: String,
    pub cancelled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Payments ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Payment {
    pub id: String,
    pub user_id: String,
    pub appointment_id: String,
    pub status: String,
    pub method: String,
    pub paid_at: Option<String>,
    pub amount_received_cents: i64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentWithAppointment {
    pub payment_id: Option<String>,
    pub appointment_id: String,
    pub appointment_status: String,
    pub status: String,
    pub method: String,
    pub amount_received_cents: i64,
    pub paid_at: Option<String>,
    pub patient_name: String,
    pub starts_at: String,
    pub session_price_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpsertPaymentInput {
    #[validate(length(min = 1, max = "ID", message = "Atendimento invalido."))]
    pub appointment_id: String,
    #[validate(length(min = 1, max = "MINUSCULO", message = "Selecione o status."))]
    pub status: String,
    #[validate(length(min = 1, max = "MINUSCULO", message = "Selecione o metodo."))]
    pub method: String,
    #[validate(length(max = "MINUSCULO", message = "Data de pagamento invalida."))]
    pub paid_at: Option<String>,
    #[validate(range(min = 0, max = 100_000_000, message = "Valor recebido fora da faixa."))]
    pub amount_received_cents: i64,
    #[validate(length(max = "LONGO", message = "Observacoes longas demais."))]
    pub notes: Option<String>,
}

// ─── Session Records ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub appointment_id: String,
    pub encrypted_payload: String,
    pub iv: String,
    pub auth_tag: String,
    pub key_version: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SaveRecordInput {
    #[validate(length(min = 1, max = "ID", message = "Atendimento invalido."))]
    pub appointment_id: String,
    #[validate(length(min = 1, max = "ID", message = "Paciente invalido."))]
    pub patient_id: String,
    #[validate(length(max = "PRONTUARIO", message = "Prontuario longo demais."))]
    pub content: String,
}

// ─── Record Files ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordFile {
    pub id: String,
    pub user_id: String,
    pub patient_id: String,
    pub appointment_id: String,
    pub payment_id: Option<String>,
    pub kind: String,
    pub storage_path: String,
    pub original_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub uploaded_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FileUploadRequest {
    #[validate(length(min = 1, max = "ID", message = "Atendimento invalido."))]
    pub appointment_id: String,
    #[validate(length(min = 1, max = "ID", message = "Paciente invalido."))]
    pub patient_id: String,
    #[validate(length(max = "ID", message = "Pagamento invalido."))]
    pub payment_id: Option<String>,
    #[validate(length(min = 1, max = "MINUSCULO", message = "Tipo de anexo invalido."))]
    pub kind: String,
    #[validate(length(min = 1, max = "NOME_ARQUIVO", message = "Nome de arquivo invalido."))]
    pub file_name: String,
    #[validate(range(min = 0, message = "Tamanho de arquivo invalido."))]
    pub file_size: i64,
    #[validate(length(min = 1, max = "CURTO", message = "Tipo MIME invalido."))]
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSession {
    pub file_id: String,
    pub storage_path: String,
}

// ─── Audit Logs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: String,
    pub created_at: String,
}

// ─── Dashboard ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub upcoming_appointments: Vec<CalendarEvent>,
    pub todays_appointments: Vec<CalendarEvent>,
    pub financial_summary: FinancialSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialSummary {
    pub paid_cents: i64,
    pub pending_cents: i64,
    pub appointments_count: i64,
}

// ─── Export ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub exported_at: String,
    pub patient: serde_json::Value,
    pub appointments: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paciente_base() -> CreatePatientInput {
        CreatePatientInput {
            full_name: "Paciente de Teste".into(),
            chart_number: None,
            phone: None,
            email: None,
            birth_date: None,
            health_history: None,
            medications_in_use: None,
            emergency_phone: None,
            admin_notes: None,
        }
    }

    /// Sem teto no servidor, um `POST /api/patients` fora da interface aceitava um
    /// campo de megabytes — o unico limite era o `DefaultBodyLimit` global de 20 MB.
    #[test]
    fn rejects_oversized_free_text_fields() {
        let mut input = paciente_base();
        input.admin_notes = Some("a".repeat(LONGO as usize + 1));
        assert!(input.validate().is_err(), "admin_notes acima do limite");

        let mut input = paciente_base();
        input.health_history = Some("a".repeat(LONGO as usize));
        assert!(input.validate().is_ok(), "exatamente no limite deve passar");
    }

    #[test]
    fn rejects_oversized_single_line_fields() {
        let mut input = paciente_base();
        input.phone = Some("9".repeat(CURTO as usize + 1));
        assert!(input.validate().is_err(), "telefone absurdo");

        let mut input = paciente_base();
        input.full_name = "ab".into();
        assert!(input.validate().is_err(), "nome com menos de 3 caracteres");
    }

    /// Toda regra precisa de `message`: `errors.rs` descarta mensagens vazias e so
    /// inclui o campo se sobrou alguma, entao uma regra sem mensagem produz um 422
    /// com `errors: {}` — o cliente recebe "invalido" sem saber o que.
    #[test]
    fn every_violation_carries_a_message() {
        let mut input = paciente_base();
        input.full_name = String::new();
        input.admin_notes = Some("a".repeat(LONGO as usize + 1));

        let erros = input.validate().unwrap_err();
        let campos = erros.field_errors();
        assert!(campos.contains_key("full_name"));
        assert!(campos.contains_key("admin_notes"));

        for (campo, lista) in campos.iter() {
            for e in lista.iter() {
                assert!(
                    e.message.is_some(),
                    "a regra de '{campo}' precisa de message, senao a resposta sai vazia"
                );
            }
        }
    }

    /// `Option<Option<String>>` nao aceita os validadores padrao, entao estes dois
    /// campos passam por `custom` — e e facil o custom deixar de ser aplicado numa
    /// refatoracao sem ninguem notar.
    #[test]
    fn nullable_appointment_fields_are_bounded() {
        let base = UpdateAppointmentInput {
            patient_id: None,
            starts_at: None,
            ends_at: None,
            status: None,
            confirmation_status: None,
            session_price_cents: None,
            quick_notes: None,
            cancel_reason: None,
        };

        assert!(base.validate().is_ok(), "tudo ausente e valido");

        let mut input = base.clone();
        input.quick_notes = Some(None);
        assert!(input.validate().is_ok(), "null explicito limpa o campo");

        let mut input = base.clone();
        input.quick_notes = Some(Some("a".repeat(LONGO as usize + 1)));
        assert!(input.validate().is_err(), "texto acima do limite");

        let mut input = base;
        input.cancel_reason = Some(Some("motivo".into()));
        assert!(input.validate().is_ok());
    }

    #[test]
    fn recurrence_count_is_bounded() {
        let base = CreateAppointmentInput {
            patient_id: "550e8400-e29b-41d4-a716-446655440001".into(),
            starts_at: "2026-01-01T10:00:00".into(),
            ends_at: "2026-01-01T11:00:00".into(),
            status: None,
            confirmation_status: None,
            session_price_cents: None,
            quick_notes: None,
            cancel_reason: None,
            recurrence_frequency: None,
            recurrence_end_mode: None,
            recurrence_until_date: None,
            recurrence_occurrences: None,
        };

        let mut input = base.clone();
        input.recurrence_occurrences = Some(9999);
        assert!(
            input.validate().is_err(),
            "o gerador trunca em 52; aceitar 9999 e devolver 52 confunde quem chama"
        );

        let mut input = base;
        input.recurrence_occurrences = Some(52);
        assert!(input.validate().is_ok());
    }
}
