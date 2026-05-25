use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

/// GET /admin/resource-pools
pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.display_name, p.region, p.description, p.is_active, p.created_at,
                  COUNT(DISTINCT c.id) AS cluster_count,
                  COUNT(DISTINCT n.id) AS node_count
           FROM resource_pools p
           LEFT JOIN clusters c ON c.pool_id = p.id AND c.is_active = 1
           LEFT JOIN cluster_nodes n ON n.cluster_id = c.id
           GROUP BY p.id ORDER BY p.name"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "region": r.region,
        "description": r.description,
        "is_active": r.is_active != 0,
        "cluster_count": r.cluster_count,
        "node_count": r.node_count,
        "created_at": r.created_at,
    })).collect::<Vec<_>>())))
}

/// GET /admin/resource-pools/:id
pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let r = sqlx::query!(
        r#"SELECT id, name, display_name, region, description, is_active, created_at
           FROM resource_pools WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("resource pool {id}")))?;

    let clusters = sqlx::query!(
        r#"SELECT id, name, display_name, is_active FROM clusters WHERE pool_id = ? ORDER BY name"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "region": r.region,
        "description": r.description,
        "is_active": r.is_active != 0,
        "created_at": r.created_at,
        "clusters": clusters.iter().map(|c| serde_json::json!({
            "id": c.id,
            "name": c.name,
            "display_name": c.display_name,
            "is_active": c.is_active != 0,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct CreatePoolRequest {
    pub name: String,
    pub display_name: String,
    pub region: Option<String>,
    pub description: Option<String>,
}

/// POST /admin/resource-pools
pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreatePoolRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    if body.name.is_empty() || !body.name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest("name must be alphanumeric slug (a-z, 0-9, -, _)".into()));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO resource_pools (id, name, display_name, region, description)
           VALUES (?, ?, ?, ?, ?)"#,
        id, body.name, body.display_name, body.region, body.description,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() =>
            AppError::Conflict(format!("pool '{}' already exists", body.name)),
        other => AppError::Database(other),
    })?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdatePoolRequest {
    pub display_name: Option<String>,
    pub region: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

/// PUT /admin/resource-pools/:id
pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePoolRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let existing = sqlx::query!(
        r#"SELECT display_name, region, description, is_active FROM resource_pools WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("resource pool {id}")))?;

    let display_name = body.display_name.as_deref().unwrap_or(&existing.display_name);
    let region = body.region.as_deref().or(existing.region.as_deref());
    let description = body.description.as_deref().or(existing.description.as_deref());
    let is_active = body.is_active.map(|v| v as i8).unwrap_or(existing.is_active);

    sqlx::query!(
        r#"UPDATE resource_pools SET display_name=?, region=?, description=?, is_active=? WHERE id=?"#,
        display_name, region, description, is_active, id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /admin/resource-pools/:id  (refused if clusters exist)
pub async fn delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let cluster_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM clusters WHERE pool_id = ?"#, id
    )
    .fetch_one(&state.db)
    .await?;

    if cluster_count > 0 {
        return Err(AppError::Conflict(
            format!("pool has {cluster_count} cluster(s); remove them first")
        ));
    }

    sqlx::query!(r#"DELETE FROM resource_pools WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
