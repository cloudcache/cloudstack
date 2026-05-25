use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

/// GET /admin/registries
/// Returns registries ordered by priority ascending (highest-priority first).
pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT id, name, endpoint, username, is_default, priority, is_active, created_at
           FROM image_registries ORDER BY priority ASC, name ASC"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "endpoint": r.endpoint,
        "username": r.username,
        "is_default": r.is_default != 0,
        "priority": r.priority,
        "is_active": r.is_active != 0,
        "created_at": r.created_at,
    })).collect::<Vec<_>>())))
}

/// GET /admin/registries/:id
pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let r = sqlx::query!(
        r#"SELECT id, name, endpoint, username, is_default, priority, is_active, created_at
           FROM image_registries WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("registry {id}")))?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "name": r.name,
        "endpoint": r.endpoint,
        "username": r.username,
        "is_default": r.is_default != 0,
        "priority": r.priority,
        "is_active": r.is_active != 0,
        "created_at": r.created_at,
    })))
}

#[derive(Deserialize)]
pub struct CreateRegistryRequest {
    pub name: String,
    pub endpoint: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub is_default: Option<bool>,
    pub priority: Option<i16>,
}

/// POST /admin/registries
pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateRegistryRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let id = Uuid::new_v4().to_string();
    let is_default = body.is_default.unwrap_or(false) as i8;
    let priority = body.priority.unwrap_or(100);

    let enc_password = body.password.as_deref()
        .map(|p| state.crypto.encrypt(p))
        .transpose()?;

    sqlx::query!(
        r#"INSERT INTO image_registries (id, name, endpoint, username, password, is_default, priority)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id, body.name, body.endpoint, body.username, enc_password, is_default, priority,
    )
    .execute(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdateRegistryRequest {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub username: Option<String>,
    /// Pass null explicitly to clear the password
    pub password: Option<String>,
    pub is_default: Option<bool>,
    pub priority: Option<i16>,
    pub is_active: Option<bool>,
}

/// PUT /admin/registries/:id
pub async fn update_registry(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRegistryRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let existing = sqlx::query!(
        r#"SELECT name, endpoint, username, is_default, priority, is_active
           FROM image_registries WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("registry {id}")))?;

    let name = body.name.as_deref().unwrap_or(&existing.name);
    let endpoint = body.endpoint.as_deref().unwrap_or(&existing.endpoint);
    let is_default = body.is_default.map(|v| v as i8).unwrap_or(existing.is_default);
    let priority = body.priority.unwrap_or(existing.priority);
    let is_active = body.is_active.map(|v| v as i8).unwrap_or(existing.is_active);

    // username: explicit None in JSON means "keep"; we use Option semantics here
    let username = body.username.as_deref().or(existing.username.as_deref());

    if let Some(new_pass) = &body.password {
        let enc = state.crypto.encrypt(new_pass)?;
        sqlx::query!(
            r#"UPDATE image_registries
               SET name=?, endpoint=?, username=?, password=?, is_default=?, priority=?, is_active=?
               WHERE id=?"#,
            name, endpoint, username, enc, is_default, priority, is_active, id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            r#"UPDATE image_registries
               SET name=?, endpoint=?, username=?, is_default=?, priority=?, is_active=?
               WHERE id=?"#,
            name, endpoint, username, is_default, priority, is_active, id,
        )
        .execute(&state.db)
        .await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /admin/registries/:id/images
/// Fetches the catalog from a Docker Registry v2 API and returns image + tag lists.
pub async fn list_images(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let r = sqlx::query!(
        r#"SELECT endpoint, username, password FROM image_registries WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("registry {id}")))?;

    // Build HTTP client with optional basic auth
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let catalog_url = format!("{}/v2/_catalog", r.endpoint.trim_end_matches('/'));

    let mut req = client.get(&catalog_url);
    if let (Some(user), Some(enc_pass)) = (&r.username, &r.password) {
        let pass = state.crypto.decrypt(enc_pass)?;
        req = req.basic_auth(user, Some(pass));
    }

    let resp = req.send().await
        .map_err(|e| AppError::Internal(format!("registry request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(
            format!("registry returned {}", resp.status())
        ));
    }

    let body: serde_json::Value = resp.json().await
        .map_err(|e| AppError::Internal(format!("invalid registry response: {e}")))?;

    let repos = body.get("repositories")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Fetch tags for each repository (cap at 50 repos to avoid timeouts)
    let mut images = Vec::new();
    for repo in repos.iter().take(50) {
        let repo_name = match repo.as_str() {
            Some(s) => s,
            None => continue,
        };
        let tags_url = format!("{}/v2/{}/tags/list",
            r.endpoint.trim_end_matches('/'), repo_name);

        let mut tag_req = client.get(&tags_url);
        if let (Some(user), Some(enc_pass)) = (&r.username, &r.password) {
            let pass = state.crypto.decrypt(enc_pass)?;
            tag_req = tag_req.basic_auth(user, Some(pass));
        }

        if let Ok(tag_resp) = tag_req.send().await {
            if let Ok(tag_body) = tag_resp.json::<serde_json::Value>().await {
                images.push(serde_json::json!({
                    "name": repo_name,
                    "tags": tag_body.get("tags").cloned().unwrap_or(serde_json::json!([])),
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({ "images": images })))
}

/// DELETE /admin/registries/:id
pub async fn delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    sqlx::query!(r#"DELETE FROM image_registries WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
