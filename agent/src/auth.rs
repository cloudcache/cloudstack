use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// Tower middleware: validates Bearer token against the configured agent token.
pub async fn require_token(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = request
        .extensions()
        .get::<AgentToken>()
        .map(|t| t.0.as_str())
        .unwrap_or("");

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !auth_header.starts_with("Bearer ") || &auth_header[7..] != expected {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

/// Holds the agent token for injection into request extensions.
#[derive(Clone)]
pub struct AgentToken(pub String);
