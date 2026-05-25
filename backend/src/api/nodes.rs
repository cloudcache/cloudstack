use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

/// GET /admin/nodes  — lists all nodes across all clusters
pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT n.id, n.hostname, n.ip_address, n.node_role, n.has_gpu, n.gpu_model,
                  n.gpu_count, n.node_status, n.cpu_capacity_mcores, n.mem_capacity_mb,
                  n.storage_available, n.storage_path, n.pod_cidr, n.ldap_auth_active,
                  n.last_seen_at, n.cluster_id,
                  c.name AS cluster_name, p.name AS pool_name
           FROM cluster_nodes n
           LEFT JOIN clusters c ON c.id = n.cluster_id
           LEFT JOIN resource_pools p ON p.id = c.pool_id
           ORDER BY p.name, c.name, n.hostname"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "hostname": r.hostname,
        "ip_address": r.ip_address,
        "node_role": r.node_role,
        "has_gpu": r.has_gpu != 0,
        "gpu_model": r.gpu_model,
        "gpu_count": r.gpu_count,
        "node_status": r.node_status,
        "cpu_capacity_mcores": r.cpu_capacity_mcores,
        "mem_capacity_mb": r.mem_capacity_mb,
        "storage_available": r.storage_available != 0,
        "storage_path": r.storage_path,
        "pod_cidr": r.pod_cidr,
        "ldap_auth_active": r.ldap_auth_active != 0,
        "last_seen_at": r.last_seen_at,
        "cluster_id": r.cluster_id,
        "cluster_name": r.cluster_name,
        "pool_name": r.pool_name,
    })).collect::<Vec<_>>())))
}

/// GET /admin/nodes/:id
pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let r = sqlx::query!(
        r#"SELECT n.id, n.hostname, n.ip_address, n.node_role, n.has_gpu, n.gpu_model,
                  n.gpu_count, n.k8s_labels, n.node_status, n.cpu_capacity_mcores,
                  n.mem_capacity_mb, n.storage_available, n.storage_path, n.pod_cidr,
                  n.ldap_auth_active, n.last_seen_at, n.created_at, n.cluster_id,
                  c.name AS cluster_name, c.pool_id,
                  p.name AS pool_name
           FROM cluster_nodes n
           LEFT JOIN clusters c ON c.id = n.cluster_id
           LEFT JOIN resource_pools p ON p.id = c.pool_id
           WHERE n.id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "hostname": r.hostname,
        "ip_address": r.ip_address,
        "node_role": r.node_role,
        "has_gpu": r.has_gpu != 0,
        "gpu_model": r.gpu_model,
        "gpu_count": r.gpu_count,
        "k8s_labels": r.k8s_labels,
        "node_status": r.node_status,
        "cpu_capacity_mcores": r.cpu_capacity_mcores,
        "mem_capacity_mb": r.mem_capacity_mb,
        "storage_available": r.storage_available != 0,
        "storage_path": r.storage_path,
        "pod_cidr": r.pod_cidr,
        "ldap_auth_active": r.ldap_auth_active != 0,
        "last_seen_at": r.last_seen_at,
        "created_at": r.created_at,
        "cluster": { "id": r.cluster_id, "name": r.cluster_name },
        "pool":    { "id": r.pool_id,    "name": r.pool_name    },
    })))
}

#[derive(Deserialize)]
pub struct AddNodeRequest {
    pub cluster_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub node_role: Option<String>,
    /// SSH password for first-time connection
    pub ssh_password: String,
    /// Local FS path for hostPath volumes (defaults to /storage)
    pub storage_path: Option<String>,
}

