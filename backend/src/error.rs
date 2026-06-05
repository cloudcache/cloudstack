use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("k8s error: {0}")]
    Kubernetes(#[from] kube::Error),

    #[error("ldap error: {0}")]
    Ldap(#[from] ldap3::LdapError),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("ssh error: {0}")]
    Ssh(String),

    #[error("proxy error: {0}")]
    Proxy(String),

    #[error("docker error: {0}")]
    Docker(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::QuotaExceeded(_) => StatusCode::UNPROCESSABLE_ENTITY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::Forbidden(_) => "FORBIDDEN",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::QuotaExceeded(_) => "QUOTA_EXCEEDED",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Kubernetes(_) => "KUBERNETES_ERROR",
            AppError::Ldap(_) => "LDAP_ERROR",
            AppError::Crypto(_) => "CRYPTO_ERROR",
            AppError::Ssh(_) => "SSH_ERROR",
            AppError::Proxy(_) => "PROXY_ERROR",
            AppError::Docker(_) => "DOCKER_ERROR",
            AppError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.error_code();

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!("internal error: {}", self);
        }

        let body = Json(json!({
            "error": {
                "code": code,
                "message": self.to_string()
            }
        }));
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
