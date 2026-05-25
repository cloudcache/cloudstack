use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    auth::jwt,
    crypto::CryptoService,
    error::{AppError, AppResult},
    state::AppState,
};

/// Resolved caller, injected into handlers via `Extension<AuthUser>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub is_global_admin: bool,
}

pub async fn require_auth(
    State(state): State<AppState>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    let token = auth.token();
    let secret = state.jwt_secret.read().await.clone();
    let claims = jwt::verify(token, &secret)?;

    let token_hash = CryptoService::sha256_hex(token);

    // Validate session exists and is not expired
    let session = sqlx::query!(
        r#"SELECT u.id, u.username, u.is_global_admin, u.is_active
           FROM user_sessions s
           JOIN users u ON u.id = s.user_id
           WHERE s.token_hash = ? AND s.expires_at > NOW()"#,
        token_hash
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("session not found or expired".into()))?;

    if session.is_active == 0 {
        return Err(AppError::Forbidden("account disabled".into()));
    }

    req.extensions_mut().insert(AuthUser {
        user_id: session.id,
        username: session.username,
        is_global_admin: session.is_global_admin != 0,
    });

    Ok(next.run(req).await)
}

/// Helper — extract AuthUser from request extensions (already inserted by middleware).
pub fn auth_user(req: &Request) -> AppResult<&AuthUser> {
    req.extensions()
        .get::<AuthUser>()
        .ok_or_else(|| AppError::Unauthorized("missing auth context".into()))
}
