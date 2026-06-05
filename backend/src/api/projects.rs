/// Project CRUD + member management.
///
/// Access levels:
///   OBSERVER  — read project/members/apps, no write
///   OPERATOR  — OBSERVER + deploy/manage apps and databases
///   ADMIN     — OPERATOR + invite/remove members, update display_name, delete project
///   (global admin bypasses all project-level checks)
///
/// Quota fields (cpu, mem, storage, apps, db_instances) are only writable by
/// global admin — never by project-level ADMIN.
use axum::{
    extract::{Path, Query, State},
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

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn slug_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && name.starts_with(|c: char| c.is_ascii_lowercase())
}

fn validate_role(role: &str) -> AppResult<()> {
    match role {
        "ADMIN" | "OPERATOR" | "OBSERVER" => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "invalid role '{role}'; valid: ADMIN OPERATOR OBSERVER"
        ))),
    }
}

pub async fn project_role(
    state: &AppState,
    user_id: &str,
    project_id: &str,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT role FROM project_members WHERE project_id = ? AND user_id = ?"#,
        project_id,
        user_id,
    )
    .fetch_optional(&state.db)
    .await?)
}

/// Returns Ok if the calling user has at least `min_role` in the project,
/// or is a global admin.
///   min_role: "OBSERVER" | "OPERATOR" | "ADMIN"
pub async fn check_project_access(
    state: &AppState,
    auth: &AuthUser,
    project_id: &str,
    min_role: &str,
) -> AppResult<()> {
    if auth.is_global_admin {
        return Ok(());
    }

    // Project owner always has implicit ADMIN access, even if not in project_members
    let is_owner = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM projects WHERE id = ? AND owner_id = ?"#,
        project_id,
        auth.user_id,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if is_owner > 0 {
        return Ok(()); // owner ≥ ADMIN, satisfies any min_role
    }

    let role = project_role(state, &auth.user_id, project_id).await?;
    let Some(role) = role else {
        return Err(AppError::Forbidden("not a member of this project".into()));
    };
    let level = role_level(&role);
    if level < role_level(min_role) {
        return Err(AppError::Forbidden(format!("{min_role} role required")));
    }
    Ok(())
}

fn role_level(role: &str) -> u8 {
    match role {
        "OBSERVER" => 1,
        "OPERATOR" => 2,
        "ADMIN" => 3,
        _ => 0,
    }
}

async fn admin_count(state: &AppState, project_id: &str) -> AppResult<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM project_members WHERE project_id = ? AND role = 'ADMIN'"#,
        project_id
    )
    .fetch_one(&state.db)
    .await?)
}

// ─── User: list my projects ───────────────────────────────────────────────────

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    // Global admin sees all projects; regular users see only their memberships
    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.display_name, p.owner_id,
                  u.username AS owner_username,
                  p.is_active, p.is_default,
                  p.quota_cpu_mcores, p.quota_mem_mb, p.quota_storage_gb,
                  p.quota_apps, p.quota_db_instances,
                  p.quota_bandwidth_gb, p.quota_domain_count, p.quota_request_million,
                  pm.role AS my_role,
                  (SELECT COUNT(*) FROM project_members WHERE project_id = p.id) AS member_count,
                  p.created_at
           FROM projects p
           LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
           JOIN users u ON u.id = p.owner_id
           WHERE pm.user_id IS NOT NULL OR ? = 1
           ORDER BY p.display_name"#,
        auth.user_id,
        auth.is_global_admin
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "owner_id": r.owner_id,
        "owner_username": r.owner_username,
        "is_active": r.is_active.unwrap_or(0) != 0,
        "is_default": r.is_default.unwrap_or(0) != 0,
        "my_role": r.my_role.as_deref().unwrap_or(if auth.is_global_admin { "ADMIN" } else { "" }),
        "member_count": r.member_count,
        "quota": {
            "cpu_mcores":     r.quota_cpu_mcores,
            "mem_mb":         r.quota_mem_mb,
            "storage_gb":     r.quota_storage_gb,
            "apps":           r.quota_apps,
            "db_instances":   r.quota_db_instances,
            "bandwidth_gb":   r.quota_bandwidth_gb,
            "domain_count":   r.quota_domain_count,
            "request_million": r.quota_request_million,
        },
        "created_at": r.created_at,
    })).collect::<Vec<_>>()),
    ))
}

