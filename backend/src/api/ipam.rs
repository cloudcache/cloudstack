use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

// ─── IP pools ─────────────────────────────────────────────────────────────────

/// GET /admin/ip-pools
pub async fn list_pools(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.cidr, p.pool_type, p.gateway, p.description, p.is_active, p.created_at,
                  COUNT(a.id) AS allocated_count
           FROM ip_pools p
           LEFT JOIN ip_allocations a ON a.pool_id = p.id
           GROUP BY p.id ORDER BY p.name"#
    )
    .fetch_all(&state.db)
    .await?;

    // Bindings: for each ip_pool, which clusters use it and how many nodes
    // does each of those clusters have. Fetched once and grouped in memory
    // (avoids N+1 queries).
    #[derive(sqlx::FromRow)]
    struct BindingRow {
        ip_pool_id: String,
        cluster_id: String,
        cluster_name: String,
        cluster_display_name: Option<String>,
        resource_pool_name: String,
        resource_pool_display_name: Option<String>,
        node_count: i64,
    }
    let bindings: Vec<BindingRow> = sqlx::query_as(
        "SELECT c.ip_pool_id AS ip_pool_id, \
                c.id AS cluster_id, \
                c.name AS cluster_name, \
                c.display_name AS cluster_display_name, \
                rp.name AS resource_pool_name, \
                rp.display_name AS resource_pool_display_name, \
                COUNT(n.id) AS node_count \
         FROM clusters c \
         JOIN resource_pools rp ON rp.id = c.pool_id \
         LEFT JOIN cluster_nodes n ON n.cluster_id = c.id \
         WHERE c.ip_pool_id IS NOT NULL \
         GROUP BY c.id",
    )
    .fetch_all(&state.db)
    .await?;

    let mut by_pool: std::collections::HashMap<String, Vec<serde_json::Value>> = Default::default();
    for b in &bindings {
        by_pool.entry(b.ip_pool_id.clone()).or_default().push(serde_json::json!({
            "cluster_id":               b.cluster_id,
            "cluster_name":             b.cluster_name,
            "cluster_display_name":     b.cluster_display_name,
            "resource_pool_name":       b.resource_pool_name,
            "resource_pool_display_name": b.resource_pool_display_name,
            "node_count":               b.node_count,
        }));
    }

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "cidr": r.cidr,
        "pool_type": r.pool_type,
        "gateway": r.gateway,
        "description": r.description,
        "is_active": r.is_active != 0,
        "allocated_count": r.allocated_count,
        "total_count": cidr_size(&r.cidr),
        "created_at": r.created_at,
        "bindings": by_pool.get(&r.id).cloned().unwrap_or_default(),
    })).collect::<Vec<_>>())))
}

/// GET /admin/ip-pools/:id
pub async fn get_pool(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let r = sqlx::query!(
        r#"SELECT id, name, cidr, pool_type, gateway, description, is_active, created_at
           FROM ip_pools WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ip pool {id}")))?;

    let allocated = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM ip_allocations WHERE pool_id = ?"#, id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": r.id,
        "name": r.name,
        "cidr": r.cidr,
        "pool_type": r.pool_type,
        "gateway": r.gateway,
        "description": r.description,
        "is_active": r.is_active != 0,
        "allocated_count": allocated,
        "total_count": cidr_size(&r.cidr),
        "created_at": r.created_at,
    })))
}

#[derive(Deserialize)]
pub struct CreatePoolRequest {
    pub name: String,
    pub cidr: String,
    pub pool_type: Option<String>,
    pub gateway: Option<String>,
    pub description: Option<String>,
}

