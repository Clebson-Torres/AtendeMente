use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest {
        message: String,
        code: String,
    },
    #[error("{message}")]
    Unauthorized {
        message: String,
        code: String,
    },
    #[error("{message}")]
    NotFound {
        message: String,
        code: String,
    },
    #[error("{message}")]
    Conflict {
        message: String,
        code: String,
    },
    #[error("{message}")]
    RateLimited {
        message: String,
        code: String,
    },
    #[error("{message}")]
    Internal {
        message: String,
        code: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Validation(#[from] validator::ValidationErrors),
    #[error("Crypto error: {0}")]
    Crypto(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
            code: "BAD_REQUEST".into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized {
            message: message.into(),
            code: "UNAUTHORIZED".into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
            code: "NOT_FOUND".into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
            code: "CONFLICT".into(),
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::RateLimited {
            message: message.into(),
            code: "RATE_LIMITED".into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            code: "INTERNAL_ERROR".into(),
            source: None,
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Crypto(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}

/// Shown instead of the internal error text, which routinely embeds SQL, column
/// names and filesystem paths. The detail is logged server-side.
const GENERIC_INTERNAL_MESSAGE: &str =
    "Ocorreu um erro interno. Tente novamente; se persistir, consulte os logs do aplicativo.";

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        // Log the full detail, then decide what the client is allowed to see.
        let message = match &self {
            Self::Internal { message, .. } => {
                tracing::error!("[AppError::Internal] {}", message);
                GENERIC_INTERNAL_MESSAGE.to_string()
            }
            Self::Database(e) => {
                tracing::error!("[AppError::Database] {}", e);
                GENERIC_INTERNAL_MESSAGE.to_string()
            }
            Self::Crypto(detail) => {
                tracing::error!("[AppError::Crypto] {}", detail);
                GENERIC_INTERNAL_MESSAGE.to_string()
            }
            // BadRequest / Unauthorized / NotFound / Conflict / RateLimited carry
            // messages written for the user.
            other => other.to_string(),
        };

        let body = match self {
            Self::Validation(ref errs) => {
                let mut errors = HashMap::new();
                for (field, errs) in errs.field_errors() {
                    let msgs: Vec<String> = errs
                        .iter()
                        .map(|e| e.message.as_ref().map(|m| m.to_string()).unwrap_or_default())
                        .filter(|m| !m.is_empty())
                        .collect();
                    if !msgs.is_empty() {
                        errors.insert(field.to_string(), msgs);
                    }
                }
                Json(ErrorResponse {
                    success: false,
                    message: "Verifique os dados informados.".into(),
                    errors: Some(errors),
                })
            }
            _ => Json(ErrorResponse {
                success: false,
                message,
                errors: None,
            }),
        };

        (status, body).into_response()
    }
}

#[derive(Serialize)]
pub struct PaginatedData<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Serialize)]
pub struct ActionResponse<T: Serialize = ()> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;

    async fn render(err: AppError) -> (StatusCode, serde_json::Value) {
        let res = err.into_response();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn user_facing_errors_keep_their_message() {
        for (err, expected_status) in [
            (AppError::bad_request("Informe o nome completo."), StatusCode::BAD_REQUEST),
            (AppError::unauthorized("Token nao informado."), StatusCode::UNAUTHORIZED),
            (AppError::not_found("Paciente nao encontrado."), StatusCode::NOT_FOUND),
            (AppError::conflict("Ja existe um paciente."), StatusCode::CONFLICT),
            (
                AppError::rate_limited("Muitas tentativas em pouco tempo."),
                StatusCode::TOO_MANY_REQUESTS,
            ),
        ] {
            let original = err.to_string();
            let (status, body) = render(err).await;
            assert_eq!(status, expected_status);
            assert_eq!(body["success"], false);
            assert_eq!(
                body["message"], original,
                "mensagens escritas para o usuario devem chegar intactas"
            );
        }
    }

    /// Internal detail must never reach the client: these messages routinely embed
    /// SQL, column names and filesystem paths.
    #[tokio::test]
    async fn internal_errors_are_sanitized() {
        let leaky = "DB error: no such column: patients.pii_encrypted \
                     em C:\\Users\\alguem\\.config\\atendemente\\data";
        let (status, body) = render(AppError::internal(leaky)).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let msg = body["message"].as_str().unwrap();
        assert_eq!(msg, GENERIC_INTERNAL_MESSAGE);
        for vazamento in ["no such column", "pii_encrypted", "C:\\Users", "SQL", "DB error"] {
            assert!(
                !msg.contains(vazamento),
                "mensagem vazou {vazamento:?}: {msg}"
            );
        }
    }

    #[tokio::test]
    async fn database_errors_are_sanitized() {
        let err = AppError::Database(sqlx::Error::RowNotFound);
        let (status, body) = render(err).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["message"], GENERIC_INTERNAL_MESSAGE);
    }

    #[tokio::test]
    async fn crypto_errors_are_sanitized() {
        let (status, body) = render(AppError::Crypto("chave derivada invalida".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["message"], GENERIC_INTERNAL_MESSAGE);
    }

    #[tokio::test]
    async fn validation_errors_report_fields_without_leaking_internals() {
        let mut errs = validator::ValidationErrors::new();
        let mut e = validator::ValidationError::new("min");
        e.message = Some("Minimo 3 caracteres.".into());
        errs.add("full_name", e);

        let (status, body) = render(AppError::Validation(errs)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["message"], "Verifique os dados informados.");
        assert_eq!(body["errors"]["full_name"][0], "Minimo 3 caracteres.");
    }

    #[tokio::test]
    async fn successful_responses_omit_empty_fields() {
        let ok = ActionResponse::success("Salvo.", serde_json::json!({"id": 1}));
        let json = serde_json::to_value(&ok).unwrap();
        assert_eq!(json["success"], true);
        assert!(json.get("errors").is_none(), "errors deve ser omitido quando None");

        let empty = ActionResponse::<()>::success_empty("Feito.");
        let json = serde_json::to_value(&empty).unwrap();
        assert!(json.get("data").is_none(), "data deve ser omitido quando None");
    }
}

impl<T: Serialize> ActionResponse<T> {
    pub fn success(message: impl Into<String>, data: T) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            errors: None,
        }
    }

    pub fn success_empty(message: impl Into<String>) -> ActionResponse<()> {
        ActionResponse {
            success: true,
            message: message.into(),
            data: None,
            errors: None,
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            errors: None,
        }
    }
}