// ─── User: get project ────────────────────────────────────────────────────────

pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &id, "OBSERVER").await?;

    let r = sqlx::query!(
        r#"SELECT p.id, p.name, p.display_name, p.owner_id,
                  u.username AS owner_username,
                  p.is_active, p.is_default,
                  p.quota_cpu_mcores, p.quota_mem_mb, p.quota_storage_gb,
                  p.quota_apps, p.quota_db_instances,
                  p.quota_bandwidth_gb, p.quota_domain_count, p.quota_request_million,
                  p.created_at, p.updated_at
           FROM projects p
           JOIN users u ON u.id = p.owner_id
           WHERE p.id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {id}")))?;

    let my_role = project_role(&state, &auth.user_id, &id).await?;

    let members = sqlx::query!(
        r#"SELECT pm.user_id, pm.role, u.username, u.display_name, u.email,
                  pm.added_at,
                  ab.username AS added_by_username
           FROM project_members pm
           JOIN users u ON u.id = pm.user_id
           LEFT JOIN users ab ON ab.id = pm.added_by
           WHERE pm.project_id = ?
           ORDER BY pm.added_at"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "owner_id": r.owner_id,
        "owner_username": r.owner_username,
        "is_active": r.is_active != 0,
        "is_default": r.is_default != 0,
        "my_role": my_role,
        "quota": {
            "cpu_mcores":     r.quota_cpu_mcores,
            "mem_mb":         r.quota_mem_mb,
            "storage_gb":     r.quota_storage_gb,
            "apps":           r.quota_apps,
            "db_instances":   r.quota_db_instances,
            "bandwidth_gb":   r.quota_bandwidth_gb,
            "domain_count":   r.quota_domain_count,
            "request_million": r.quota_request_million,
        },
        "members": members.iter().map(|m| serde_json::json!({
            "user_id": m.user_id,
            "username": m.username,
            "display_name": m.display_name,
            "email": m.email,
            "role": m.role,
            "added_at": m.added_at,
            "added_by": m.added_by_username,
        })).collect::<Vec<_>>(),
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    })))
}

// ─── User: create project ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub display_name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateProjectRequest>,
) -> AppResult<impl IntoResponse> {
    // Platform setting: can regular users create projects?
    if !auth.is_global_admin {
        let allowed = sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'allow_user_create_projects'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "1".to_string());
        if allowed != "1" {
            return Err(AppError::Forbidden(
                "project creation is restricted to administrators".into(),
            ));
        }
    }

    if !slug_valid(&body.name) {
        return Err(AppError::BadRequest(
            "project name must be lowercase letters/digits/hyphens, start with a letter, max 63 chars".into(),
        ));
    }
    if body.display_name.trim().is_empty() {
        return Err(AppError::BadRequest("display_name is required".into()));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO projects (id, name, display_name, owner_id)
           VALUES (?, ?, ?, ?)"#,
        id,
        body.name,
        body.display_name.trim(),
        auth.user_id,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("project name '{}' already taken", body.name))
        }
        other => AppError::Database(other),
    })?;

    // Owner gets ADMIN role automatically
    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role) VALUES (?, ?, 'ADMIN')"#,
        id,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    // K8s namespace is created lazily on first app deploy (cluster not known at project creation time)

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "name": body.name })),
    ))
}