/// POST /admin/ip-pools
pub async fn create_pool(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreatePoolRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    // Validate CIDR before inserting
    cidr_usable_ips(&body.cidr)?;

    let id = Uuid::new_v4().to_string();
    let pool_type = body.pool_type.as_deref().unwrap_or("LB");

    sqlx::query!(
        r#"INSERT INTO ip_pools (id, name, cidr, pool_type, gateway, description)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        id, body.name, body.cidr, pool_type, body.gateway, body.description,
    )
    .execute(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdatePoolRequest {
    pub name: Option<String>,
    pub gateway: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

/// PUT /admin/ip-pools/:id
pub async fn update_pool(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdatePoolRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let existing = sqlx::query!(
        r#"SELECT name, gateway, description, is_active FROM ip_pools WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ip pool {id}")))?;

    let name = body.name.as_deref().unwrap_or(&existing.name);
    let is_active = body.is_active.map(|v| v as i8).unwrap_or(existing.is_active);
    let gateway = body.gateway.as_deref().or(existing.gateway.as_deref());
    let description = body.description.as_deref().or(existing.description.as_deref());

    sqlx::query!(
        r#"UPDATE ip_pools SET name=?, gateway=?, description=?, is_active=? WHERE id=?"#,
        name, gateway, description, is_active, id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /admin/ip-pools/:id  (refused if allocations exist)
pub async fn delete_pool(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    // Refuse delete if a cluster still references this pool (FK is ON DELETE
    // SET NULL — silently severing the binding would surprise admins).
    let cluster_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM clusters WHERE ip_pool_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await?;
    if cluster_count > 0 {
        return Err(AppError::Conflict(format!(
            "pool is bound to {cluster_count} cluster(s); detach them first"
        )));
    }

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM ip_allocations WHERE pool_id = ?"#, id
    )
    .fetch_one(&state.db)
    .await?;

    if count > 0 {
        return Err(AppError::Conflict(
            format!("pool has {count} active allocations; release them first")
        ));
    }

    sqlx::query!(r#"DELETE FROM ip_pools WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Allocations ──────────────────────────────────────────────────────────────

/// GET /admin/ip-pools/:id/allocations
pub async fn list_allocations(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT id, ip_address, allocated_to, purpose, allocated_at
           FROM ip_allocations WHERE pool_id = ? ORDER BY ip_address"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "ip_address": r.ip_address,
        "allocated_to": r.allocated_to,
        "purpose": r.purpose,
        "allocated_at": r.allocated_at,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct AllocateRequest {
    pub allocated_to: Option<String>,
    pub purpose: Option<String>,
    /// Optionally request a specific IP instead of the next available one.
    pub ip_address: Option<String>,
}

/// POST /admin/ip-pools/:id/allocate
/// Allocates the next available IP in the pool (or a specific one if requested).
pub async fn allocate(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<AllocateRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let pool = sqlx::query!(
        r#"SELECT cidr, is_active FROM ip_pools WHERE id = ?"#, id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ip pool {id}")))?;

    if pool.is_active == 0 {
        return Err(AppError::BadRequest("pool is inactive".into()));
    }

    // Get already-allocated IPs
    let taken: std::collections::HashSet<String> = sqlx::query_scalar!(
        r#"SELECT ip_address FROM ip_allocations WHERE pool_id = ?"#, id
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect();

    let ip = if let Some(requested) = body.ip_address {
        // Validate requested IP is in the pool's CIDR
        let usable = cidr_usable_ips(&pool.cidr)?;
        if !usable.contains(&requested) {
            return Err(AppError::BadRequest(
                format!("{requested} is not in pool CIDR {}", pool.cidr)
            ));
        }
        if taken.contains(&requested) {
            return Err(AppError::Conflict(format!("{requested} is already allocated")));
        }
        requested
    } else {
        cidr_usable_ips(&pool.cidr)?
            .into_iter()
            .find(|ip| !taken.contains(ip))
            .ok_or_else(|| AppError::Conflict("no available IPs in pool".into()))?
    };

    let alloc_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO ip_allocations (id, pool_id, ip_address, allocated_to, purpose)
           VALUES (?, ?, ?, ?, ?)"#,
        alloc_id, id, ip, body.allocated_to, body.purpose,
    )
    .execute(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({
        "id": alloc_id,
        "ip_address": ip,
        "pool_id": id,
    }))))
}

/// DELETE /admin/ip-pools/:pool_id/allocations/:ip
pub async fn release(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((pool_id, ip)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let result = sqlx::query!(
        r#"DELETE FROM ip_allocations WHERE pool_id = ? AND ip_address = ?"#,
        pool_id, ip,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("{ip} not allocated in pool {pool_id}")));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── CIDR utilities ───────────────────────────────────────────────────────────

/// Returns number of usable host addresses in the CIDR (for display only).
fn cidr_size(cidr: &str) -> u64 {
    cidr_usable_ips(cidr).map(|v| v.len() as u64).unwrap_or(0)
}

/// Returns the list of usable host IPs in the CIDR (excludes network + broadcast for /0–/30).
pub fn cidr_usable_ips(cidr: &str) -> AppResult<Vec<String>> {
    let (ip_str, prefix_str) = cidr.split_once('/')
        .ok_or_else(|| AppError::BadRequest("CIDR must be in the form x.x.x.x/n".into()))?;

    let prefix: u32 = prefix_str.parse()
        .map_err(|_| AppError::BadRequest("invalid prefix length".into()))?;
    if prefix > 32 {
        return Err(AppError::BadRequest("prefix length must be 0–32".into()));
    }

    let octets: Vec<u32> = ip_str.split('.')
        .map(|s| s.parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| AppError::BadRequest("invalid IP address in CIDR".into()))?;

    if octets.len() != 4 || octets.iter().any(|&o| o > 255) {
        return Err(AppError::BadRequest("invalid IP address in CIDR".into()));
    }

    let base = (octets[0] << 24) | (octets[1] << 16) | (octets[2] << 8) | octets[3];
    let mask = if prefix == 0 { 0u32 } else { !0u32 << (32 - prefix) };
    let network = base & mask;
    let broadcast = network | !mask;

    let (start, end) = if prefix >= 31 {
        (network, broadcast)     // /31 and /32: all addresses usable
    } else {
        (network + 1, broadcast - 1)
    };

    if start > end {
        return Err(AppError::BadRequest("CIDR has no usable addresses".into()));
    }

    let mut ips = Vec::with_capacity((end - start + 1) as usize);
    let mut ip = start;
    while ip <= end {
        ips.push(format!("{}.{}.{}.{}",
            (ip >> 24) & 0xFF, (ip >> 16) & 0xFF, (ip >> 8) & 0xFF, ip & 0xFF
        ));
        ip += 1;
    }
    Ok(ips)
}
