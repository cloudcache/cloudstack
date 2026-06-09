use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

/// Row type for the list query (uses query_as instead of query! to include provision_error).
#[derive(sqlx::FromRow)]
struct NodeRow {
    id: String,
    hostname: String,
    ip_address: String,
    node_role: String,
    has_gpu: i8,
    gpu_model: Option<String>,
    gpu_count: Option<u8>,
    node_status: String,
    provision_error: Option<String>,
    cpu_capacity_mcores: Option<u32>,
    mem_capacity_mb: Option<u32>,
    storage_available: i8,
    storage_path: Option<String>,
    ssh_port: Option<u16>,
    pod_cidr: Option<String>,
    ldap_auth_active: i8,
    last_seen_at: Option<chrono::NaiveDateTime>,
    cluster_id: Option<String>,
    cluster_name: Option<String>,
    cluster_display_name: Option<String>,
    cluster_orchestrator: Option<String>,
    pool_name: Option<String>,
    pool_display_name: Option<String>,
    ip_pool_id: Option<String>,
    ip_pool_name: Option<String>,
    ip_pool_cidr: Option<String>,
    ip_pool_gateway: Option<String>,
}

/// GET /admin/nodes  — lists all nodes across all clusters
pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query_as::<_, NodeRow>(
        "SELECT n.id, n.hostname, n.ip_address, n.node_role, n.has_gpu, n.gpu_model, \
                n.gpu_count, n.node_status, n.provision_error, \
                n.cpu_capacity_mcores, n.mem_capacity_mb, \
                n.storage_available, n.storage_path, n.ssh_port, n.pod_cidr, n.ldap_auth_active, \
                n.last_seen_at, n.cluster_id, \
                c.name AS cluster_name, c.display_name AS cluster_display_name, \
                c.orchestrator AS cluster_orchestrator, \
                p.name AS pool_name, p.display_name AS pool_display_name, \
                c.ip_pool_id AS ip_pool_id, \
                ip.name AS ip_pool_name, ip.cidr AS ip_pool_cidr, ip.gateway AS ip_pool_gateway \
         FROM cluster_nodes n \
         LEFT JOIN clusters c ON c.id = n.cluster_id \
         LEFT JOIN resource_pools p ON p.id = c.pool_id \
         LEFT JOIN ip_pools ip ON ip.id = c.ip_pool_id \
         ORDER BY p.name, c.name, n.hostname",
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
        "provision_error": r.provision_error,
        "cpu_capacity_mcores": r.cpu_capacity_mcores,
        "mem_capacity_mb": r.mem_capacity_mb,
        "storage_available": r.storage_available != 0,
        "storage_path": r.storage_path,
        "ssh_port": r.ssh_port,
        "pod_cidr": r.pod_cidr,
        "ldap_auth_active": r.ldap_auth_active != 0,
        "last_seen_at": r.last_seen_at,
        "cluster_id": r.cluster_id,
        "cluster_name": r.cluster_name,
        "cluster_display_name": r.cluster_display_name,
        "cluster_orchestrator": r.cluster_orchestrator,
        "pool_name": r.pool_name,
        "pool_display_name": r.pool_display_name,
        "ip_pool_id": r.ip_pool_id,
        "ip_pool_name": r.ip_pool_name,
        "ip_pool_cidr": r.ip_pool_cidr,
        "ip_pool_gateway": r.ip_pool_gateway,
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
    /// SSH password for first-time connection. Omit/empty when the backend can
    /// already SSH to root@node with the platform key (key-based provisioning).
    pub ssh_password: Option<String>,
    /// SSH port (defaults to 22).
    pub ssh_port: Option<u16>,
    /// Local FS path for hostPath volumes (defaults to /storage)
    pub storage_path: Option<String>,
    /// Agent HTTP port for Docker-mode nodes (defaults to 9800)
    pub agent_port: Option<u16>,
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

    // Determine orchestrator type early (needed for master check + provisioning)
    let orch: String = sqlx::query_scalar(
        "SELECT orchestrator FROM clusters WHERE id = ?",
    )
    .bind(&body.cluster_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "K3S".to_string());

    // K3s workers require at least one existing MASTER node; Docker nodes don't
    if orch != "DOCKER" && role == "WORKER" {
        let has_master = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS cnt FROM cluster_nodes WHERE cluster_id = ? AND node_role = 'MASTER'"#,
            body.cluster_id
        )
        .fetch_one(&state.db)
        .await?;
        if has_master == 0 {
            return Err(AppError::BadRequest(
                "Cannot add a WORKER node: no MASTER node exists in this cluster yet. Add a MASTER node first.".into()
            ));
        }
    }
    let storage_path = body.storage_path.clone().unwrap_or_else(|| "/storage".to_string());
    let ssh_port: u16 = body.ssh_port.unwrap_or(22);

    sqlx::query(
        "INSERT INTO cluster_nodes (id, cluster_id, hostname, ip_address, node_role, storage_path, ssh_port) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&body.cluster_id)
    .bind(&body.hostname)
    .bind(&body.ip_address)
    .bind(&role)
    .bind(&storage_path)
    .bind(ssh_port)
    .execute(&state.db)
    .await?;

    let agent_port: u16 = body.agent_port.unwrap_or(9800);

    let state_clone = state.clone();
    let cluster_id = body.cluster_id.clone();
    let ip = body.ip_address.clone();
    let hostname = body.hostname.clone();
    let ssh_pass = body.ssh_password.clone().unwrap_or_default();
    let node_id = id.clone();
    let sp = storage_path.clone();
    tokio::spawn(async move {
        let result = std::panic::AssertUnwindSafe(async {
            if orch == "DOCKER" {
                crate::k8s::node::provision_docker_node(
                    &state_clone, &node_id, &cluster_id, &ip, &hostname, &ssh_pass, ssh_port, &sp, agent_port,
                ).await
            } else {
                crate::k8s::node::provision_node(
                    &state_clone, &node_id, &cluster_id, &ip, &hostname, &ssh_pass, ssh_port, &role, &sp,
                ).await
            }
        });
        match futures::FutureExt::catch_unwind(result).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("node provision failed for {hostname}: {e}");
                // logger.finish_err already called inside provision_*; update status as fallback
                let _ = sqlx::query(
                    "UPDATE cluster_nodes SET node_status = 'NOT_READY', provision_error = COALESCE(provision_error, ?) WHERE id = ?"
                )
                .bind(format!("{e}"))
                .bind(&node_id)
                .execute(&state_clone.db)
                .await;
            }
            Err(_panic) => {
                tracing::error!("node provision panicked for {hostname}");
                let _ = sqlx::query(
                    "UPDATE cluster_nodes SET node_status = 'NOT_READY', provision_step = 'failed', \
                     provision_error = 'internal panic during provisioning' WHERE id = ?"
                )
                .bind(&node_id)
                .execute(&state_clone.db)
                .await;
            }
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

/// PUT /admin/nodes/:id — update node settings (hostname, ip, role, storage_path)
pub async fn update_node(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateNodeRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    // Verify node exists
    sqlx::query!( r#"SELECT id FROM cluster_nodes WHERE id = ?"#, id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    if let Some(ref h) = body.hostname {
        sqlx::query!("UPDATE cluster_nodes SET hostname = ? WHERE id = ?", h, id)
            .execute(&state.db).await?;
    }
    if let Some(ref ip) = body.ip_address {
        sqlx::query!("UPDATE cluster_nodes SET ip_address = ? WHERE id = ?", ip, id)
            .execute(&state.db).await?;
    }
    if let Some(ref role) = body.node_role {
        sqlx::query("UPDATE cluster_nodes SET node_role = ? WHERE id = ?")
            .bind(role).bind(&id)
            .execute(&state.db).await?;
    }
    if let Some(ref sp) = body.storage_path {
        sqlx::query!("UPDATE cluster_nodes SET storage_path = ? WHERE id = ?", sp, id)
            .execute(&state.db).await?;
    }
    if let Some(port) = body.ssh_port {
        sqlx::query("UPDATE cluster_nodes SET ssh_port = ? WHERE id = ?")
            .bind(port).bind(&id)
            .execute(&state.db).await?;
    }

    Ok(Json(serde_json::json!({ "id": id, "updated": true })))
}

#[derive(Deserialize)]
pub struct UpdateNodeRequest {
    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub node_role: Option<String>,
    pub storage_path: Option<String>,
    pub ssh_port: Option<u16>,
}

/// POST /admin/nodes/:id/reprovision — retry provisioning for a failed node
pub async fn reprovision(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<ReprovisionRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let node = sqlx::query!(
        r#"SELECT hostname, ip_address, node_role, cluster_id, storage_path, node_status
           FROM cluster_nodes WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("node {id}")))?;

    if node.node_status == "PROVISIONING" {
        return Err(AppError::BadRequest("Node is already being provisioned.".into()));
    }

    let cluster_id = node.cluster_id
        .ok_or_else(|| AppError::Internal("node has no cluster_id".into()))?;
    let role = node.node_role.clone();
    let storage_path = node.storage_path.clone().unwrap_or_else(|| "/storage".to_string());
    let ssh_port: u16 = sqlx::query_scalar::<_, Option<u16>>(
        "SELECT ssh_port FROM cluster_nodes WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .unwrap_or(22);

    // Clear previous error
    sqlx::query("UPDATE cluster_nodes SET node_status = 'PROVISIONING', provision_error = NULL WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    let orch: String = sqlx::query_scalar(
        "SELECT orchestrator FROM clusters WHERE id = ?",
    )
    .bind(&cluster_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "K3S".to_string());

    let state_clone = state.clone();
    let node_id = id.clone();
    let ip = node.ip_address.clone();
    let hostname = node.hostname.clone();
    let ssh_pass = body.ssh_password.clone().unwrap_or_default();
    tokio::spawn(async move {
        let result = std::panic::AssertUnwindSafe(async {
            if orch == "DOCKER" {
                crate::k8s::node::provision_docker_node(
                    &state_clone, &node_id, &cluster_id, &ip, &hostname, &ssh_pass, ssh_port, &storage_path, 9800,
                ).await
            } else {
                crate::k8s::node::provision_node(
                    &state_clone, &node_id, &cluster_id, &ip, &hostname, &ssh_pass, ssh_port, &role, &storage_path,
                ).await
            }
        });
        match futures::FutureExt::catch_unwind(result).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("node reprovision failed for {hostname}: {e}");
                let _ = sqlx::query(
                    "UPDATE cluster_nodes SET node_status = 'NOT_READY', provision_error = COALESCE(provision_error, ?) WHERE id = ?"
                )
                .bind(format!("{e}"))
                .bind(&node_id)
                .execute(&state_clone.db)
                .await;
            }
            Err(_panic) => {
                tracing::error!("node reprovision panicked for {hostname}");
                let _ = sqlx::query(
                    "UPDATE cluster_nodes SET node_status = 'NOT_READY', provision_step = 'failed', \
                     provision_error = 'internal panic during provisioning' WHERE id = ?"
                )
                .bind(&node_id)
                .execute(&state_clone.db)
                .await;
            }
        }
    });

    Ok((axum::http::StatusCode::ACCEPTED, Json(serde_json::json!({
        "id": id, "status": "provisioning"
    }))))
}

