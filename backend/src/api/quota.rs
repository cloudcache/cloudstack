use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

// ── GET /projects/:pid/quota ──────────────────────────────────────────────────

pub async fn get_project_quota(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "VIEWER").await?;

    let status = crate::quota::check(&state, &project_id).await?;

    Ok(Json(serde_json::json!({
        "quota": {
            "cpu_mcores": status.quota.cpu_mcores,
            "mem_mb": status.quota.mem_mb,
            "app_count": status.quota.app_count,
        },
        "usage": {
            "cpu_mcores": status.usage.cpu_mcores,
            "mem_mb": status.usage.mem_mb,
            "app_count": status.usage.app_count,
        },
        "pct": {
            "cpu": status.cpu_pct,
            "mem": status.mem_pct,
            "app": status.app_pct,
        },
        "warned":   { "cpu": status.cpu_warned,   "mem": status.mem_warned,   "app": status.app_warned },
        "exceeded": { "cpu": status.cpu_exceeded, "mem": status.mem_exceeded, "app": status.app_exceeded },
    })))
}

// ── GET /projects/:pid/quota/violations ───────────────────────────────────────

pub async fn list_violations(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "VIEWER").await?;

    let rows = sqlx::query!(
        r#"SELECT id, app_id, dimension, used_value, quota_value, pct_used,
                  action, resolved_at, created_at
           FROM quota_violations
           WHERE project_id = ?
           ORDER BY created_at DESC
           LIMIT 200"#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;

    let items: Vec<serde_json::Value> = rows.into_iter().map(|r| serde_json::json!({
        "id":          r.id,
        "app_id":      r.app_id,
        "dimension":   r.dimension,
        "used":        r.used_value,
        "quota":       r.quota_value,
        "pct_used":    r.pct_used,
        "action":      r.action,
        "resolved_at": r.resolved_at,
        "created_at":  r.created_at,
    })).collect();

    Ok(Json(items))
}

// ── POST /admin/projects/:pid/quota/enforce ───────────────────────────────────

pub async fn admin_enforce(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    let suspended = crate::quota::enforce_project(&state, &project_id, "admin_forced").await?;

    Ok(Json(serde_json::json!({
        "suspended": suspended,
        "count": suspended.len(),
    })))
}

// ── POST /admin/apps/:app_id/suspend ─────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct SuspendRequest {
    pub reason: Option<String>,
}

pub async fn admin_suspend_app(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(app_id): Path<String>,
    body: Option<Json<SuspendRequest>>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    let row = sqlx::query!(
        r#"SELECT id, name, project_id, pool_id FROM apps WHERE id = ?"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    let reason = body
        .and_then(|b| b.reason.clone())
        .unwrap_or_else(|| "admin_suspend".to_string());

    let ns = sqlx::query_scalar!(
        r#"SELECT name FROM projects WHERE id = ?"#, row.project_id
    )
    .fetch_one(&state.db)
    .await?;

    let cluster_id = sqlx::query_scalar!(
        r#"SELECT id FROM clusters
           WHERE pool_id = ? AND is_active = 1
           ORDER BY created_at LIMIT 1"#,
        row.pool_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("no cluster for app".into()))?;

    crate::k8s::deployment::scale_deployment(&state, &cluster_id, &ns, &row.name, 0).await?;

    sqlx::query!(
        r#"UPDATE apps
           SET status = 'SUSPENDED', paused_at = NOW(), paused_by = 'admin', pause_reason = ?
           WHERE id = ?"#,
        reason,
        app_id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── POST /admin/apps/:app_id/unsuspend ────────────────────────────────────────

pub async fn admin_unsuspend_app(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(app_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    let row = sqlx::query!(
        r#"SELECT id, name, project_id, pool_id, replicas FROM apps WHERE id = ?"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    // Check quota before unsuspending
    let res = sqlx::query!(
        r#"SELECT COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) AS cpu,
                  COALESCE(mem_limit_mb, mem_reservation_mb, 0) AS mem
           FROM apps WHERE id = ?"#,
        app_id
    )
    .fetch_one(&state.db)
    .await?;
    let extra_cpu = res.cpu as i64 * row.replicas as i64;
    let extra_mem = res.mem as i64 * row.replicas as i64;
    crate::quota::check_deploy_allowed(&state, &row.project_id, extra_cpu, extra_mem, 0).await?;

    let ns = sqlx::query_scalar!(
        r#"SELECT name FROM projects WHERE id = ?"#, row.project_id
    )
    .fetch_one(&state.db)
    .await?;

    let cluster_id = sqlx::query_scalar!(
        r#"SELECT id FROM clusters
           WHERE pool_id = ? AND is_active = 1
           ORDER BY created_at LIMIT 1"#,
        row.pool_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("no cluster for app".into()))?;

    crate::k8s::deployment::scale_deployment(&state, &cluster_id, &ns, &row.name, row.replicas as i32).await?;

    sqlx::query!(
        r#"UPDATE apps
           SET status = 'RUNNING', paused_at = NULL, paused_by = NULL, pause_reason = NULL
           WHERE id = ?"#,
        app_id,
    )
    .execute(&state.db)
    .await?;

    crate::quota::resolve_violations(&state, &row.project_id).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