// ─── User: update project (display_name only) ─────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateProjectRequest {
    pub display_name: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectRequest>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &id, "ADMIN").await?;

    if let Some(ref dn) = body.display_name {
        if dn.trim().is_empty() {
            return Err(AppError::BadRequest("display_name cannot be empty".into()));
        }
    }

    sqlx::query!(
        r#"UPDATE projects SET display_name = COALESCE(?, display_name) WHERE id = ?"#,
        body.display_name.as_deref().map(str::trim),
        id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── User: delete project ─────────────────────────────────────────────────────

pub async fn delete_project(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &id, "ADMIN").await?;

    let ns = sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {id}")))?;

    // Block deletion if there are running apps
    let running: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM apps WHERE project_id = ? AND status NOT IN ('STOPPED','FAILED')"#,
        id
    )
    .fetch_one(&state.db)
    .await?;
    if running > 0 {
        return Err(AppError::BadRequest(format!(
            "cannot delete project with {running} running app(s) — stop them first"
        )));
    }

    // Best-effort: delete namespace from every cluster that ever hosted apps in this project
    let cluster_ids: Vec<String> = sqlx::query_scalar!(
        r#"SELECT DISTINCT c.id FROM clusters c
           JOIN apps a ON a.pool_id IN (SELECT id FROM resource_pools WHERE id = c.pool_id)
           WHERE a.project_id = ?"#,
        id
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for cid in cluster_ids {
        let _ = crate::k8s::namespace::delete_namespace(&state, &cid, &ns).await;
    }

    sqlx::query!(r#"DELETE FROM projects WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── User: leave project ──────────────────────────────────────────────────────

pub async fn leave_project(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let role = project_role(&state, &auth.user_id, &id).await?;
    if role.is_none() {
        return Err(AppError::BadRequest(
            "you are not a member of this project".into(),
        ));
    }

    // Cannot leave if you're the last ADMIN
    if role.as_deref() == Some("ADMIN") && admin_count(&state, &id).await? <= 1 {
        return Err(AppError::BadRequest(
            "cannot leave: you are the last ADMIN — transfer ownership or delete the project first"
                .into(),
        ));
    }

    // Cannot leave if you're the project owner
    let owner_id: String = sqlx::query_scalar!(r#"SELECT owner_id FROM projects WHERE id = ?"#, id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {id}")))?;

    if owner_id == auth.user_id {
        return Err(AppError::BadRequest(
            "project owner cannot leave — transfer ownership first".into(),
        ));
    }

    sqlx::query!(
        r#"DELETE FROM project_members WHERE project_id = ? AND user_id = ?"#,
        id,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── User: transfer ownership ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TransferOwnerRequest {
    pub new_owner_id: String,
}

pub async fn transfer_owner(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<TransferOwnerRequest>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &id, "ADMIN").await?;

    // Only the current owner or global admin can transfer
    if !auth.is_global_admin {
        let owner_id: String =
            sqlx::query_scalar!(r#"SELECT owner_id FROM projects WHERE id = ?"#, id)
                .fetch_optional(&state.db)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("project {id}")))?;
        if owner_id != auth.user_id {
            return Err(AppError::Forbidden(
                "only the project owner can transfer ownership".into(),
            ));
        }
    }

    // New owner must be an existing project member
    let new_role = project_role(&state, &body.new_owner_id, &id).await?;
    if new_role.is_none() {
        return Err(AppError::BadRequest(
            "new owner must already be a project member".into(),
        ));
    }

    // Ensure new owner has ADMIN role
    sqlx::query!(
        r#"UPDATE project_members SET role = 'ADMIN' WHERE project_id = ? AND user_id = ?"#,
        id,
        body.new_owner_id,
    )
    .execute(&state.db)
    .await?;

    sqlx::query!(
        r#"UPDATE projects SET owner_id = ? WHERE id = ?"#,
        body.new_owner_id,
        id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Members: list ────────────────────────────────────────────────────────────

pub async fn list_members(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT pm.user_id, pm.role,
                  u.username, u.display_name, u.email, u.is_active,
                  pm.added_at,
                  ab.username AS added_by_username
           FROM project_members pm
           JOIN users u ON u.id = pm.user_id
           LEFT JOIN users ab ON ab.id = pm.added_by
           WHERE pm.project_id = ?
           ORDER BY
             FIELD(pm.role,'ADMIN','OPERATOR','OBSERVER'),
             u.username"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "user_id": r.user_id,
            "username": r.username,
            "display_name": r.display_name,
            "email": r.email,
            "is_active": r.is_active != 0,
            "role": r.role,
            "added_at": r.added_at,
            "added_by": r.added_by_username,
        }))
        .collect::<Vec<_>>())))
}

