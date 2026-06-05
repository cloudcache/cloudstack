use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{ldap::LdapService, middleware::AuthUser},
    error::{AppError, AppResult},
    state::AppState,
};

fn require_admin(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("global admin required".into()));
    }
    Ok(())
}

// ─── Shared response type ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub ldap_uid: Option<u32>,
    pub ldap_gid: Option<u32>,
    pub is_global_admin: bool,
    pub is_active: bool,
}

// ─── List ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListQuery {
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<ListQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(200);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT id, username, email, display_name, ldap_uid, ldap_gid,
                  is_global_admin, is_active
           FROM users
           WHERE (? IS NULL OR username LIKE ? OR email LIKE ? OR display_name LIKE ?)
             AND (? IS NULL OR is_active = ?)
           ORDER BY username
           LIMIT ? OFFSET ?"#,
        q.search.clone(),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.is_active,
        q.is_active.map(|v| v as i8),
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM users
           WHERE (? IS NULL OR username LIKE ? OR email LIKE ? OR display_name LIKE ?)
             AND (? IS NULL OR is_active = ?)"#,
        q.search.clone(),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.is_active,
        q.is_active.map(|v| v as i8),
    )
    .fetch_one(&state.db)
    .await?;

    let users: Vec<UserRow> = rows
        .into_iter()
        .map(|r| UserRow {
            id: r.id,
            username: r.username,
            email: r.email,
            display_name: r.display_name,
            ldap_uid: r.ldap_uid,
            ldap_gid: r.ldap_gid,
            is_global_admin: r.is_global_admin != 0,
            is_active: r.is_active != 0,
        })
        .collect();

    Ok(Json(serde_json::json!({
        "data": users,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

// ─── Get ──────────────────────────────────────────────────────────────────────

pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let r = sqlx::query!(
        r#"SELECT id, username, email, display_name, ldap_uid, ldap_gid,
                  is_global_admin, is_active, created_at, updated_at
           FROM users WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user {id}")))?;

    let balance = sqlx::query_scalar!(
        r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?;

    let subscription = sqlx::query!(
        r#"SELECT s.status, p.display_name AS plan_display_name, s.expires_at
           FROM user_subscriptions s
           JOIN subscription_plans p ON p.id = s.plan_id
           WHERE s.user_id = ? ORDER BY s.created_at DESC LIMIT 1"#,
        id
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "username": r.username,
        "email": r.email,
        "display_name": r.display_name,
        "ldap_uid": r.ldap_uid,
        "ldap_gid": r.ldap_gid,
        "is_global_admin": r.is_global_admin != 0,
        "is_active": r.is_active != 0,
        "wallet_balance": balance,
        "subscription": subscription.map(|s| serde_json::json!({
            "status": s.status,
            "plan": s.plan_display_name,
            "expires_at": s.expires_at,
        })),
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    })))
}

// ─── Create (admin pre-provision) ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
    pub is_global_admin: Option<bool>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if body.password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters".into()));
    }

    // Create in LLDAP first
    state
        .lldap
        .create_user(
            &body.username,
            &body.email,
            body.display_name.as_deref(),
            &body.password,
        )
        .await?;

    if let Some(group_id) = state.config.ldap.default_user_group_id {
        let _ = state.lldap.add_user_to_group(&body.username, group_id).await;
    }

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO users
           (id, username, email, display_name, is_global_admin)
           VALUES (?, ?, ?, ?, ?)"#,
        id,
        body.username,
        body.email,
        body.display_name,
        body.is_global_admin.unwrap_or(false) as i8,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict("username or email already exists".into())
        }
        other => AppError::Database(other),
    })?;

    sqlx::query!(
        r#"INSERT IGNORE INTO user_wallets (user_id) VALUES (?)"#,
        id
    )
    .execute(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

// ─── Update ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub is_global_admin: Option<bool>,
}

pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    // Prevent removing own admin flag
    if id == auth.user_id {
        if let Some(false) = body.is_global_admin {
            return Err(AppError::BadRequest("cannot remove your own admin flag".into()));
        }
    }

    // Fetch current identity for LLDAP sync / ownership checks
    let current = sqlx::query(r#"SELECT username, ldap_dn FROM users WHERE id = ?"#)
        .bind(&id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {id}")))?;
    let current_username: String = sqlx::Row::get(&current, "username");
    let current_ldap_dn: Option<String> = sqlx::Row::get(&current, "ldap_dn");

    if let Some(username) = body.username.as_deref() {
        if username.trim().is_empty() {
            return Err(AppError::BadRequest("username is required".into()));
        }
    }

    if let Some(email) = body.email.as_deref() {
        if !email.contains('@') {
            return Err(AppError::BadRequest("invalid email".into()));
        }
    }

    if body.username.is_some() || body.email.is_some() {
        let conflict: Option<String> = sqlx::query_scalar(
            r#"SELECT id FROM users
               WHERE id <> ? AND ((? IS NOT NULL AND username = ?) OR (? IS NOT NULL AND email = ?))
               LIMIT 1"#,
        )
        .bind(&id)
        .bind(&body.username)
        .bind(&body.username)
        .bind(&body.email)
        .bind(&body.email)
        .fetch_optional(&state.db)
        .await?;

        if conflict.is_some() {
            return Err(AppError::Conflict("username or email already exists".into()));
        }
    }

    let mut new_ldap_dn = None;
    if let Some(ref current_dn) = current_ldap_dn {
        let ldap_username = body.username.as_deref().unwrap_or(&current_username);
        if body.username.as_deref().is_some_and(|u| u != current_username) {
            let ldap = LdapService::new(state.config.ldap.clone());
            new_ldap_dn = Some(ldap.rename_user(current_dn, ldap_username).await?);
        }

        if body.username.is_some() || body.display_name.is_some() || body.email.is_some() {
            state
                .lldap
                .update_user(
                    ldap_username,
                    body.display_name.as_deref(),
                    body.email.as_deref(),
                )
                .await?;
        }
    }

    sqlx::query(
        r#"UPDATE users
           SET username        = COALESCE(?, username),
               display_name    = COALESCE(?, display_name),
               email           = COALESCE(?, email),
               ldap_dn         = COALESCE(?, ldap_dn),
               is_active       = COALESCE(?, is_active),
               is_global_admin = COALESCE(?, is_global_admin)
           WHERE id = ?"#,
    )
    .bind(&body.username)
    .bind(&body.display_name)
    .bind(&body.email)
    .bind(&new_ldap_dn)
    .bind(body.is_active.map(|v| v as i8))
    .bind(body.is_global_admin.map(|v| v as i8))
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict("username or email already exists".into())
        }
        other => AppError::Database(other),
    })?;

    // If account was deactivated, revoke all sessions immediately
    if body.is_active == Some(false) {
        let _ = sqlx::query!(
            r#"DELETE FROM user_sessions WHERE user_id = ?"#, id
        )
        .execute(&state.db)
        .await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Delete ───────────────────────────────────────────────────────────────────

pub async fn delete_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if id == auth.user_id {
        return Err(AppError::BadRequest("cannot delete yourself".into()));
    }

    let user = sqlx::query!(
        r#"SELECT username FROM users WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user {id}")))?;

    // Delete from LLDAP (best-effort — user may not exist there)
    let _ = state.lldap.delete_user(&user.username).await;

    sqlx::query!(r#"DELETE FROM users WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: reset password ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AdminResetPasswordRequest {
    pub new_password: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<AdminResetPasswordRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if body.new_password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters".into()));
    }

    let username = sqlx::query_scalar!(
        r#"SELECT username FROM users WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user {id}")))?;

    state.lldap.change_password(&username, &body.new_password).await?;

    // Revoke all sessions so user must re-login with new password
    sqlx::query!(
        r#"DELETE FROM user_sessions WHERE user_id = ?"#,
        id
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: get user usage summary ───────────────────────────────────────────

pub async fn get_usage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let app_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM apps a
           JOIN projects p ON p.id = a.project_id
           JOIN project_members pm ON pm.project_id = p.id
           WHERE pm.user_id = ? AND a.status != 'STOPPED'"#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    let db_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM database_instances WHERE created_by = ? AND status = 'ACTIVE'"#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    let last_snapshot = sqlx::query!(
        r#"SELECT cpu_mcores_used, mem_mb_used, storage_gb_used, cost, snapshot_time
           FROM usage_snapshots WHERE user_id = ? ORDER BY snapshot_time DESC LIMIT 1"#,
        id
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "active_apps": app_count,
        "active_databases": db_count,
        "last_snapshot": last_snapshot.map(|s| serde_json::json!({
            "time": s.snapshot_time,
            "cpu_mcores": s.cpu_mcores_used,
            "mem_mb": s.mem_mb_used,
            "storage_gb": s.storage_gb_used,
            "hourly_cost": s.cost,
        })),
    })))
}