/// POST /admin/nodes
pub async fn add(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<AddNodeRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    // Verify cluster exists and is active
    let cluster_active = sqlx::query_scalar!(
        r#"SELECT is_active FROM clusters WHERE id = ?"#, body.cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("cluster {}", body.cluster_id)))?;

    if cluster_active == 0 {
        return Err(AppError::BadRequest("cluster is inactive".into()));
    }

    let id = Uuid::new_v4().to_string();
    let role = body.node_role.clone().unwrap_or_else(|| "WORKER".to_string());
    let storage_path = body.storage_path.clone().unwrap_or_else(|| "/storage".to_string());

    sqlx::query!(
        r#"INSERT INTO cluster_nodes (id, cluster_id, hostname, ip_address, node_role, storage_path)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        id, body.cluster_id, body.hostname, body.ip_address, role, storage_path,
    )
    .execute(&state.db)
    .await?;

    let state_clone = state.clone();
    let cluster_id = body.cluster_id.clone();
    let ip = body.ip_address.clone();
    let hostname = body.hostname.clone();
    let ssh_pass = body.ssh_password.clone();
    let node_id = id.clone();
    let sp = storage_path.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::k8s::node::provision_node(
            &state_clone, &node_id, &cluster_id, &ip, &hostname, &ssh_pass, &role, &sp,
        )
        .await
        {
            tracing::error!("node provision failed for {hostname}: {e}");
            let _ = sqlx::query!(
                r#"UPDATE cluster_nodes SET node_status = 'NOT_READY' WHERE id = ?"#,
                node_id
            )
            .execute(&state_clone.db)
            .await;
        }
    });

    Ok((axum::http::StatusCode::ACCEPTED, Json(serde_json::json!({
        "id": id,
        "status": "provisioning",
        "cluster_id": body.cluster_id,
        "storage_path": storage_path,
    }))))
}