// ─── Members: invite / add ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddMemberRequest {
    /// Lookup by user_id takes priority; fall back to username lookup
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub role: String,
}

pub async fn add_member(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &project_id, "ADMIN").await?;
    validate_role(&body.role)?;

    // Resolve target user
    let target_id = if let Some(ref uid) = body.user_id {
        let exists = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM users WHERE id = ? AND is_active = 1"#,
            uid
        )
        .fetch_one(&state.db)
        .await?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("user {uid}")));
        }
        uid.clone()
    } else if let Some(ref uname) = body.username {
        sqlx::query_scalar!(
            r#"SELECT id FROM users WHERE username = ? AND is_active = 1"#,
            uname
        )
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user '{uname}'")))?
    } else {
        return Err(AppError::BadRequest(
            "provide either user_id or username".into(),
        ));
    };

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role, added_by)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE role = VALUES(role), added_by = VALUES(added_by)"#,
        project_id,
        target_id,
        body.role,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Members: update role ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
}

pub async fn update_member(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, user_id)): Path<(String, String)>,
    Json(body): Json<UpdateMemberRequest>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &project_id, "ADMIN").await?;
    validate_role(&body.role)?;

    // Prevent demoting the last ADMIN
    if body.role != "ADMIN" {
        let current_role = project_role(&state, &user_id, &project_id).await?;
        if current_role.as_deref() == Some("ADMIN") && admin_count(&state, &project_id).await? <= 1
        {
            return Err(AppError::BadRequest(
                "cannot demote: this user is the last ADMIN of the project".into(),
            ));
        }
    }

    // Prevent demoting project owner
    let owner_id: String =
        sqlx::query_scalar!(r#"SELECT owner_id FROM projects WHERE id = ?"#, project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    if user_id == owner_id && body.role != "ADMIN" {
        return Err(AppError::BadRequest(
            "project owner must retain ADMIN role — transfer ownership first".into(),
        ));
    }

    let result = sqlx::query!(
        r#"UPDATE project_members SET role = ? WHERE project_id = ? AND user_id = ?"#,
        body.role,
        project_id,
        user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "member {user_id} in project {project_id}"
        )));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Members: remove ──────────────────────────────────────────────────────────

pub async fn remove_member(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, user_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    check_project_access(&state, &auth, &project_id, "ADMIN").await?;

    // Cannot remove project owner
    let owner_id: String =
        sqlx::query_scalar!(r#"SELECT owner_id FROM projects WHERE id = ?"#, project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    if user_id == owner_id {
        return Err(AppError::BadRequest(
            "cannot remove the project owner — transfer ownership first".into(),
        ));
    }

    // Cannot remove the last ADMIN
    let current_role = project_role(&state, &user_id, &project_id).await?;
    if current_role.as_deref() == Some("ADMIN") && admin_count(&state, &project_id).await? <= 1 {
        return Err(AppError::BadRequest(
            "cannot remove: this user is the last ADMIN of the project".into(),
        ));
    }

    let result = sqlx::query!(
        r#"DELETE FROM project_members WHERE project_id = ? AND user_id = ?"#,
        project_id,
        user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "member {user_id} in project {project_id}"
        )));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ══════════════════════════════════════════════════════════════════════════════
// Admin routes — global admin only, no project-membership requirement
// ══════════════════════════════════════════════════════════════════════════════

fn require_admin(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("global admin required".into()));
    }
    Ok(())
}

