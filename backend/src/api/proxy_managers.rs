/// Admin API for pingora-proxy-manager instances.
///
/// pingora is deployed independently on a LB node (not in K3s).
/// Admins register the instance here; QuickStack then pushes routes via its REST API.
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    proxy::pingora::PingoraClient,
    state::AppState,
};

fn require_admin(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("global admin required".into()));
    }
    Ok(())
}

// ─── List ─────────────────────────────────────────────────────────────────────

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let rows = sqlx::query!(
        r#"SELECT id, name, host, api_base_url, api_username, is_active, created_at
           FROM proxy_managers ORDER BY name"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id":           r.id,
        "name":         r.name,
        "host":         r.host,
        "api_base_url": r.api_base_url,
        "api_username": r.api_username,
        "is_active":    r.is_active != 0,
        "created_at":   r.created_at,
    })).collect::<Vec<_>>())))
}

// ─── Get ──────────────────────────────────────────────────────────────────────

pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let r = sqlx::query!(
        r#"SELECT id, name, host, api_base_url, api_username, is_active, created_at
           FROM proxy_managers WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("proxy manager {id}")))?;

    Ok(Json(serde_json::json!({
        "id":           r.id,
        "name":         r.name,
        "host":         r.host,
        "api_base_url": r.api_base_url,
        "api_username": r.api_username,
        "is_active":    r.is_active != 0,
        "created_at":   r.created_at,
    })))
}

// ─── Create ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProxyManagerRequest {
    pub name: String,
    /// IP or hostname of the LB node running pingora
    pub host: String,
    /// Full URL of the pingora management API, e.g. http://10.0.0.5:81/api
    pub api_base_url: String,
    pub api_username: Option<String>,
    pub api_password: String,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateProxyManagerRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    // Verify connectivity before saving
    let client = PingoraClient::new(
        &body.api_base_url,
        body.api_username.as_deref().unwrap_or("admin"),
        &body.api_password,
    );
    if !client.health_check().await {
        return Err(AppError::BadRequest(format!(
            "cannot reach pingora at {} — check URL and credentials",
            body.api_base_url
        )));
    }

    let encrypted_pass = state.crypto.encrypt(&body.api_password)?;
    let id = Uuid::new_v4().to_string();

    sqlx::query!(
        r#"INSERT INTO proxy_managers (id, name, host, api_base_url, api_username, api_password)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        id,
        body.name,
        body.host,
        body.api_base_url,
        body.api_username.as_deref().unwrap_or("admin"),
        encrypted_pass,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("proxy manager name '{}' already exists", body.name))
        }
        other => AppError::Database(other),
    })?;

    // Hot-load if this is the first active manager
    let mut pingora_guard = state.pingora.write().await;
    if pingora_guard.is_none() {
        *pingora_guard = Some(client);
        tracing::info!("pingora client loaded: {}", body.api_base_url);
    }
    drop(pingora_guard);

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

// ─── Update ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateProxyManagerRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub api_base_url: Option<String>,
    pub api_username: Option<String>,
    pub api_password: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProxyManagerRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let encrypted_pass = body
        .api_password
        .as_deref()
        .map(|p| state.crypto.encrypt(p))
        .transpose()?;

    sqlx::query!(
        r#"UPDATE proxy_managers
           SET name         = COALESCE(?, name),
               host         = COALESCE(?, host),
               api_base_url = COALESCE(?, api_base_url),
               api_username = COALESCE(?, api_username),
               api_password = COALESCE(?, api_password),
               is_active    = COALESCE(?, is_active)
           WHERE id = ?"#,
        body.name,
        body.host,
        body.api_base_url,
        body.api_username,
        encrypted_pass,
        body.is_active.map(|v| v as i8),
        id,
    )
    .execute(&state.db)
    .await?;

    // Reload pingora client if credentials or URL changed
    if body.api_password.is_some() || body.api_base_url.is_some() {
        reload_active_client(&state).await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Delete ───────────────────────────────────────────────────────────────────

pub async fn delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    sqlx::query!(r#"DELETE FROM proxy_managers WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    // Reload (clears client if none active)
    reload_active_client(&state).await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Test connection ──────────────────────────────────────────────────────────

pub async fn test_connection(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let r = sqlx::query!(
        r#"SELECT api_base_url, api_username, api_password FROM proxy_managers WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("proxy manager {id}")))?;

    let password = state.crypto.decrypt(&r.api_password)?;
    let client = PingoraClient::new(&r.api_base_url, &r.api_username, &password);
    let ok = client.health_check().await;

    Ok(Json(serde_json::json!({ "reachable": ok })))
}

// ─── Internal helper ─────────────────────────────────────────────────────────

/// Reload the in-memory pingora client from the first active proxy_managers row.
async fn reload_active_client(state: &AppState) -> AppResult<()> {
    let row = sqlx::query!(
        r#"SELECT api_base_url, api_username, api_password
           FROM proxy_managers WHERE is_active = 1 ORDER BY created_at LIMIT 1"#
    )
    .fetch_optional(&state.db)
    .await?;

    let mut guard = state.pingora.write().await;
    *guard = match row {
        Some(r) => {
            let password = state.crypto.decrypt(&r.api_password)?;
            Some(PingoraClient::new(&r.api_base_url, &r.api_username, &password))
        }
        None => None,
    };
    Ok(())
}
