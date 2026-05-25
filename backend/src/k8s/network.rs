/// Network attachment management using Multus CNI.
///
/// Architecture:
///   Each cluster can have two IP pools: VPC (internal services) and PUBLIC (external-facing).
///   Multus installs a secondary CNI (macvlan) alongside K3s flannel.
///   A NetworkAttachmentDefinition (NAD) is created in the "default" namespace once per pool.
///   At deploy time, each app gets a fixed IP allocated from the pool and injected via pod annotation.
///
/// Pod network interfaces:
///   eth0 - K3s flannel (cluster-internal pod IP, 10.244.x.x)
///   net1  - VPC macvlan (fixed IP from vpc pool, e.g. 10.10.0.50/24)
///   net2  - Public macvlan (fixed IP from pub pool, e.g. 172.16.0.50/24)

use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, Patch, PatchParams};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

fn nad_api_resource() -> ApiResource {
    ApiResource {
        group: "k8s.cni.cncf.io".to_string(),
        version: "v1".to_string(),
        api_version: "k8s.cni.cncf.io/v1".to_string(),
        kind: "NetworkAttachmentDefinition".to_string(),
        plural: "network-attachment-definitions".to_string(),
    }
}

/// Name used for the NAD created for each pool (in the "default" namespace).
/// Must be a valid DNS label.
fn nad_name_for_pool(pool_name: &str) -> String {
    format!("qs-{}", pool_name.replace('_', "-").to_lowercase())
}

/// Creates or updates the NetworkAttachmentDefinition for an IP pool.
///
/// The NAD uses macvlan bridge mode on the cluster's main NIC.
/// Static IPAM is used — the actual IP is injected per-pod via annotation.
pub async fn ensure_network_attachment_def(
    client: &kube::Client,
    pool_name: &str,
    pool_cidr: &str,
    gateway: Option<&str>,
    master_iface: &str,
) -> AppResult<()> {
    let nad_name = nad_name_for_pool(pool_name);
    let prefix = pool_cidr.split('/').nth(1).unwrap_or("24");

    let cni_config = serde_json::json!({
        "cniVersion": "0.3.1",
        "name": nad_name,
        "type": "macvlan",
        "master": master_iface,
        "mode": "bridge",
        "ipam": {
            "type": "static",
            "routes": gateway.map(|gw| serde_json::json!([{"dst": "0.0.0.0/0", "gw": gw}]))
        }
    });

    let ar = nad_api_resource();
    // NADs are created in "default" namespace; pods reference them cross-namespace.
    let nad_api: Api<DynamicObject> = Api::namespaced_with(client.clone(), "default", &ar);

    let mut nad = DynamicObject::new(&nad_name, &ar);
    nad.metadata.namespace = Some("default".to_string());
    nad.metadata.annotations = Some({
        let mut a = std::collections::BTreeMap::new();
        a.insert("qs-pool-cidr".to_string(), pool_cidr.to_string());
        a.insert("qs-prefix-len".to_string(), prefix.to_string());
        if let Some(gw) = gateway {
            a.insert("qs-gateway".to_string(), gw.to_string());
        }
        a
    });
    nad.data = serde_json::json!({
        "spec": {
            "config": serde_json::to_string(&cni_config).unwrap()
        }
    });

    nad_api
        .patch(
            &nad_name,
            &PatchParams::apply("quickstack"),
            &Patch::Apply(&nad),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.into()))?;

    Ok(())
}

/// Ensures NetworkAttachmentDefinitions exist for both VPC and public pools of a cluster.
/// Called when a cluster's vpc_pool_id / pub_pool_id is set or updated.
pub async fn ensure_cluster_nads(state: &AppState, cluster_id: &str) -> AppResult<()> {
    let row = sqlx::query!(
        r#"SELECT c.vpc_pool_id, c.pub_pool_id, c.node_main_iface,
                  vp.name AS vpc_name, vp.cidr AS vpc_cidr, vp.gateway AS vpc_gw,
                  pp.name AS pub_name, pp.cidr AS pub_cidr, pp.gateway AS pub_gw
           FROM clusters c
           LEFT JOIN ip_pools vp ON vp.id = c.vpc_pool_id
           LEFT JOIN ip_pools pp ON pp.id = c.pub_pool_id
           WHERE c.id = ?"#,
        cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("cluster {cluster_id}")))?;

    let client = super::client_for_cluster(state, cluster_id).await?;

    if let (Some(name), Some(cidr)) = (&row.vpc_name, &row.vpc_cidr) {
        ensure_network_attachment_def(
            &client, name, cidr, row.vpc_gw.as_deref(), &row.node_main_iface,
        ).await?;
    }
    if let (Some(name), Some(cidr)) = (&row.pub_name, &row.pub_cidr) {
        ensure_network_attachment_def(
            &client, name, cidr, row.pub_gw.as_deref(), &row.node_main_iface,
        ).await?;
    }

    Ok(())
}

/// Result of looking up or allocating an app's fixed IPs.
pub struct AppNetworkIps {
    /// VPC IP with prefix (e.g. "10.10.0.50/24") and NAD name, if VPC pool is configured.
    pub vpc: Option<IpAssignment>,
    /// Public IP with prefix (e.g. "172.16.0.50/24") and NAD name, if pub pool is configured.
    pub pub_zone: Option<IpAssignment>,
}

pub struct IpAssignment {
    /// IP with CIDR prefix, ready for Multus annotation (e.g. "10.10.0.50/24").
    pub ip_with_prefix: String,
    /// Cross-namespace NAD reference (e.g. "default/qs-vpc-internal").
    pub nad_ref: String,
    /// Gateway IP, if any.
    pub gateway: Option<String>,
}