// ─── Admin: list all projects ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AdminListQuery {
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub owner_id: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn admin_list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<AdminListQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(200);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.display_name, p.owner_id,
                  u.username AS owner_username,
                  p.is_active, p.is_default,
                  p.quota_cpu_mcores, p.quota_mem_mb, p.quota_storage_gb,
                  p.quota_apps, p.quota_db_instances,
                  p.quota_bandwidth_gb, p.quota_domain_count, p.quota_request_million,
                  (SELECT COUNT(*) FROM project_members WHERE project_id = p.id) AS member_count,
                  (SELECT COUNT(*) FROM apps WHERE project_id = p.id) AS app_count,
                  p.created_at
           FROM projects p
           JOIN users u ON u.id = p.owner_id
           WHERE (? IS NULL OR p.name LIKE ? OR p.display_name LIKE ?)
             AND (? IS NULL OR p.is_active = ?)
             AND (? IS NULL OR p.owner_id = ?)
           ORDER BY p.display_name
           LIMIT ? OFFSET ?"#,
        q.search.clone(),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.is_active,
        q.is_active.map(|v| v as i8),
        q.owner_id.clone(),
        q.owner_id.clone(),
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM projects p
           WHERE (? IS NULL OR p.name LIKE ? OR p.display_name LIKE ?)
             AND (? IS NULL OR p.is_active = ?)
             AND (? IS NULL OR p.owner_id = ?)"#,
        q.search.clone(),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.is_active,
        q.is_active.map(|v| v as i8),
        q.owner_id.clone(),
        q.owner_id.clone(),
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.id,
            "name": r.name,
            "display_name": r.display_name,
            "owner_id": r.owner_id,
            "owner_username": r.owner_username,
            "is_active": r.is_active.unwrap_or(0) != 0,
            "is_default": r.is_default.unwrap_or(0) != 0,
            "member_count": r.member_count,
            "app_count": r.app_count,
            "quota": {
                "cpu_mcores":     r.quota_cpu_mcores,
                "mem_mb":         r.quota_mem_mb,
                "storage_gb":     r.quota_storage_gb,
                "apps":           r.quota_apps,
                "db_instances":   r.quota_db_instances,
                "bandwidth_gb":   r.quota_bandwidth_gb,
                "domain_count":   r.quota_domain_count,
                "request_million": r.quota_request_million,
            },
            "created_at": r.created_at,
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

// ─── Admin: get project ───────────────────────────────────────────────────────

pub async fn admin_get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let r = sqlx::query!(
        r#"SELECT p.id, p.name, p.display_name, p.owner_id,
                  u.username AS owner_username, u.email AS owner_email,
                  p.is_active, p.is_default,
                  p.quota_cpu_mcores, p.quota_mem_mb, p.quota_storage_gb,
                  p.quota_apps, p.quota_db_instances,
                  p.quota_bandwidth_gb, p.quota_domain_count, p.quota_request_million,
                  p.created_at, p.updated_at
           FROM projects p
           JOIN users u ON u.id = p.owner_id
           WHERE p.id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {id}")))?;

    let members = sqlx::query!(
        r#"SELECT pm.user_id, pm.role,
                  u.username, u.display_name, u.email, u.is_active,
                  pm.added_at,
                  ab.username AS added_by_username
           FROM project_members pm
           JOIN users u ON u.id = pm.user_id
           LEFT JOIN users ab ON ab.id = pm.added_by
           WHERE pm.project_id = ?
           ORDER BY FIELD(pm.role,'ADMIN','OPERATOR','OBSERVER'), u.username"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    let app_count: i64 =
        sqlx::query_scalar!(r#"SELECT COUNT(*) FROM apps WHERE project_id = ?"#, id)
            .fetch_one(&state.db)
            .await?;

    let db_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM database_instances WHERE project_id = ?"#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "owner_id": r.owner_id,
        "owner_username": r.owner_username,
        "owner_email": r.owner_email,
        "is_active": r.is_active != 0,
        "is_default": r.is_default != 0,
        "quota": {
            "cpu_mcores":     r.quota_cpu_mcores,
            "mem_mb":         r.quota_mem_mb,
            "storage_gb":     r.quota_storage_gb,
            "apps":           r.quota_apps,
            "db_instances":   r.quota_db_instances,
            "bandwidth_gb":   r.quota_bandwidth_gb,
            "domain_count":   r.quota_domain_count,
            "request_million": r.quota_request_million,
        },
        "stats": {
            "app_count": app_count,
            "db_count": db_count,
            "member_count": members.len(),
        },
        "members": members.iter().map(|m| serde_json::json!({
            "user_id": m.user_id,
            "username": m.username,
            "display_name": m.display_name,
            "email": m.email,
            "is_active": m.is_active != 0,
            "role": m.role,
            "added_at": m.added_at,
            "added_by": m.added_by_username,
        })).collect::<Vec<_>>(),
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    })))
}

