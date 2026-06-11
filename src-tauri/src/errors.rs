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

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let message = self.to_string();

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
pub struct ActionResponse<T: Serialize = ()> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
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
