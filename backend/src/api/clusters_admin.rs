use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

/// GET /admin/clusters
pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT c.id, c.name, c.display_name, c.description, c.is_active, c.created_at,
                  p.id AS pool_id, p.name AS pool_name, p.display_name AS pool_display_name,
                  COUNT(n.id) AS node_count,
                  SUM(CASE WHEN n.node_status = 'READY' THEN 1 ELSE 0 END) AS ready_count
           FROM clusters c
           JOIN resource_pools p ON p.id = c.pool_id
           LEFT JOIN cluster_nodes n ON n.cluster_id = c.id
           GROUP BY c.id ORDER BY p.name, c.name"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "description": r.description,
        "is_active": r.is_active != 0,
        "pool": { "id": r.pool_id, "name": r.pool_name, "display_name": r.pool_display_name },
        "node_count": r.node_count,
        "ready_count": r.ready_count,
        "created_at": r.created_at,
    })).collect::<Vec<_>>())))
}

/// GET /admin/clusters/:id
pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let c = sqlx::query!(
        r#"SELECT c.id, c.name, c.display_name, c.description, c.is_active, c.created_at,
                  c.vpc_pool_id, c.pub_pool_id, c.node_main_iface,
                  p.id AS pool_id, p.name AS pool_name
           FROM clusters c
           JOIN resource_pools p ON p.id = c.pool_id
           WHERE c.id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("cluster {id}")))?;

    let nodes = sqlx::query!(
        r#"SELECT id, hostname, ip_address, node_role, node_status,
                  cpu_capacity_mcores, mem_capacity_mb, storage_path, pod_cidr,
                  has_gpu, last_seen_at
           FROM cluster_nodes WHERE cluster_id = ? ORDER BY node_role DESC, hostname"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": c.id,
        "name": c.name,
        "display_name": c.display_name,
        "description": c.description,
        "is_active": c.is_active != 0,
        "pool": { "id": c.pool_id, "name": c.pool_name },
        "vpc_pool_id": c.vpc_pool_id,
        "pub_pool_id": c.pub_pool_id,
        "node_main_iface": c.node_main_iface,
        "created_at": c.created_at,
        "nodes": nodes.iter().map(|n| serde_json::json!({
            "id": n.id,
            "hostname": n.hostname,
            "ip_address": n.ip_address,
            "node_role": n.node_role,
            "node_status": n.node_status,
            "cpu_capacity_mcores": n.cpu_capacity_mcores,
            "mem_capacity_mb": n.mem_capacity_mb,
            "storage_path": n.storage_path,
            "pod_cidr": n.pod_cidr,
            "has_gpu": n.has_gpu != 0,
            "last_seen_at": n.last_seen_at,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct CreateClusterRequest {
    pub pool_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    /// Pre-shared K3s join token. Auto-generated (random 32-char hex) if omitted.
    pub k3s_token: Option<String>,
    /// IP pool ID for the VPC (internal) network zone.
    pub vpc_pool_id: Option<String>,
    /// IP pool ID for the public network zone.
    pub pub_pool_id: Option<String>,
    /// Physical NIC name used for macvlan (default: "eth0").
    pub node_main_iface: Option<String>,
}

/// POST /admin/clusters
pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateClusterRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    // Verify pool exists
    let pool_exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM resource_pools WHERE id = ?"#, body.pool_id
    )
    .fetch_one(&state.db)
    .await?;
    if pool_exists == 0 {
        return Err(AppError::NotFound(format!("resource pool {}", body.pool_id)));
    }

    let token = body.k3s_token.unwrap_or_else(generate_k3s_token);
    let enc_token = state.crypto.encrypt(&token)?;
    let main_iface = body.node_main_iface.as_deref().unwrap_or("eth0");

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO clusters
             (id, pool_id, name, display_name, description, k3s_token,
              vpc_pool_id, pub_pool_id, node_main_iface)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        id, body.pool_id, body.name,
        body.display_name.as_deref().unwrap_or(&body.name),
        body.description, enc_token,
        body.vpc_pool_id, body.pub_pool_id, main_iface,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() =>
            AppError::Conflict(format!("cluster '{}' already exists", body.name)),
        other => AppError::Database(other),
    })?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdateClusterRequest {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub vpc_pool_id: Option<String>,
    pub pub_pool_id: Option<String>,
    pub node_main_iface: Option<String>,
}

/// PUT /admin/clusters/:id
pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateClusterRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let existing = sqlx::query!(
        r#"SELECT display_name, description, is_active,
                  vpc_pool_id, pub_pool_id, node_main_iface
           FROM clusters WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("cluster {id}")))?;

    let existing_display_name = existing.display_name.unwrap_or_default();
    let display_name = body.display_name.as_deref().unwrap_or(&existing_display_name);
    let description = body.description.as_deref().or(existing.description.as_deref());
    let is_active = body.is_active.map(|v| v as i8).unwrap_or(existing.is_active);
    let vpc_pool_id = body.vpc_pool_id.as_deref().or(existing.vpc_pool_id.as_deref());
    let pub_pool_id = body.pub_pool_id.as_deref().or(existing.pub_pool_id.as_deref());
    let main_iface = body.node_main_iface.as_deref().unwrap_or(&existing.node_main_iface);

    sqlx::query!(
        r#"UPDATE clusters
           SET display_name=?, description=?, is_active=?,
               vpc_pool_id=?, pub_pool_id=?, node_main_iface=?
           WHERE id=?"#,
        display_name, description, is_active,
        vpc_pool_id, pub_pool_id, main_iface, id,
    )
    .execute(&state.db)
    .await?;

    // If network pools were updated and the cluster has a kubeconfig, recreate NADs
    let pools_changed = body.vpc_pool_id.is_some() || body.pub_pool_id.is_some()
        || body.node_main_iface.is_some();
    if pools_changed {
        let state_clone = state.clone();
        let cluster_id = id.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::k8s::network::ensure_cluster_nads(&state_clone, &cluster_id).await {
                tracing::warn!("NAD sync for cluster {cluster_id}: {e}");
            }
        });
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /admin/clusters/:id  (refused if nodes exist)
pub async fn delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let node_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM cluster_nodes WHERE cluster_id = ?"#, id
    )
    .fetch_one(&state.db)
    .await?;

    if node_count > 0 {
        return Err(AppError::Conflict(
            format!("cluster has {node_count} node(s); remove them first")
        ));
    }

    sqlx::query!(r#"DELETE FROM clusters WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn generate_k3s_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| format!("{:02x}", rng.gen::<u8>()))
        .collect()
}