// ─── Admin: create project for any owner ─────────────────────────────────────

#[derive(Deserialize)]
pub struct AdminCreateProjectRequest {
    pub name: String,
    pub display_name: String,
    /// Defaults to the requesting admin when omitted.
    pub owner_id: Option<String>,
    pub quota_cpu_mcores: Option<u32>,
    pub quota_mem_mb: Option<u32>,
    pub quota_storage_gb: Option<u32>,
    pub quota_apps: Option<u32>,
    pub quota_db_instances: Option<u32>,
    pub quota_bandwidth_gb: Option<u32>,
    pub quota_domain_count: Option<u32>,
    pub quota_request_million: Option<u32>,
}

pub async fn admin_create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<AdminCreateProjectRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if !slug_valid(&body.name) {
        return Err(AppError::BadRequest(
            "project name must be lowercase letters/digits/hyphens, start with a letter, max 63 chars".into(),
        ));
    }

    let owner_id = body.owner_id.as_deref().unwrap_or(&auth.user_id);

    // Validate owner exists
    let owner_exists = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM users WHERE id = ?"#, owner_id)
        .fetch_one(&state.db)
        .await?;
    if owner_exists == 0 {
        return Err(AppError::NotFound(format!("owner user {owner_id}")));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO projects
           (id, name, display_name, owner_id,
            quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
            quota_apps, quota_db_instances,
            quota_bandwidth_gb, quota_domain_count, quota_request_million)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        id,
        body.name,
        body.display_name.trim(),
        owner_id,
        body.quota_cpu_mcores.unwrap_or(0),
        body.quota_mem_mb.unwrap_or(0),
        body.quota_storage_gb.unwrap_or(0),
        body.quota_apps.unwrap_or(0),
        body.quota_db_instances.unwrap_or(0),
        body.quota_bandwidth_gb.unwrap_or(0),
        body.quota_domain_count.unwrap_or(0),
        body.quota_request_million.unwrap_or(0),
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("project name '{}' already taken", body.name))
        }
        other => AppError::Database(other),
    })?;

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role, added_by)
           VALUES (?, ?, 'ADMIN', ?)"#,
        id,
        owner_id,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    // Namespace is created lazily on first app deploy (cluster is not known at project creation time)

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "name": body.name })),
    ))
}

// ─── Admin: full update (including quotas, is_active, owner) ──────────────────

#[derive(Deserialize)]
pub struct AdminUpdateProjectRequest {
    pub display_name: Option<String>,
    pub is_active: Option<bool>,
    pub owner_id: Option<String>,
    pub quota_cpu_mcores: Option<u32>,
    pub quota_mem_mb: Option<u32>,
    pub quota_storage_gb: Option<u32>,
    pub quota_apps: Option<u32>,
    pub quota_db_instances: Option<u32>,
    pub quota_bandwidth_gb: Option<u32>,
    pub quota_domain_count: Option<u32>,
    pub quota_request_million: Option<u32>,
}

