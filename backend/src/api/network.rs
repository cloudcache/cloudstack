//! User-facing network / IP-allocation APIs.
//!
//! Admin owns ip_pools and ip_allocations (raw IPAM).
//! Users own the view layer: which pools are wired to their project's cluster,
//! and which fixed IPs their apps hold.
//!
//! Allocation lifecycle (automatic):
//!   deploy → get_or_allocate_app_ips() assigns VPC + pub IPs
//!   delete → release_app_ips() frees them
//!
//! This module adds the visibility + manual-release surface:
//!   GET  /projects/:pid/network/pools          — pools available to this project
//!   GET  /projects/:pid/network/allocations    — all apps' IPs in this project
//!   GET  /projects/:pid/apps/:aid/network      — IPs for one app
//!   DELETE /projects/:pid/apps/:aid/network    — release IPs (freed on next deploy)
//!   POST /projects/:pid/apps/:aid/network/reassign — release + immediately reallocate

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

// ── GET /projects/:pid/network/pools ─────────────────────────────────────────
//
// Returns the VPC pool and pub pool wired to the cluster(s) serving this
// project.  Read-only; pool provisioning is admin-only.

pub async fn list_project_pools(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    // Find the resource pool used by apps in this project
    let pool_id = sqlx::query_scalar!(
        r#"SELECT DISTINCT pool_id FROM apps WHERE project_id = ? AND pool_id IS NOT NULL LIMIT 1"#,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .flatten();

    if pool_id.is_none() {
        return Ok(Json(serde_json::json!({ "vpc_pool": null, "pub_pool": null })));
    }
    let pool_id = pool_id.unwrap();

    // Find the active cluster for that resource pool
    let cluster = sqlx::query!(
        r#"SELECT vpc_pool_id, pub_pool_id FROM clusters
           WHERE pool_id = ? AND is_active = 1
           ORDER BY created_at LIMIT 1"#,
        pool_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(cluster) = cluster else {
        return Ok(Json(serde_json::json!({ "vpc_pool": null, "pub_pool": null })));
    };

    let fetch_pool = |id: Option<String>| {
        let db = state.db.clone();
        async move {
            let Some(id) = id else { return Ok::<_, AppError>(None) };
            let row = sqlx::query!(
                r#"SELECT id, name, cidr, gateway, pool_type, description
                   FROM ip_pools WHERE id = ? AND is_active = 1"#,
                id
            )
            .fetch_optional(&db)
            .await?;
            Ok(row.map(|r| serde_json::json!({
                "id":          r.id,
                "name":        r.name,
                "cidr":        r.cidr,
                "gateway":     r.gateway,
                "pool_type":   r.pool_type,
                "description": r.description,
            })))
        }
    };

    let vpc_pool = fetch_pool(cluster.vpc_pool_id).await?;
    let pub_pool = fetch_pool(cluster.pub_pool_id).await?;

    Ok(Json(serde_json::json!({
        "vpc_pool": vpc_pool,
        "pub_pool": pub_pool,
    })))
}

// ── GET /projects/:pid/network/allocations ───────────────────────────────────
//
// All fixed-IP allocations for apps in this project, grouped by app.

pub async fn list_project_allocations(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT
             a.id AS app_id,
             a.name AS app_name,
             a.status AS app_status,
             aia.ip_address,
             aia.pool_id,
             p.name AS pool_name,
             p.pool_type,
             p.gateway,
             aia.created_at
           FROM app_ip_allocations aia
           JOIN apps    a ON a.id    = aia.app_id
           JOIN ip_pools p ON p.id   = aia.pool_id
           WHERE a.project_id = ?
           ORDER BY a.name, p.pool_type"#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;

    // Group by app
    let mut by_app: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();

    for r in rows {
        let entry = by_app.entry(r.app_id.clone()).or_insert_with(|| serde_json::json!({
            "app_id":     r.app_id,
            "app_name":   r.app_name,
            "app_status": r.app_status,
            "ips":        [],
        }));

        entry["ips"].as_array_mut().unwrap().push(serde_json::json!({
            "ip_address": r.ip_address,
            "pool_id":    r.pool_id,
            "pool_name":  r.pool_name,
            "pool_type":  r.pool_type,
            "gateway":    r.gateway,
            "allocated_at": r.created_at,
        }));
    }

    Ok(Json(by_app.into_values().collect::<Vec<_>>()))
}

// ── GET /projects/:pid/apps/:aid/network ─────────────────────────────────────
//
// Fixed IPs for one app.

pub async fn get_app_network(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    // Verify app belongs to project
    let exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM apps WHERE id = ? AND project_id = ?"#,
        app_id, project_id
    )
    .fetch_one(&state.db)
    .await?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("app {app_id}")));
    }

    let rows = sqlx::query!(
        r#"SELECT
             aia.ip_address,
             aia.pool_id,
             p.name        AS pool_name,
             p.pool_type,
             p.cidr,
             p.gateway,
             aia.created_at
           FROM app_ip_allocations aia
           JOIN ip_pools p ON p.id = aia.pool_id
           WHERE aia.app_id = ?
           ORDER BY p.pool_type"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    let ips: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::json!({
        "ip_address":    r.ip_address,
        "pool_id":       r.pool_id,
        "pool_name":     r.pool_name,
        "pool_type":     r.pool_type,
        "cidr":          r.cidr,
        "gateway":       r.gateway,
        "allocated_at":  r.created_at,
    })).collect();

    Ok(Json(serde_json::json!({
        "app_id": app_id,
        "ips":    ips,
    })))
}