#[derive(Deserialize)]
pub struct ReprovisionRequest {
    /// Omit/empty to reprovision over the platform key (no password needed once
    /// the backend can already SSH to the node).
    pub ssh_password: Option<String>,
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

// ── Provision progress endpoints ─────────────────────────────────────────────

/// GET /admin/nodes/:id/provision-logs — list all provision log rows for a node
pub async fn provision_logs(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    #[derive(sqlx::FromRow, serde::Serialize)]
    struct LogRow {
        id: i64,
        attempt: u32,
        step_index: i16,
        step_name: String,
        status: String,
        output: Option<String>,
        started_at: chrono::NaiveDateTime,
        finished_at: Option<chrono::NaiveDateTime>,
    }

    let rows: Vec<LogRow> = sqlx::query_as(
        "SELECT id, attempt, step_index, step_name, status, output, started_at, finished_at \
         FROM node_provision_logs WHERE node_id = ? ORDER BY attempt DESC, step_index ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows)))
}

/// POST /admin/nodes/:id/provision-cancel — set the cancel flag
pub async fn provision_cancel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    sqlx::query("UPDATE cluster_nodes SET provision_cancel = 1 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /admin/nodes/:id/provision-stream — SSE stream of provision progress
///
/// Polls `node_provision_logs` for the current attempt and emits new/updated
/// steps as SSE events. Ends when provision_step = 'done' or 'failed'.
pub async fn provision_stream(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    use axum::response::sse::{Event, KeepAlive, Sse};
    use futures::stream::Stream;
    use std::convert::Infallible;

    let stream = async_stream::stream! {
        // Resolve current attempt
        let attempt: u32 = sqlx::query_scalar::<_, Option<u32>>(
            "SELECT provision_attempt FROM cluster_nodes WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(1);

        let mut last_seen_id: i64 = 0;
        // Hard timeout: stop streaming after 30 minutes regardless
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(1800);
        // Stall detection: if no new rows for 5 minutes, treat as stuck
        let mut last_activity = tokio::time::Instant::now();

        loop {
            // Fetch new/updated log rows
            #[derive(sqlx::FromRow, serde::Serialize)]
            struct StepRow {
                id: i64,
                step_index: i16,
                step_name: String,
                status: String,
                output: Option<String>,
                started_at: chrono::NaiveDateTime,
                finished_at: Option<chrono::NaiveDateTime>,
            }

            let rows: Vec<StepRow> = sqlx::query_as(
                "SELECT id, step_index, step_name, status, output, started_at, finished_at \
                 FROM node_provision_logs WHERE node_id = ? AND attempt = ? AND id > ? \
                 ORDER BY step_index ASC",
            )
            .bind(&id)
            .bind(attempt)
            .bind(last_seen_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            if !rows.is_empty() {
                last_activity = tokio::time::Instant::now();
            }

            for row in &rows {
                if row.id > last_seen_id {
                    last_seen_id = row.id;
                }
                let json = serde_json::to_string(row).unwrap_or_default();
                yield Ok::<_, Infallible>(Event::default().event("step").data(json));
            }

            // Check if provision is finished
            let step: String = sqlx::query_scalar::<_, Option<String>>(
                "SELECT provision_step FROM cluster_nodes WHERE id = ?",
            )
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .flatten()
            .unwrap_or_default();

            if step == "done" || step == "failed" {
                yield Ok::<_, Infallible>(
                    Event::default().event("finish").data(step)
                );
                break;
            }

            let now = tokio::time::Instant::now();

            // Hard timeout
            if now >= deadline {
                yield Ok::<_, Infallible>(
                    Event::default().event("error").data("stream timeout (30 min)")
                );
                break;
            }

            // Stall detection: no new log rows for 5 minutes
            if now.duration_since(last_activity).as_secs() > 300 {
                yield Ok::<_, Infallible>(
                    Event::default().event("error").data("provision appears stalled (no output for 5 min)")
                );
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