pub async fn admin_update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<AdminUpdateProjectRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    // Validate new owner if provided
    if let Some(ref new_owner) = body.owner_id {
        let exists = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM users WHERE id = ?"#, new_owner)
            .fetch_one(&state.db)
            .await?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("owner user {new_owner}")));
        }
        // Ensure new owner is a member with ADMIN role
        sqlx::query!(
            r#"INSERT INTO project_members (project_id, user_id, role, added_by)
               VALUES (?, ?, 'ADMIN', ?)
               ON DUPLICATE KEY UPDATE role = 'ADMIN'"#,
            id,
            new_owner,
            auth.user_id,
        )
        .execute(&state.db)
        .await?;
    }

    sqlx::query!(
        r#"UPDATE projects
           SET display_name          = COALESCE(?, display_name),
               is_active             = COALESCE(?, is_active),
               owner_id              = COALESCE(?, owner_id),
               quota_cpu_mcores      = COALESCE(?, quota_cpu_mcores),
               quota_mem_mb          = COALESCE(?, quota_mem_mb),
               quota_storage_gb      = COALESCE(?, quota_storage_gb),
               quota_apps            = COALESCE(?, quota_apps),
               quota_db_instances    = COALESCE(?, quota_db_instances),
               quota_bandwidth_gb    = COALESCE(?, quota_bandwidth_gb),
               quota_domain_count    = COALESCE(?, quota_domain_count),
               quota_request_million = COALESCE(?, quota_request_million)
           WHERE id = ?"#,
        body.display_name.as_deref().map(str::trim),
        body.is_active.map(|v| v as i8),
        body.owner_id,
        body.quota_cpu_mcores,
        body.quota_mem_mb,
        body.quota_storage_gb,
        body.quota_apps,
        body.quota_db_instances,
        body.quota_bandwidth_gb,
        body.quota_domain_count,
        body.quota_request_million,
        id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: force delete project ──────────────────────────────────────────────

pub async fn admin_delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let ns = sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {id}")))?;

    // Best-effort: remove namespace from every cluster hosting apps in this project
    let cluster_ids: Vec<String> = sqlx::query_scalar!(
        r#"SELECT DISTINCT c.id FROM clusters c
           JOIN apps a ON a.pool_id IN (SELECT id FROM resource_pools WHERE id = c.pool_id)
           WHERE a.project_id = ?"#,
        id
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for cid in cluster_ids {
        let _ = crate::k8s::namespace::delete_namespace(&state, &cid, &ns).await;
    }

    sqlx::query!(r#"DELETE FROM projects WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: add member (no restriction on who) ────────────────────────────────

pub async fn admin_add_member(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;
    validate_role(&body.role)?;

    let target_id = if let Some(ref uid) = body.user_id {
        uid.clone()
    } else if let Some(ref uname) = body.username {
        sqlx::query_scalar!(r#"SELECT id FROM users WHERE username = ?"#, uname)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("user '{uname}'")))?
    } else {
        return Err(AppError::BadRequest(
            "provide either user_id or username".into(),
        ));
    };

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role, added_by)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE role = VALUES(role), added_by = VALUES(added_by)"#,
        project_id,
        target_id,
        body.role,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: update member role (no last-ADMIN guard) ─────────────────────────

pub async fn admin_update_member(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, user_id)): Path<(String, String)>,
    Json(body): Json<UpdateMemberRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;
    validate_role(&body.role)?;

    let result = sqlx::query!(
        r#"UPDATE project_members SET role = ? WHERE project_id = ? AND user_id = ?"#,
        body.role,
        project_id,
        user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "member {user_id} in project {project_id}"
        )));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: remove member (no last-ADMIN or owner guard) ─────────────────────

pub async fn admin_remove_member(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, user_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let result = sqlx::query!(
        r#"DELETE FROM project_members WHERE project_id = ? AND user_id = ?"#,
        project_id,
        user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "member {user_id} in project {project_id}"
        )));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