// ── DELETE /projects/:pid/apps/:aid/network ───────────────────────────────────
//
// Release the fixed IPs for an app.  The app must be STOPPED or SUSPENDED
// (can't yank IPs from a live deployment).  On next deploy, new IPs are
// auto-allocated from the same pools.

pub async fn release_app_network(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let app = sqlx::query!(
        r#"SELECT status FROM apps WHERE id = ? AND project_id = ?"#,
        app_id, project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    if !matches!(app.status.as_str(), "STOPPED" | "FAILED") {
        return Err(AppError::BadRequest(
            "app must be STOPPED or FAILED before releasing its IPs".into()
        ));
    }

    crate::k8s::network::release_app_ips(&state, &app_id).await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── POST /projects/:pid/apps/:aid/network/reassign ────────────────────────────
//
// Release current IPs and immediately allocate new ones from the same pools.
// Requires the app to be STOPPED — can't hot-swap IPs on a live pod.

pub async fn reassign_app_network(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let app = sqlx::query!(
        r#"SELECT status FROM apps WHERE id = ? AND project_id = ?"#,
        app_id, project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    if !matches!(app.status.as_str(), "STOPPED" | "FAILED") {
        return Err(AppError::BadRequest(
            "app must be STOPPED or FAILED to reassign IPs".into()
        ));
    }

    // Release existing
    crate::k8s::network::release_app_ips(&state, &app_id).await?;

    // Find the cluster for this app's pool
    let cluster_id = sqlx::query_scalar!(
        r#"SELECT c.id
           FROM clusters c
           JOIN apps a ON a.pool_id = c.pool_id
           WHERE a.id = ? AND c.is_active = 1
           ORDER BY c.created_at LIMIT 1"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("no active cluster for app".into()))?;

    // Allocate new IPs
    let ips = crate::k8s::network::get_or_allocate_app_ips(&state, &app_id, &cluster_id).await?;

    let result: Vec<serde_json::Value> = [
        ips.vpc.map(|a| serde_json::json!({
            "pool_type": "vpc",
            "ip_address": a.ip_with_prefix,
            "gateway": a.gateway,
        })),
        ips.pub_zone.map(|a| serde_json::json!({
            "pool_type": "pub",
            "ip_address": a.ip_with_prefix,
            "gateway": a.gateway,
        })),
    ]
    .into_iter()
    .flatten()
    .collect();

    Ok(Json(serde_json::json!({
        "app_id": app_id,
        "ips":    result,
    })))
}
