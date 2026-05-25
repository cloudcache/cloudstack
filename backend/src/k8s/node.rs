use kube::api::{Api, Patch, PatchParams};
use k8s_openapi::api::core::v1::{ConfigMap, Node};
use tokio::time::{sleep, Duration, Instant};

use crate::{error::{AppError, AppResult}, state::AppState};

pub async fn provision_node(
    state: &AppState,
    node_id: &str,
    cluster_id: &str,
    ip: &str,
    hostname: &str,
    ssh_password: &str,
    role: &str,
    storage_path: &str,
) -> AppResult<()> {
    use crate::ssh::NodeInstaller;

    sqlx::query!(
        r#"UPDATE cluster_nodes SET node_status = 'PROVISIONING' WHERE id = ?"#,
        node_id
    )
    .execute(&state.db)
    .await?;

    let pub_key = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'ssh_public_key'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_default();

    let priv_key_enc = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'ssh_private_key'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_default();
    let priv_key = state.crypto.decrypt(&priv_key_enc)?;

    let k3s_token_enc = sqlx::query_scalar!(
        r#"SELECT k3s_token FROM clusters WHERE id = ?"#, cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .ok_or_else(|| AppError::Internal(format!("cluster {cluster_id} has no k3s_token")))?;
    let k3s_token = state.crypto.decrypt(&k3s_token_enc)?;

    let master_ip = sqlx::query_scalar!(
        r#"SELECT ip_address FROM cluster_nodes
           WHERE cluster_id = ? AND node_role = 'MASTER' LIMIT 1"#,
        cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_default();

    let installer = NodeInstaller::new(ip, ssh_password, &pub_key, &priv_key);
    let result = installer
        .run(hostname, &master_ip, &k3s_token, role == "MASTER", storage_path)
        .await?;

    // Store main network interface detected on the node
    sqlx::query!(
        r#"UPDATE cluster_nodes SET main_iface = ? WHERE id = ?"#,
        result.main_iface, node_id
    )
    .execute(&state.db)
    .await?;

    // Store GPU info detected during provisioning
    if result.has_gpu {
        sqlx::query!(
            r#"UPDATE cluster_nodes SET has_gpu = 1, gpu_model = ?, gpu_count = ? WHERE id = ?"#,
            result.gpu_model, result.gpu_count, node_id
        )
        .execute(&state.db)
        .await?;
    }

    // Master: persist kubeconfig and configure local-path storage class
    if let Some(kc) = result.kubeconfig {
        let kc_enc = state.crypto.encrypt(&kc)?;
        sqlx::query!(
            r#"UPDATE clusters SET kubeconfig = ? WHERE id = ?"#,
            kc_enc, cluster_id
        )
        .execute(&state.db)
        .await?;

        configure_local_path_storage(state, cluster_id, storage_path).await?;

        // Store detected interface as the cluster-level macvlan master (first master wins)
        sqlx::query!(
            r#"UPDATE clusters SET node_main_iface = ? WHERE id = ? AND node_main_iface = 'eth0'"#,
            result.main_iface, cluster_id
        )
        .execute(&state.db)
        .await?;
    }

    let pod_cidr = wait_for_node_ready(state, cluster_id, hostname).await?;

    sqlx::query!(
        r#"UPDATE cluster_nodes
           SET node_status = 'READY', pod_cidr = ?, last_seen_at = NOW()
           WHERE id = ?"#,
        pod_cidr, node_id
    )
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn configure_local_path_storage(
    state: &AppState,
    cluster_id: &str,
    storage_path: &str,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let cm_api: Api<ConfigMap> = Api::namespaced(client, "kube-system");

    let config_json = serde_json::json!({
        "nodePathMap": [{
            "node": "DEFAULT_PATH_FOR_NON_LISTED_NODES",
            "paths": [storage_path]
        }]
    });

    let patch = serde_json::json!({
        "data": { "config.json": serde_json::to_string(&config_json).unwrap() }
    });

    cm_api
        .patch("local-path-config", &PatchParams::default(), &Patch::Merge(patch))
        .await?;

    Ok(())
}

async fn wait_for_node_ready(
    state: &AppState,
    cluster_id: &str,
    hostname: &str,
) -> AppResult<Option<String>> {
    let deadline = Instant::now() + Duration::from_secs(300);
    let client = super::client_for_cluster(state, cluster_id).await?;
    let node_api: Api<Node> = Api::all(client);

    loop {
        match node_api.get_opt(hostname).await {
            Ok(Some(node)) => {
                let ready = node.status.as_ref()
                    .and_then(|s| s.conditions.as_ref())
                    .and_then(|c| c.iter().find(|c| c.type_ == "Ready"))
                    .map(|c| c.status == "True")
                    .unwrap_or(false);

                if ready {
                    return Ok(node.spec.as_ref().and_then(|s| s.pod_cidr.clone()));
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("K8s node poll error for {hostname}: {e}"),
        }

        if Instant::now() >= deadline {
            return Err(AppError::Internal(
                format!("node {hostname} did not become Ready within 5 minutes")
            ));
        }

        sleep(Duration::from_secs(10)).await;
    }
}

pub async fn sync_node_status(
    state: &AppState,
    node_id: &str,
    cluster_id: &str,
    hostname: &str,
) -> AppResult<String> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let node_api: Api<Node> = Api::all(client);

    let (status, pod_cidr) = match node_api.get_opt(hostname).await? {
        Some(node) => {
            let ready = node.status.as_ref()
                .and_then(|s| s.conditions.as_ref())
                .and_then(|c| c.iter().find(|c| c.type_ == "Ready"))
                .map(|c| c.status == "True")
                .unwrap_or(false);
            let cidr = node.spec.as_ref().and_then(|s| s.pod_cidr.clone());
            (if ready { "READY" } else { "NOT_READY" }.to_string(), cidr)
        }
        None => ("UNKNOWN".to_string(), None),
    };

    sqlx::query!(
        r#"UPDATE cluster_nodes
           SET node_status = ?, pod_cidr = COALESCE(?, pod_cidr), last_seen_at = NOW()
           WHERE id = ?"#,
        status, pod_cidr, node_id
    )
    .execute(&state.db)
    .await?;

    Ok(status)
}

pub async fn drain_and_delete(
    state: &AppState,
    cluster_id: &str,
    hostname: &str,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let node_api: Api<Node> = Api::all(client);

    let cordon = serde_json::json!({ "spec": { "unschedulable": true } });
    let _ = node_api
        .patch(hostname, &PatchParams::default(), &Patch::Merge(cordon))
        .await;
    let _ = node_api
        .delete(hostname, &kube::api::DeleteParams::default())
        .await;

    Ok(())
}

/// Cordon or uncordon a node (set/clear unschedulable).
pub async fn set_schedulable(
    state: &AppState,
    cluster_id: &str,
    hostname: &str,
    schedulable: bool,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let node_api: Api<Node> = Api::all(client);
    let patch = serde_json::json!({ "spec": { "unschedulable": !schedulable } });
    node_api
        .patch(hostname, &PatchParams::default(), &Patch::Merge(patch))
        .await
        .map_err(|e| AppError::Kubernetes(e.into()))?;
    Ok(())
}

pub async fn apply_labels(
    state: &AppState,
    cluster_id: &str,
    hostname: &str,
    labels: &serde_json::Value,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let node_api: Api<Node> = Api::all(client);

    let patch = serde_json::json!({ "metadata": { "labels": labels } });
    node_api
        .patch(hostname, &PatchParams::default(), &Patch::Merge(patch))
        .await?;

    Ok(())
}

// ── node_exporter metrics ──────────────────────────────────────────────────────

pub struct NodeMetrics {
    pub cpu_used_pct: f32,
    pub mem_total_mb: u64,
    pub mem_used_mb: u64,
    pub disk_total_gb: u64,
    pub disk_used_gb: u64,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
}

/// Fetches metrics from node_exporter on port 9100 and parses key indicators.
pub async fn fetch_node_metrics(node_ip: &str) -> AppResult<NodeMetrics> {
    let url = format!("http://{}:9100/metrics", node_ip);

    let body = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("node_exporter fetch: {e}")))?
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("node_exporter read: {e}")))?;

    parse_node_exporter_metrics(&body)
}