/// Returns (or allocates) the fixed IPs for an app from the cluster's VPC and public pools.
/// Idempotent: re-uses the same IP on re-deploy.
pub async fn get_or_allocate_app_ips(
    state: &AppState,
    app_id: &str,
    cluster_id: &str,
) -> AppResult<AppNetworkIps> {
    let row = sqlx::query!(
        r#"SELECT c.vpc_pool_id, c.pub_pool_id,
                  vp.name AS vpc_name, vp.cidr AS vpc_cidr, vp.gateway AS vpc_gw,
                  pp.name AS pub_name, pp.cidr AS pub_cidr, pp.gateway AS pub_gw
           FROM clusters c
           LEFT JOIN ip_pools vp ON vp.id = c.vpc_pool_id
           LEFT JOIN ip_pools pp ON pp.id = c.pub_pool_id
           WHERE c.id = ?"#,
        cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("cluster {cluster_id}")))?;

    let vpc = if let (Some(pool_id), Some(pool_name), Some(cidr)) =
        (&row.vpc_pool_id, &row.vpc_name, &row.vpc_cidr)
    {
        let ip = get_or_allocate_ip(state, app_id, pool_id).await?;
        let prefix = cidr.split('/').nth(1).unwrap_or("24");
        Some(IpAssignment {
            ip_with_prefix: format!("{ip}/{prefix}"),
            nad_ref: format!("default/{}", nad_name_for_pool(pool_name)),
            gateway: row.vpc_gw.clone(),
        })
    } else {
        None
    };

    let pub_zone = if let (Some(pool_id), Some(pool_name), Some(cidr)) =
        (&row.pub_pool_id, &row.pub_name, &row.pub_cidr)
    {
        let ip = get_or_allocate_ip(state, app_id, pool_id).await?;
        let prefix = cidr.split('/').nth(1).unwrap_or("24");
        Some(IpAssignment {
            ip_with_prefix: format!("{ip}/{prefix}"),
            nad_ref: format!("default/{}", nad_name_for_pool(pool_name)),
            gateway: row.pub_gw.clone(),
        })
    } else {
        None
    };

    Ok(AppNetworkIps { vpc, pub_zone })
}

/// Allocates a new IP for the app from the pool, or returns the already-allocated IP.
async fn get_or_allocate_ip(
    state: &AppState,
    app_id: &str,
    pool_id: &str,
) -> AppResult<String> {
    // Check if already allocated
    if let Some(existing) = sqlx::query_scalar!(
        r#"SELECT ip_address FROM app_ip_allocations WHERE app_id = ? AND pool_id = ?"#,
        app_id, pool_id
    )
    .fetch_optional(&state.db)
    .await?
    {
        return Ok(existing);
    }

    // Allocate: first-fit from pool
    let pool = sqlx::query!(
        r#"SELECT cidr, is_active FROM ip_pools WHERE id = ?"#, pool_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ip pool {pool_id}")))?;

    if pool.is_active == 0 {
        return Err(AppError::BadRequest("IP pool is inactive".into()));
    }

    let taken: std::collections::HashSet<String> = sqlx::query_scalar!(
        r#"SELECT ip_address FROM ip_allocations WHERE pool_id = ?"#, pool_id
    )
    .fetch_all(&state.db)
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
        alloc_id, pool_id, ip, app_id,
    )
    .execute(&state.db)
    .await?;

    // Write to app_ip_allocations (fast per-app lookup)
    let aia_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_ip_allocations (id, app_id, pool_id, ip_address, alloc_ref_id)
           VALUES (?, ?, ?, ?, ?)"#,
        aia_id, app_id, pool_id, ip, alloc_id,
    )
    .execute(&state.db)
    .await?;

    Ok(ip)
}

/// Releases all fixed IPs held by an app (called on app delete).
pub async fn release_app_ips(state: &AppState, app_id: &str) -> AppResult<()> {
    let alloc_ref_ids: Vec<String> = sqlx::query_scalar!(
        r#"SELECT alloc_ref_id FROM app_ip_allocations WHERE app_id = ?"#, app_id
    )
    .fetch_all(&state.db)
    .await?;

    // Delete global ip_allocations rows (FK cascade will delete app_ip_allocations)
    for rid in &alloc_ref_ids {
        let _ = sqlx::query!(
            r#"DELETE FROM ip_allocations WHERE id = ?"#, rid
        )
        .execute(&state.db)
        .await;
    }

    // Clean up directly in case FK cascade wasn't set
    sqlx::query!(r#"DELETE FROM app_ip_allocations WHERE app_id = ?"#, app_id)
        .execute(&state.db)
        .await?;

    Ok(())
}

/// Builds the Multus network annotation value for a pod.
/// Returns None if neither VPC nor public IPs are assigned.
pub fn build_network_annotation(ips: &AppNetworkIps) -> Option<String> {
    let mut networks: Vec<serde_json::Value> = Vec::new();

    if let Some(vpc) = &ips.vpc {
        let mut entry = serde_json::json!({
            "name": &vpc.nad_ref,
            "ips": [&vpc.ip_with_prefix],
        });
        if let Some(gw) = &vpc.gateway {
            entry["gateway"] = serde_json::json!([gw]);
        }
        networks.push(entry);
    }
    if let Some(pub_zone) = &ips.pub_zone {
        let mut entry = serde_json::json!({
            "name": &pub_zone.nad_ref,
            "ips": [&pub_zone.ip_with_prefix],
        });
        if let Some(gw) = &pub_zone.gateway {
            entry["gateway"] = serde_json::json!([gw]);
        }
        networks.push(entry);
    }

    if networks.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&networks).unwrap())
    }
}

// ── Public IPAM helpers re-exported for use in k8s/deployment.rs ──────────────

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
