//! App-template CRUD.
//!
//! Visibility tiers:
//!   - PUBLIC  : maintained by global admin; visible to everyone
//!   - ORG     : visible to members of `owner_project_id`
//!   - PRIVATE : visible only to `owner_user_id`
//!
//! Routes:
//!   GET    /api/v1/templates                — list all visible to the caller
//!   GET    /api/v1/templates/:id            — fetch one (visibility-gated)
//!   POST   /api/v1/admin/templates          — admin only, PUBLIC
//!   PUT    /api/v1/admin/templates/:id      — admin only
//!   DELETE /api/v1/admin/templates/:id      — admin only
//!   POST   /api/v1/projects/:pid/templates  — project owner, ORG/PRIVATE
//!   PUT    /api/v1/projects/:pid/templates/:id
//!   DELETE /api/v1/projects/:pid/templates/:id
//!
//! Phase 2 (binding to managed services) lives in a separate module.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Serialize, sqlx::FromRow)]
struct TemplateRow {
    id: String,
    slug: String,
    name: String,
    icon_url: Option<String>,
    category: String,
    description: Option<String>,
    visibility: String,
    owner_user_id: Option<String>,
    owner_project_id: Option<String>,
    image_registry_id: Option<String>,
    image_repository: String,
    image_tag: String,
    image_digest: Option<String>,
    spec: serde_json::Value,
    requirements: serde_json::Value,
    inputs: serde_json::Value,
    is_active: i8,
    version: i32,
}

const SELECT_COLS: &str = "id, slug, name, icon_url, category, description, visibility, \
     owner_user_id, owner_project_id, \
     image_registry_id, image_repository, image_tag, image_digest, \
     spec, requirements, inputs, is_active, version";

/// Render the canonical image reference: `[registry_endpoint/]repo[:tag][@digest]`.
/// Falls back to bare `repo:tag` if the registry can't be resolved.
pub(crate) async fn render_image_ref(
    state: &AppState,
    registry_id: Option<&str>,
    repo: &str,
    tag: &str,
    digest: Option<&str>,
) -> String {
    let registry_endpoint = if let Some(rid) = registry_id {
        sqlx::query_scalar::<_, String>(
            "SELECT endpoint FROM image_registries WHERE id = ? AND is_active = 1",
        )
        .bind(rid)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let mut out = match registry_endpoint {
        Some(ep) if !ep.is_empty() && ep != "docker.io" => format!("{ep}/{repo}"),
        _ => repo.to_string(),
    };
    if !tag.is_empty() {
        out.push(':');
        out.push_str(tag);
    }
    if let Some(d) = digest {
        if !d.is_empty() {
            out.push('@');
            out.push_str(d);
        }
    }
    out
}

async fn row_json(state: &AppState, r: &TemplateRow) -> serde_json::Value {
    let image_ref = render_image_ref(
        state,
        r.image_registry_id.as_deref(),
        &r.image_repository,
        &r.image_tag,
        r.image_digest.as_deref(),
    )
    .await;

    serde_json::json!({
        "id":               r.id,
        "slug":             r.slug,
        "name":             r.name,
        "icon_url":         r.icon_url,
        "category":         r.category,
        "description":      r.description,
        "visibility":       r.visibility,
        "owner_user_id":    r.owner_user_id,
        "owner_project_id": r.owner_project_id,
        "image_registry_id": r.image_registry_id,
        "image_repository": r.image_repository,
        "image_tag":        r.image_tag,
        "image_digest":     r.image_digest,
        // Convenience: server-rendered full ref. Clients can either use this
        // directly or recompose from the structured fields.
        "image_ref":        image_ref,
        "spec":             r.spec,
        "requirements":     r.requirements,
        "inputs":           r.inputs,
        "is_active":        r.is_active != 0,
        "version":          r.version,
    })
}

// ── List ────────────────────────────────────────────────────────────────────

/// GET /api/v1/templates
/// Returns every template the caller can see: PUBLIC + own PRIVATE +
/// templates whose owner_project_id is one of the caller's projects.
pub async fn list_visible(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows: Vec<TemplateRow> = sqlx::query_as(
        &format!(
            "SELECT {SELECT_COLS} FROM app_templates \
             WHERE is_active = 1 AND ( \
                  visibility = 'PUBLIC' \
                  OR (visibility = 'PRIVATE' AND owner_user_id = ?) \
                  OR (visibility = 'ORG' AND owner_project_id IN \
                      (SELECT project_id FROM project_members WHERE user_id = ?)) \
             ) ORDER BY category, name"
        ),
    )
    .bind(&auth.user_id)
    .bind(&auth.user_id)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(row_json(&state, r).await);
    }
    Ok(Json(out))
}

/// GET /api/v1/templates/:id
pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let row: TemplateRow = sqlx::query_as(
        &format!("SELECT {SELECT_COLS} FROM app_templates WHERE id = ?"),
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("template {id}")))?;

    // Visibility gate
    let can_see = match row.visibility.as_str() {
        "PUBLIC" => true,
        "PRIVATE" => row.owner_user_id.as_deref() == Some(&auth.user_id),
        "ORG" => {
            if let Some(pid) = &row.owner_project_id {
                let is_member: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND user_id = ?",
                )
                .bind(pid)
                .bind(&auth.user_id)
                .fetch_one(&state.db)
                .await?;
                is_member > 0
            } else {
                false
            }
        }
        _ => false,
    };

    if !can_see && !auth.is_global_admin {
        return Err(AppError::Forbidden("template not visible to you".into()));
    }

    Ok(Json(row_json(&state, &row).await))
}

