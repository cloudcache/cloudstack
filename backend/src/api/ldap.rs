use axum::{extract::State, response::IntoResponse, Extension, Json};

use crate::{
    auth::{ldap::LdapService, ldap_sync, middleware::AuthUser},
    error::{AppError, AppResult},
    state::AppState,
};

fn require_admin(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("global admin required".into()));
    }
    Ok(())
}

pub async fn test_connection(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let ldap = LdapService::new(state.config.ldap.clone());
    let users = ldap.list_users().await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "user_count": users.len(),
    })))
}

pub async fn sync(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let report = ldap_sync::sync_ldap_users(&state.db, &state.config.ldap).await?;

    Ok(Json(report))
}