fn parse_node_exporter_metrics(text: &str) -> AppResult<NodeMetrics> {
    let mut mem_total: f64 = 0.0;
    let mut mem_available: f64 = 0.0;
    let mut disk_total: f64 = 0.0;
    let mut disk_avail: f64 = 0.0;
    let mut load1: f32 = 0.0;
    let mut load5: f32 = 0.0;
    let mut load15: f32 = 0.0;
    // For CPU %: track idle seconds vs total seconds across all CPUs
    let mut cpu_idle: f64 = 0.0;
    let mut cpu_total: f64 = 0.0;

    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }

        // Split at last space to get metric_name{labels} value
        let (name_labels, value_str) = match line.rsplit_once(' ') {
            Some(pair) => pair,
            None => continue,
        };
        let value: f64 = match value_str.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        if name_labels.starts_with("node_cpu_seconds_total") {
            cpu_total += value;
            if name_labels.contains("mode=\"idle\"") {
                cpu_idle += value;
            }
        } else if name_labels == "node_memory_MemTotal_bytes" {
            mem_total = value;
        } else if name_labels == "node_memory_MemAvailable_bytes" {
            mem_available = value;
        } else if name_labels.starts_with("node_filesystem_size_bytes")
            && (name_labels.contains("mountpoint=\"/\"") || name_labels.contains("mountpoint=\"/ \""))
        {
            disk_total = value;
        } else if name_labels.starts_with("node_filesystem_avail_bytes")
            && (name_labels.contains("mountpoint=\"/\"") || name_labels.contains("mountpoint=\"/ \""))
        {
            disk_avail = value;
        } else if name_labels == "node_load1" {
            load1 = value as f32;
        } else if name_labels == "node_load5" {
            load5 = value as f32;
        } else if name_labels == "node_load15" {
            load15 = value as f32;
        }
    }

    let cpu_used_pct = if cpu_total > 0.0 {
        ((1.0 - cpu_idle / cpu_total) * 100.0) as f32
    } else {
        0.0
    };
    let mem_used_mb = ((mem_total - mem_available) / 1_048_576.0) as u64;
    let mem_total_mb = (mem_total / 1_048_576.0) as u64;
    let disk_total_gb = (disk_total / 1_073_741_824.0) as u64;
    let disk_used_gb = ((disk_total - disk_avail) / 1_073_741_824.0) as u64;

    Ok(NodeMetrics { cpu_used_pct, mem_total_mb, mem_used_mb, disk_total_gb, disk_used_gb, load1, load5, load15 })
}

/// Fetches live metrics from node_exporter and caches them in the DB.
pub async fn refresh_node_metrics(
    state: &AppState,
    node_id: &str,
    node_ip: &str,
) -> AppResult<NodeMetrics> {
    let m = fetch_node_metrics(node_ip).await?;

    sqlx::query!(
        r#"UPDATE cluster_nodes
           SET cpu_used_pct = ?, mem_used_mb = ?, disk_used_gb = ?, disk_total_gb = ?,
               load1 = ?, metrics_updated_at = NOW()
           WHERE id = ?"#,
        m.cpu_used_pct, m.mem_used_mb as i64, m.disk_used_gb as i64, m.disk_total_gb as i64,
        m.load1, node_id
    )
    .execute(&state.db)
    .await?;

    Ok(m)
}
