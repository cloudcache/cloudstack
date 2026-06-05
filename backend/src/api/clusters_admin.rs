use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

/// GET /admin/clusters
pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    #[derive(sqlx::FromRow)]
    struct ListRow {
        id: String, name: String, display_name: Option<String>,
        description: Option<String>, is_active: i8,
        created_at: chrono::NaiveDateTime,
        orchestrator: String,
        ip_pool_id: Option<String>, node_main_iface: String,
        pool_id: String, pool_name: String, pool_display_name: Option<String>,
        node_count: i64,
        ready_count: Option<i64>,
    }
    let rows: Vec<ListRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.display_name, c.description, c.is_active, c.created_at, \
                c.orchestrator, c.ip_pool_id, c.node_main_iface, \
                p.id AS pool_id, p.name AS pool_name, p.display_name AS pool_display_name, \
                COUNT(n.id) AS node_count, \
                CAST(SUM(CASE WHEN n.node_status = 'READY' THEN 1 ELSE 0 END) AS SIGNED) AS ready_count \
         FROM clusters c \
         JOIN resource_pools p ON p.id = c.pool_id \
         LEFT JOIN cluster_nodes n ON n.cluster_id = c.id \
         GROUP BY c.id ORDER BY p.name, c.name",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "id": r.id,
            "name": r.name,
            "display_name": r.display_name,
            "description": r.description,
            "is_active": r.is_active != 0,
            "orchestrator": r.orchestrator,
            "ip_pool_id": r.ip_pool_id,
            "node_main_iface": r.node_main_iface,
            "pool": { "id": r.pool_id, "name": r.pool_name, "display_name": r.pool_display_name },
            "node_count": r.node_count,
            "ready_count": r.ready_count,
            "created_at": r.created_at,
        }))
        .collect::<Vec<_>>())))
}

/// GET /admin/clusters/:id
pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    #[derive(sqlx::FromRow)]
    struct ClusterRow {
        id: String, name: String, display_name: Option<String>,
        description: Option<String>, is_active: i8,
        created_at: chrono::NaiveDateTime,
        ip_pool_id: Option<String>, node_main_iface: String,
        orchestrator: String,
        pool_id: String, pool_name: String,
    }
    let c: ClusterRow = sqlx::query_as(
        "SELECT c.id, c.name, c.display_name, c.description, c.is_active, c.created_at, \
                c.ip_pool_id, c.node_main_iface, c.orchestrator, \
                p.id AS pool_id, p.name AS pool_name \
         FROM clusters c \
         JOIN resource_pools p ON p.id = c.pool_id \
         WHERE c.id = ?",
    )
    .bind(&id)
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
        "orchestrator": c.orchestrator,
        "pool": { "id": c.pool_id, "name": c.pool_name },
        "ip_pool_id": c.ip_pool_id,
        "node_main_iface": c.node_main_iface,
        "created_at": c.created_at.to_string(),
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
    /// If omitted, a "default" resource pool is created/reused automatically.
    pub pool_id: Option<String>,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    /// Pre-shared K3s join token. Auto-generated (random 32-char hex) if omitted.
    pub k3s_token: Option<String>,
    /// IP pool ID for the flat L2 network (bridge CNI).
    pub ip_pool_id: Option<String>,
    /// Physical NIC name on the node (default: "eth0").
    pub node_main_iface: Option<String>,
    /// Orchestrator type: "K3S" (default) or "DOCKER".
    pub orchestrator: Option<String>,
}

/// POST /admin/clusters
pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateClusterRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    // Resolve pool_id: use provided one, or auto-create/reuse a "default" pool
    let pool_id = if let Some(ref pid) = body.pool_id {
        let pool_exists =
            sqlx::query_scalar!(r#"SELECT COUNT(*) FROM resource_pools WHERE id = ?"#, pid)
                .fetch_one(&state.db)
                .await?;
        if pool_exists == 0 {
            return Err(AppError::NotFound(format!("resource pool {pid}")));
        }
        pid.clone()
    } else {
        // Reuse existing "default" pool or create one
        let existing =
            sqlx::query_scalar!(r#"SELECT id FROM resource_pools WHERE name = 'default' LIMIT 1"#)
                .fetch_optional(&state.db)
                .await?;
        match existing {
            Some(id) => id,
            None => {
                let pid = Uuid::new_v4().to_string();
                sqlx::query!(
                    r#"INSERT INTO resource_pools (id, name, display_name) VALUES (?, 'default', 'Default')"#,
                    pid
                )
                .execute(&state.db)
                .await?;
                pid
            }
        }
    };

    let token = body.k3s_token.unwrap_or_else(generate_k3s_token);
    let enc_token = state.crypto.encrypt(&token)?;
    let main_iface = body.node_main_iface.as_deref().unwrap_or("eth0");

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO clusters \
             (id, pool_id, name, display_name, description, k3s_token, \
              ip_pool_id, node_main_iface) \
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&pool_id)
    .bind(&body.name)
    .bind(body.display_name.as_deref().unwrap_or(&body.name))
    .bind(&body.description)
    .bind(&enc_token)
    .bind(&body.ip_pool_id)
    .bind(main_iface)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("cluster '{}' already exists", body.name))
        }
        other => AppError::Database(other),
    })?;

    // Set orchestrator if DOCKER (default is K3S from migration)
    if body.orchestrator.as_deref() == Some("DOCKER") {
        sqlx::query("UPDATE clusters SET orchestrator = 'DOCKER' WHERE id = ?")
            .bind(&id)
            .execute(&state.db)
            .await?;
    }

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

#[derive(Deserialize)]
pub struct UpdateClusterRequest {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub ip_pool_id: Option<String>,
    pub node_main_iface: Option<String>,
}

/// PUT /admin/clusters/:id
pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateClusterRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    #[derive(sqlx::FromRow)]
    struct ExistingCluster {
        display_name: Option<String>,
        description: Option<String>,
        is_active: i8,
        ip_pool_id: Option<String>,
        node_main_iface: String,
    }
    let existing: ExistingCluster = sqlx::query_as(
        "SELECT display_name, description, is_active, ip_pool_id, node_main_iface \
         FROM clusters WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("cluster {id}")))?;

    let existing_display_name = existing.display_name.unwrap_or_default();
    let display_name = body
        .display_name
        .as_deref()
        .unwrap_or(&existing_display_name);
    let description = body
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let is_active = body
        .is_active
        .map(|v| v as i8)
        .unwrap_or(existing.is_active);
    let ip_pool_id = body
        .ip_pool_id
        .as_deref()
        .or(existing.ip_pool_id.as_deref());
    let main_iface = body
        .node_main_iface
        .as_deref()
        .unwrap_or(&existing.node_main_iface);

    sqlx::query(
        "UPDATE clusters \
         SET display_name=?, description=?, is_active=?, \
             ip_pool_id=?, node_main_iface=? \
         WHERE id=?",
    )
    .bind(display_name)
    .bind(description)
    .bind(is_active)
    .bind(ip_pool_id)
    .bind(main_iface)
    .bind(&id)
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /admin/clusters/:id  (refused if nodes exist)
pub async fn delete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }

    let node_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM cluster_nodes WHERE cluster_id = ?"#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    if node_count > 0 {
        return Err(AppError::Conflict(format!(
            "cluster has {node_count} node(s); remove them first"
        )));
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