/// DELETE /admin/nodes/:id
pub async fn delete_node(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let node = sqlx::query!(
        r#"SELECT hostname, cluster_id FROM cluster_nodes WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    let cluster_id = node.cluster_id
        .ok_or_else(|| AppError::Internal("node has no cluster_id".into()))?;

    crate::k8s::node::drain_and_delete(&state, &cluster_id, &node.hostname).await?;

    sqlx::query!(r#"DELETE FROM cluster_nodes WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// PUT /admin/nodes/:id/labels
pub async fn update_labels(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(labels): Json<serde_json::Value>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let node = sqlx::query!(
        r#"SELECT hostname, cluster_id FROM cluster_nodes WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    let cluster_id = node.cluster_id
        .ok_or_else(|| AppError::Internal("node has no cluster_id".into()))?;

    crate::k8s::node::apply_labels(&state, &cluster_id, &node.hostname, &labels).await?;

    sqlx::query!(
        r#"UPDATE cluster_nodes SET k8s_labels = ? WHERE id = ?"#,
        serde_json::to_string(&labels).unwrap(), id
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /admin/nodes/:id/metrics — live metrics from node_exporter (port 9100)
/// Also refreshes the cached values in cluster_nodes.
pub async fn metrics(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let node = sqlx::query!(
        r#"SELECT hostname, ip_address, has_gpu, gpu_model, gpu_count,
                  cpu_capacity_mcores, mem_capacity_mb
           FROM cluster_nodes WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    let m = crate::k8s::node::refresh_node_metrics(&state, &id, &node.ip_address).await?;

    Ok(Json(serde_json::json!({
        "id": id,
        "hostname": node.hostname,
        "cpu": {
            "used_pct": m.cpu_used_pct,
            "capacity_mcores": node.cpu_capacity_mcores,
        },
        "memory": {
            "used_mb": m.mem_used_mb,
            "total_mb": m.mem_total_mb,
            "capacity_mb": node.mem_capacity_mb,
        },
        "disk": {
            "used_gb": m.disk_used_gb,
            "total_gb": m.disk_total_gb,
        },
        "load": {
            "load1": m.load1,
            "load5": m.load5,
            "load15": m.load15,
        },
        "gpu": {
            "enabled": node.has_gpu != 0,
            "model": node.gpu_model,
            "count": node.gpu_count,
        },
    })))
}

/// GET /admin/nodes/:id/health — live K8s status sync
pub async fn health(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let node = sqlx::query!(
        r#"SELECT hostname, cluster_id FROM cluster_nodes WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    let cluster_id = node.cluster_id
        .ok_or_else(|| AppError::Internal("node has no cluster_id".into()))?;

    let status =
        crate::k8s::node::sync_node_status(&state, &id, &cluster_id, &node.hostname).await?;

    Ok(Json(serde_json::json!({ "id": id, "node_status": status })))
}

/// POST /admin/nodes/:id/cordon — mark node unschedulable
pub async fn cordon(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    set_node_schedulable(&state, &id, false).await?;
    Ok(Json(serde_json::json!({ "id": id, "schedulable": false })))
}

/// POST /admin/nodes/:id/uncordon — mark node schedulable
pub async fn uncordon(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    set_node_schedulable(&state, &id, true).await?;
    Ok(Json(serde_json::json!({ "id": id, "schedulable": true })))
}

async fn set_node_schedulable(state: &AppState, node_id: &str, schedulable: bool) -> AppResult<()> {
    let node = sqlx::query!(
        r#"SELECT hostname, cluster_id FROM cluster_nodes WHERE id = ?"#, node_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {node_id}")))?;

    let cluster_id = node.cluster_id
        .ok_or_else(|| AppError::Internal("node has no cluster_id".into()))?;

    crate::k8s::node::set_schedulable(state, &cluster_id, &node.hostname, schedulable).await?;

    // Reflect in DB (column may not exist yet — swallow schema errors)
    let _ = sqlx::query(
        "UPDATE cluster_nodes SET schedulable = ? WHERE id = ?"
    )
    .bind(schedulable as i8)
    .bind(node_id)
    .execute(&state.db)
    .await;

    Ok(())
}

/// GET /api/v1/admin/nodes/metrics/aggregate
/// Returns cluster-wide CPU / RAM / disk totals from the metrics store.
/// Falls back to DB-stored capacity if no live metrics are available.
pub async fn metrics_aggregate(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    use crate::metrics::names::{
        NODE_CPU_USED_PCT, NODE_MEM_USED_BYTES, NODE_MEM_TOTAL_BYTES,
        NODE_FS_USED_BYTES, NODE_FS_TOTAL_BYTES,
    };
    use crate::metrics::types::MetricSelector;

    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let nodes = sqlx::query!(
        r#"SELECT id, hostname, cpu_capacity_mcores, mem_capacity_mb FROM cluster_nodes"#
    )
    .fetch_all(&state.db)
    .await?;

    let mut total_cpu_capacity_mcores: i64 = 0;
    let mut total_mem_capacity_mb: i64 = 0;
    let mut total_cpu_used_pct_sum: f64 = 0.0;
    let mut total_mem_used_bytes: f64 = 0.0;
    let mut total_mem_total_bytes: f64 = 0.0;
    let mut total_disk_used_bytes: f64 = 0.0;
    let mut total_disk_total_bytes: f64 = 0.0;
    let node_count = nodes.len();

    for node in &nodes {
        total_cpu_capacity_mcores += node.cpu_capacity_mcores.unwrap_or(0) as i64;
        total_mem_capacity_mb += node.mem_capacity_mb.unwrap_or(0) as i64;

        let cpu_pts = state.metrics.query_latest(
            MetricSelector::new(NODE_CPU_USED_PCT).label("node_id", node.id.clone())
        ).await.unwrap_or_default();
        let mem_used_pts = state.metrics.query_latest(
            MetricSelector::new(NODE_MEM_USED_BYTES).label("node_id", node.id.clone())
        ).await.unwrap_or_default();
        let mem_total_pts = state.metrics.query_latest(
            MetricSelector::new(NODE_MEM_TOTAL_BYTES).label("node_id", node.id.clone())
        ).await.unwrap_or_default();
        let disk_used_pts = state.metrics.query_latest(
            MetricSelector::new(NODE_FS_USED_BYTES).label("node_id", node.id.clone())
        ).await.unwrap_or_default();
        let disk_total_pts = state.metrics.query_latest(
            MetricSelector::new(NODE_FS_TOTAL_BYTES).label("node_id", node.id.clone())
        ).await.unwrap_or_default();

        total_cpu_used_pct_sum += cpu_pts.first().map(|p| p.value).unwrap_or(0.0);
        total_mem_used_bytes += mem_used_pts.first().map(|p| p.value).unwrap_or(0.0);
        total_mem_total_bytes += mem_total_pts.first().map(|p| p.value)
            .unwrap_or_else(|| node.mem_capacity_mb.unwrap_or(0) as f64 * 1024.0 * 1024.0);
        total_disk_used_bytes += disk_used_pts.first().map(|p| p.value).unwrap_or(0.0);
        total_disk_total_bytes += disk_total_pts.first().map(|p| p.value).unwrap_or(0.0);
    }

    let avg_cpu_used_pct = if node_count > 0 { total_cpu_used_pct_sum / node_count as f64 } else { 0.0 };

    Ok(Json(serde_json::json!({
        "node_count": node_count,
        "total_cpu_capacity_mcores": total_cpu_capacity_mcores,
        "avg_cpu_used_pct": avg_cpu_used_pct,
        "total_mem_capacity_mb": total_mem_capacity_mb,
        "total_mem_used_bytes": total_mem_used_bytes,
        "total_mem_total_bytes": total_mem_total_bytes,
        "total_disk_used_bytes": total_disk_used_bytes,
        "total_disk_total_bytes": total_disk_total_bytes,
    })))
}