// ── Create / Update / Delete ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TemplateUpsert {
    pub slug: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    // Image identity — first-class, not buried in inputs.
    pub image_registry_id: Option<String>,
    pub image_repository: String,
    pub image_tag: Option<String>,
    pub image_digest: Option<String>,
    pub spec: serde_json::Value,
    pub requirements: Option<serde_json::Value>,
    pub inputs: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

async fn insert_template(
    state: &AppState,
    body: TemplateUpsert,
    visibility: &str,
    owner_user_id: Option<&str>,
    owner_project_id: Option<&str>,
) -> AppResult<String> {
    if body.slug.is_empty() || body.name.is_empty() {
        return Err(AppError::BadRequest("slug and name are required".into()));
    }
    if !body.slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest(
            "slug may contain only alphanumerics, '-' or '_'".into(),
        ));
    }
    if body.image_repository.is_empty() {
        return Err(AppError::BadRequest("image_repository is required".into()));
    }

    let id = Uuid::new_v4().to_string();
    let category = body.category.as_deref().unwrap_or("app");
    let requirements = body
        .requirements
        .unwrap_or_else(|| serde_json::json!([]))
        .to_string();
    let inputs = body
        .inputs
        .unwrap_or_else(|| serde_json::json!([]))
        .to_string();
    let spec = body.spec.to_string();
    let image_tag = body.image_tag.as_deref().unwrap_or("latest");

    sqlx::query(
        "INSERT INTO app_templates \
            (id, slug, name, icon_url, category, description, visibility, \
             owner_user_id, owner_project_id, \
             image_registry_id, image_repository, image_tag, image_digest, \
             spec, requirements, inputs, is_active) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&body.slug)
    .bind(&body.name)
    .bind(&body.icon_url)
    .bind(category)
    .bind(&body.description)
    .bind(visibility)
    .bind(owner_user_id)
    .bind(owner_project_id)
    .bind(&body.image_registry_id)
    .bind(&body.image_repository)
    .bind(image_tag)
    .bind(&body.image_digest)
    .bind(&spec)
    .bind(&requirements)
    .bind(&inputs)
    .bind(if body.is_active.unwrap_or(true) { 1i8 } else { 0i8 })
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("template slug '{}' already exists", body.slug))
        }
        other => AppError::Database(other),
    })?;

    Ok(id)
}

async fn update_template(
    state: &AppState,
    id: &str,
    body: TemplateUpsert,
) -> AppResult<()> {
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_templates WHERE id = ?")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("template {id}")));
    }

    if body.image_repository.is_empty() {
        return Err(AppError::BadRequest("image_repository is required".into()));
    }

    let category = body.category.as_deref().unwrap_or("app");
    let requirements = body
        .requirements
        .unwrap_or_else(|| serde_json::json!([]))
        .to_string();
    let inputs = body
        .inputs
        .unwrap_or_else(|| serde_json::json!([]))
        .to_string();
    let spec = body.spec.to_string();
    let image_tag = body.image_tag.as_deref().unwrap_or("latest");

    sqlx::query(
        "UPDATE app_templates SET \
             slug=?, name=?, icon_url=?, category=?, description=?, \
             image_registry_id=?, image_repository=?, image_tag=?, image_digest=?, \
             spec=?, requirements=?, inputs=?, is_active=?, version=version+1 \
         WHERE id=?",
    )
    .bind(&body.slug)
    .bind(&body.name)
    .bind(&body.icon_url)
    .bind(category)
    .bind(&body.description)
    .bind(&body.image_registry_id)
    .bind(&body.image_repository)
    .bind(image_tag)
    .bind(&body.image_digest)
    .bind(&spec)
    .bind(&requirements)
    .bind(&inputs)
    .bind(if body.is_active.unwrap_or(true) { 1i8 } else { 0i8 })
    .bind(id)
    .execute(&state.db)
    .await?;

    Ok(())
}

// Admin (PUBLIC) — global admin only

pub async fn admin_create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<TemplateUpsert>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    let id = insert_template(&state, body, "PUBLIC", None, None).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

pub async fn admin_update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<TemplateUpsert>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    update_template(&state, &id, body).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn admin_delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    sqlx::query("DELETE FROM app_templates WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// Project-scoped — OWNER role required

pub async fn project_create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(body): Json<TemplateUpsert>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OWNER").await?;
    let id = insert_template(
        &state,
        body,
        "ORG",
        Some(&auth.user_id),
        Some(&project_id),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

pub async fn project_update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, id)): Path<(String, String)>,
    Json(body): Json<TemplateUpsert>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OWNER").await?;

    // Ownership check: template must belong to this project
    let owner: Option<String> = sqlx::query_scalar(
        "SELECT owner_project_id FROM app_templates WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    if owner.as_deref() != Some(&project_id) {
        return Err(AppError::NotFound(format!("template {id}")));
    }

    update_template(&state, &id, body).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn project_delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OWNER").await?;

    let owner: Option<String> = sqlx::query_scalar(
        "SELECT owner_project_id FROM app_templates WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    if owner.as_deref() != Some(&project_id) {
        return Err(AppError::NotFound(format!("template {id}")));
    }

    sqlx::query("DELETE FROM app_templates WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
