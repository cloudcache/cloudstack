/// Flat L2 network management — bridge CNI with host-local IPAM.
///
/// Architecture:
///   Each cluster has a single IP pool (clusters.ip_pool_id).
///   Bridge CNI is the default (and only) CNI — configured via conflist on each node.
///   host-local IPAM auto-assigns pod IPs from the pool's subnet.
///   After a pod reaches RUNNING, status_sync reads its IP from pod.status and
///   records it in app_ip_allocations for visibility / tracking.
///
/// No Multus, no NADs, no annotation injection.
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

// ── Post-deploy IP recording ────────────────────────────────────────────────

/// Records a pod's IP (read from pod.status.podIP) into app_ip_allocations.
/// Called by status_sync when an app transitions to RUNNING.
/// Idempotent: skips if already recorded for this app+pool.
pub async fn record_pod_ip(
    state: &AppState,
    app_id: &str,
    cluster_id: &str,
    pod_ip: &str,
) -> AppResult<()> {
    let pool_id: Option<String> = sqlx::query_scalar(
        "SELECT ip_pool_id FROM clusters WHERE id = ?",
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    let Some(pool_id) = pool_id else { return Ok(()) };

    // Idempotent: already recorded?
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_ip_allocations WHERE app_id = ? AND pool_id = ?",
    )
    .bind(app_id)
    .bind(&pool_id)
    .fetch_one(&state.db)
    .await?;

    if count > 0 {
        return Ok(());
    }

    // Write to ip_allocations (global tracker)
    let alloc_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ip_allocations (id, pool_id, ip_address, allocated_to, purpose) \
         VALUES (?, ?, ?, ?, 'app-network')",
    )
    .bind(&alloc_id)
    .bind(&pool_id)
    .bind(pod_ip)
    .bind(app_id)
    .execute(&state.db)
    .await?;

    // Write to app_ip_allocations (fast per-app lookup)
    let aia_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO app_ip_allocations (id, app_id, pool_id, ip_address, alloc_ref_id) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&aia_id)
    .bind(app_id)
    .bind(&pool_id)
    .bind(pod_ip)
    .bind(&alloc_id)
    .execute(&state.db)
    .await?;

    tracing::info!(app_id, pod_ip, "recorded pod IP from CNI");
    Ok(())
}

// ── IP release (on app delete) ──────────────────────────────────────────────

/// Releases all fixed IPs held by an app (called on app delete).
pub async fn release_app_ips(state: &AppState, app_id: &str) -> AppResult<()> {
    let alloc_ref_ids: Vec<String> = sqlx::query_scalar!(
        r#"SELECT alloc_ref_id FROM app_ip_allocations WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    for rid in &alloc_ref_ids {
        let _ = sqlx::query!(r#"DELETE FROM ip_allocations WHERE id = ?"#, rid)
            .execute(&state.db)
            .await;
    }

    sqlx::query!(r#"DELETE FROM app_ip_allocations WHERE app_id = ?"#, app_id)
        .execute(&state.db)
        .await?;

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Returns the fixed IPs for an app as a compact summary (for logging).
pub async fn app_ip_summary(state: &AppState, app_id: &str) -> String {
    let rows = sqlx::query!(
        r#"SELECT aia.ip_address, p.pool_type
           FROM app_ip_allocations aia
           JOIN ip_pools p ON p.id = aia.pool_id
           WHERE aia.app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| format!("{}({})", r.ip_address, r.pool_type))
        .collect::<Vec<_>>()
        .join(", ")
}

// ── Docker-mode IPAM (pre-allocate from pool) ───────────────────────────────
// Docker containers need IPs assigned before creation (passed to docker run --ip).
// K8s pods get IPs from host-local IPAM automatically; this is Docker-only.

/// Allocates a new IP for the app from the pool, or returns the already-allocated IP.
/// Uses a transaction with row-level locking to prevent concurrent duplicate allocation.
pub async fn allocate_ip_for_docker(
    state: &AppState,
    app_id: &str,
    pool_id: &str,
) -> AppResult<String> {
    // Check if already allocated (fast path, no lock needed)
    if let Some(existing) = sqlx::query_scalar!(
        r#"SELECT ip_address FROM app_ip_allocations WHERE app_id = ? AND pool_id = ?"#,
        app_id,
        pool_id
    )
    .fetch_optional(&state.db)
    .await?
    {
        return Ok(existing);
    }

    // Allocate inside a transaction with row-level locking
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Internal(format!("begin tx: {e}")))?;

    // Lock the pool row to serialize concurrent allocations for the same pool
    #[derive(sqlx::FromRow)]
    struct PoolRow {
        cidr: String,
        is_active: i8,
    }
    let pool: PoolRow =
        sqlx::query_as("SELECT cidr, is_active FROM ip_pools WHERE id = ? FOR UPDATE")
            .bind(pool_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("ip pool {pool_id}")))?;

    if pool.is_active == 0 {
        return Err(AppError::BadRequest("IP pool is inactive".into()));
    }

    // Re-check inside transaction (another request may have allocated concurrently)
    if let Some(existing) = sqlx::query_scalar!(
        r#"SELECT ip_address FROM app_ip_allocations WHERE app_id = ? AND pool_id = ?"#,
        app_id,
        pool_id
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        tx.rollback().await.ok();
        return Ok(existing);
    }

    let taken: std::collections::HashSet<String> = sqlx::query_scalar!(
        r#"SELECT ip_address FROM ip_allocations WHERE pool_id = ?"#,
        pool_id
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();

    let ip = crate::api::ipam::cidr_usable_ips(&pool.cidr)?
        .into_iter()
        .find(|ip| !taken.contains(ip))
        .ok_or_else(|| AppError::Conflict("no available IPs in pool".into()))?;

    // Write to ip_allocations (global tracker)
    let alloc_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO ip_allocations (id, pool_id, ip_address, allocated_to, purpose)
           VALUES (?, ?, ?, ?, 'app-network')"#,
        alloc_id,
        pool_id,
        ip,
        app_id,
    )
    .execute(&mut *tx)
    .await?;

    // Write to app_ip_allocations (fast per-app lookup)
    let aia_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_ip_allocations (id, app_id, pool_id, ip_address, alloc_ref_id)
           VALUES (?, ?, ?, ?, ?)"#,
        aia_id,
        app_id,
        pool_id,
        ip,
        alloc_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("commit ip alloc: {e}")))?;

    Ok(ip)
}
