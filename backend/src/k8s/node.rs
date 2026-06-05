use k8s_openapi::api::core::v1::{ConfigMap, Node};
use kube::api::{Api, Patch, PatchParams};
use tokio::time::{sleep, Duration, Instant};

use crate::{
    error::{AppError, AppResult},
    ssh::ProvisionLogger,
    state::AppState,
};

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

    let logger = ProvisionLogger::begin(state.db.clone(), node_id).await?;

    // Auto-generate platform SSH keypair if not yet present
    let existing_pub = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'ssh_public_key'"#
    )
    .fetch_optional(&state.db)
    .await?;

    let (pub_key, priv_key) = if let Some(pk) = existing_pub {
        let priv_key_enc = sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'ssh_private_key'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_default();
        let priv_key = state.crypto.decrypt(&priv_key_enc)?;
        (pk, priv_key)
    } else {
        tracing::info!("Generating platform SSH keypair (first node provision)…");
        let (priv_pem, pub_line) = crate::ssh::generate_keypair()?;
        let priv_enc = state.crypto.encrypt(&priv_pem)?;
        sqlx::query!(
            r#"INSERT INTO platform_config (`key`, `value`) VALUES ('ssh_public_key', ?)
               ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)"#,
            pub_line
        )
        .execute(&state.db)
        .await?;
        sqlx::query!(
            r#"INSERT INTO platform_config (`key`, `value`) VALUES ('ssh_private_key', ?)
               ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)"#,
            priv_enc
        )
        .execute(&state.db)
        .await?;
        (pub_line, priv_pem)
    };

    // Auto-generate k3s_token if cluster doesn't have one yet
    let k3s_token_enc_opt =
        sqlx::query_scalar!(r#"SELECT k3s_token FROM clusters WHERE id = ?"#, cluster_id)
            .fetch_optional(&state.db)
            .await?
            .flatten();

    let k3s_token = if let Some(enc) = k3s_token_enc_opt {
        state.crypto.decrypt(&enc)?
    } else {
        tracing::info!("Generating k3s token for cluster {cluster_id}…");
        let token = uuid::Uuid::new_v4().to_string();
        let token_enc = state.crypto.encrypt(&token)?;
        sqlx::query!(
            r#"UPDATE clusters SET k3s_token = ? WHERE id = ?"#,
            token_enc, cluster_id
        )
        .execute(&state.db)
        .await?;
        token
    };

    let master_ip: String = sqlx::query_scalar!(
        r#"SELECT ip_address AS `ip: String` FROM cluster_nodes
           WHERE cluster_id = ? AND node_role = 'MASTER' AND id != ? LIMIT 1"#,
        cluster_id, node_id
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_default();

    // ── Pre-flight: cluster must have an IP pool with CIDR + gateway ────────
    #[derive(sqlx::FromRow)]
    struct PoolInfo { cidr: Option<String>, gateway: Option<String> }
    let pool_info: Option<PoolInfo> = sqlx::query_as(
        "SELECT p.cidr, p.gateway FROM ip_pools p \
         JOIN clusters c ON c.ip_pool_id = p.id WHERE c.id = ?"
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?;

    let pool_cidr = pool_info.as_ref().and_then(|p| p.cidr.as_deref());
    let pool_gw = pool_info.as_ref().and_then(|p| p.gateway.as_deref());

    if pool_cidr.is_none() || pool_gw.is_none() {
        return Err(AppError::BadRequest(
            "cluster has no IP pool configured (or pool is missing CIDR/gateway). \
             Set ip_pool_id on the cluster before adding nodes."
                .into(),
        ));
    }

    let installer = NodeInstaller::new(
        ip, ssh_password, &pub_key, &priv_key,
        &state.config.ldap.url, &state.config.ldap.base_dn,
    );
    let result = match installer
        .run(
            hostname,
            &master_ip,
            &k3s_token,
            role == "MASTER",
            storage_path,
            pool_cidr,
            pool_gw,
            &logger,
        )
        .await
    {
        Ok(r) => {
            logger.finish_ok().await;
            r
        }
        Err(e) => {
            logger.finish_err(&e.to_string()).await;
            return Err(e);
        }
    };

    // Store main network interface detected on the node
    sqlx::query!(
        r#"UPDATE cluster_nodes SET main_iface = ? WHERE id = ?"#,
        result.main_iface,
        node_id
    )
    .execute(&state.db)
    .await?;

    // Store GPU info detected during provisioning
    if result.has_gpu {
        sqlx::query!(
            r#"UPDATE cluster_nodes SET has_gpu = 1, gpu_model = ?, gpu_count = ? WHERE id = ?"#,
            result.gpu_model,
            result.gpu_count,
            node_id
        )
        .execute(&state.db)
        .await?;
    }

    // Master: persist kubeconfig and configure local-path storage class
    if let Some(kc) = result.kubeconfig {
        let kc_enc = state.crypto.encrypt(&kc)?;
        sqlx::query!(
            r#"UPDATE clusters SET kubeconfig = ? WHERE id = ?"#,
            kc_enc,
            cluster_id
        )
        .execute(&state.db)
        .await?;

        configure_local_path_storage(state, cluster_id, storage_path).await?;

        // Store detected interface as the cluster-level main NIC (first master wins)
        sqlx::query!(
            r#"UPDATE clusters SET node_main_iface = ? WHERE id = ? AND node_main_iface = 'eth0'"#,
            result.main_iface,
            cluster_id
        )
        .execute(&state.db)
        .await?;
    }

    let pod_cidr = wait_for_node_ready(state, cluster_id, hostname).await?;

    sqlx::query!(
        r#"UPDATE cluster_nodes
           SET node_status = 'READY', pod_cidr = ?, last_seen_at = NOW()
           WHERE id = ?"#,
        pod_cidr,
        node_id
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
        .patch(
            "local-path-config",
            &PatchParams::default(),
            &Patch::Merge(patch),
        )
        .await?;

    Ok(())
}

/// Provision a node in a DOCKER-mode cluster.
/// Installs Docker Engine + qs-agent, then polls the agent until it responds.
pub async fn provision_docker_node(
    state: &AppState,
    node_id: &str,
    cluster_id: &str,
    ip: &str,
    hostname: &str,
    ssh_password: &str,
    storage_path: &str,
    agent_port: u16,
) -> AppResult<()> {
    use crate::ssh::NodeInstaller;

    let logger = ProvisionLogger::begin(state.db.clone(), node_id).await?;

    // SSH keypair
    let existing_pub = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'ssh_public_key'"#
    )
    .fetch_optional(&state.db)
    .await?;

    let (pub_key, priv_key) = if let Some(pk) = existing_pub {
        let priv_key_enc = sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'ssh_private_key'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_default();
        let priv_key = state.crypto.decrypt(&priv_key_enc)?;
        (pk, priv_key)
    } else {
        tracing::info!("Generating platform SSH keypair (first node provision)…");
        let (priv_pem, pub_line) = crate::ssh::generate_keypair()?;
        let priv_enc = state.crypto.encrypt(&priv_pem)?;
        sqlx::query!(
            r#"INSERT INTO platform_config (`key`, `value`) VALUES ('ssh_public_key', ?)
               ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)"#,
            pub_line
        )
        .execute(&state.db)
        .await?;
        sqlx::query!(
            r#"INSERT INTO platform_config (`key`, `value`) VALUES ('ssh_private_key', ?)
               ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)"#,
            priv_enc
        )
        .execute(&state.db)
        .await?;
        (pub_line, priv_pem)
    };

    let agent_token: String = sqlx::query_scalar(
        "SELECT `value` FROM platform_config WHERE `key` = 'agent_token'",
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "changeme".to_string());

    let backend_url: String = sqlx::query_scalar(
        "SELECT `value` FROM platform_config WHERE `key` = 'backend_url'",
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "http://127.0.0.1:3001".to_string());

    let agent_url: String = sqlx::query_scalar(
        "SELECT `value` FROM platform_config WHERE `key` = 'agent_binary_url'",
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| format!("{}/static/qs-agent", backend_url));

    let installer = NodeInstaller::new(
        ip, ssh_password, &pub_key, &priv_key,
        &state.config.ldap.url, &state.config.ldap.base_dn,
    );
    let result = match installer
        .run_docker(
            hostname, storage_path, &agent_url,
            agent_port, node_id, &agent_token, &backend_url,
            &logger,
        )
        .await
    {
        Ok(r) => {
            logger.finish_ok().await;
            r
        }
        Err(e) => {
            logger.finish_err(&e.to_string()).await;
            return Err(e);
        }
    };

    // Store NIC + GPU info
    sqlx::query!(
        r#"UPDATE cluster_nodes SET main_iface = ? WHERE id = ?"#,
        result.main_iface, node_id
    )
    .execute(&state.db)
    .await?;

    if result.has_gpu {
        sqlx::query!(
            r#"UPDATE cluster_nodes SET has_gpu = 1, gpu_model = ?, gpu_count = ? WHERE id = ?"#,
            result.gpu_model, result.gpu_count, node_id
        )
        .execute(&state.db)
        .await?;
    }

    // Store detected NIC as cluster-level default
    sqlx::query!(
        r#"UPDATE clusters SET node_main_iface = ? WHERE id = ? AND node_main_iface = 'eth0'"#,
        result.main_iface, cluster_id
    )
    .execute(&state.db)
    .await?;

    // Poll agent until it responds (up to 120s)
    let agent = crate::docker::agent_client::AgentClient::new(&agent_token);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        match agent.health(ip, agent_port).await {
            Ok(status) if status.ok => break,
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Internal(format!(
                "agent on {ip}:{agent_port} did not respond within 120 seconds"
            )));
        }
        sleep(Duration::from_secs(5)).await;
    }

    // Detect CPU / memory from agent status
    if let Ok(status) = agent.health(ip, agent_port).await {
        let cpu_mcores = (status.cpu_count as u32) * 1000;
        let mem_mb = status.mem_total_mb as u32;
        sqlx::query(
            "UPDATE cluster_nodes SET cpu_capacity_mcores = ?, mem_capacity_mb = ? WHERE id = ?",
        )
        .bind(cpu_mcores)
        .bind(mem_mb)
        .bind(node_id)
        .execute(&state.db)
        .await?;
    }

    sqlx::query("UPDATE cluster_nodes SET node_status = 'READY', last_seen_at = NOW() WHERE id = ?")
        .bind(node_id)
        .execute(&state.db)
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
                let ready = node
                    .status
                    .as_ref()
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
            return Err(AppError::Internal(format!(
                "node {hostname} did not become Ready within 5 minutes"
            )));
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
            let ready = node
                .status
                .as_ref()
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
        status,
        pod_cidr,
        node_id
    )
    .execute(&state.db)
    .await?;

    Ok(status)
}

pub async fn drain_and_delete(state: &AppState, cluster_id: &str, hostname: &str) -> AppResult<()> {
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
            && (name_labels.contains("mountpoint=\"/\"")
                || name_labels.contains("mountpoint=\"/ \""))
        {
            disk_total = value;
        } else if name_labels.starts_with("node_filesystem_avail_bytes")
            && (name_labels.contains("mountpoint=\"/\"")
                || name_labels.contains("mountpoint=\"/ \""))
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

    Ok(NodeMetrics {
        cpu_used_pct,
        mem_total_mb,
        mem_used_mb,
        disk_total_gb,
        disk_used_gb,
        load1,
        load5,
        load15,
    })
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
        m.cpu_used_pct,
        m.mem_used_mb as i64,
        m.disk_used_gb as i64,
        m.disk_total_gb as i64,
        m.load1,
        node_id
    )
    .execute(&state.db)
    .await?;

    Ok(m)
}
