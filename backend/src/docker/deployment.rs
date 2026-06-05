//! Docker deployment lifecycle: deploy, scale, delete, logs, terminal.
//!
//! Mirrors `k8s::deployment` but targets Docker nodes via agent HTTP calls
//! instead of the Kubernetes API.

use std::convert::Infallible;

use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::{
    docker::agent_client::{
        AgentClient, EnsureNetworkRequest, ExtraNetwork, FileMount, HealthCheck, PortBinding,
        RegistryAuth, RunContainerRequest, VolumeMount,
    },
    error::{AppError, AppResult},
    state::AppState,
};

// ── Row structs for sqlx::query_as (non-macro, offline-compatible) ───────────

#[derive(sqlx::FromRow)]
struct AppRow {
    name: String,
    replicas: u8,
    container_image: Option<String>,
    container_registry_user: Option<String>,
    container_registry_pass: Option<String>,
    container_command: Option<String>,
    container_args: Option<serde_json::Value>,
    working_dir: Option<String>,
    cpu_limit_mcores: Option<i32>,
    mem_limit_mb: Option<i32>,
    run_as_user: Option<i64>,
    privileged: i8,
    read_only_root_fs: i8,
    gpu_enabled: i8,
    gpu_count: Option<i32>,
    timezone: Option<String>,
    mount_ldap_files: i8,
    mount_user_home: i8,
    mount_app_data: i8,
    mount_app_logs: i8,
    health_check_type: Option<String>,
    health_check_path: Option<String>,
    health_check_port: Option<i32>,
    health_check_period: i32,
    health_check_timeout: i32,
    health_check_failures: i32,
    source_type: String,
    username: String,
    project_id: String,
}

#[derive(sqlx::FromRow)]
struct EnvRow {
    key_name: String,
    value: Option<String>,
    is_secret: i8,
}

#[derive(sqlx::FromRow)]
struct ExtraVolRow {
    host_path: String,
    mount_path: String,
    read_only: i8,
}

#[derive(sqlx::FromRow)]
struct FileMountRow {
    filename: String,
    mount_path: String,
    content: String,
}

#[derive(sqlx::FromRow)]
struct PortRow {
    id: String,
    container_port: i32,
    protocol: String,
    nodeport: Option<u16>,
}

#[derive(sqlx::FromRow)]
struct ContainerRow {
    id: String,
    container_id: String,
    node_id: String,
}

#[derive(sqlx::FromRow)]
struct NodeInfo {
    ip_address: String,
    agent_port: u16,
}

#[derive(sqlx::FromRow)]
struct ContainerNode {
    container_id: String,
    ip_address: String,
    agent_port: u16,
}

#[derive(sqlx::FromRow)]
struct PoolInfo {
    cidr: String,
    gateway: Option<String>,
}

/// Load agent_token from platform_config (non-macro).
async fn load_agent_token(db: &sqlx::MySqlPool) -> String {
    sqlx::query_scalar::<_, String>("SELECT `value` FROM platform_config WHERE `key` = 'agent_token'")
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "changeme".to_string())
}

/// Load a platform_config string value.
async fn load_config_value(db: &sqlx::MySqlPool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT `value` FROM platform_config WHERE `key` = ?")
        .bind(key)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
}

/// Full deploy: schedule containers on Docker nodes, push via agent API.
pub async fn deploy_app(
    state: &AppState,
    cluster_id: &str,
    project_id: &str,
    app_id: &str,
    _triggered_by: &str,
) -> AppResult<()> {
    let app: AppRow = sqlx::query_as(
        r#"SELECT a.name, a.replicas,
                  a.container_image, a.container_registry_user, a.container_registry_pass,
                  a.container_command, a.container_args, a.working_dir,
                  a.cpu_limit_mcores, a.mem_limit_mb,
                  a.run_as_user,
                  a.privileged, a.read_only_root_fs,
                  a.gpu_enabled, a.gpu_count,
                  a.timezone, a.mount_ldap_files,
                  a.mount_user_home, a.mount_app_data, a.mount_app_logs,
                  a.health_check_type, a.health_check_path, a.health_check_port,
                  a.health_check_period, a.health_check_timeout, a.health_check_failures,
                  a.source_type, a.project_id,
                  u.username
           FROM apps a
           JOIN users u ON u.id = a.owner_id
           WHERE a.id = ? AND a.project_id = ?"#,
    )
    .bind(app_id)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    // ── Resolve image ────────────────────────────────────────────────────────
    let registry = load_config_value(&state.db, "registry_host")
        .await
        .unwrap_or_default();

    let image = if app.source_type == "GIT" {
        format!("{}/{}:{}", registry, app.name, "latest")
    } else {
        app.container_image.clone().unwrap_or_default()
    };

    // ── Env vars ─────────────────────────────────────────────────────────────
    let env_rows: Vec<EnvRow> = sqlx::query_as(
        "SELECT key_name, value, is_secret FROM app_env_vars WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    let mut env: Vec<(String, String)> = Vec::new();
    for e in env_rows {
        let val = if e.is_secret != 0 {
            e.value
                .as_deref()
                .map(|v| state.crypto.decrypt(v))
                .transpose()?
                .unwrap_or_default()
        } else {
            e.value.unwrap_or_default()
        };
        env.push((e.key_name, val));
    }

    if let Some(ref tz) = app.timezone {
        if !tz.is_empty() {
            env.push(("TZ".to_string(), tz.clone()));
        }
    }

    // ── Storage root ─────────────────────────────────────────────────────────
    let storage_root = match load_config_value(&state.db, "storage_root").await {
        Some(v) if !v.is_empty() => v,
        _ => load_config_value(&state.db, "shared_storage_path")
            .await
            .unwrap_or_else(|| "/storage".to_string()),
    };

    // ── Volumes ──────────────────────────────────────────────────────────────
    let extra_vol_rows: Vec<ExtraVolRow> = sqlx::query_as(
        "SELECT host_path, mount_path, read_only FROM app_extra_volumes WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    let mut volumes: Vec<VolumeMount> = extra_vol_rows
        .into_iter()
        .map(|r| VolumeMount {
            host_path: r.host_path,
            container_path: r.mount_path,
            read_only: r.read_only != 0,
        })
        .collect();

    if app.mount_user_home != 0 {
        volumes.push(VolumeMount {
            host_path: format!("{}/home/{}", storage_root, app.username),
            container_path: format!("/home/{}", app.username),
            read_only: false,
        });
    }
    if app.mount_app_data != 0 {
        volumes.push(VolumeMount {
            host_path: format!("{}/appdata/{}", storage_root, app.name),
            container_path: "/data".to_string(),
            read_only: false,
        });
    }
    if app.mount_app_logs != 0 {
        volumes.push(VolumeMount {
            host_path: format!("{}/logs/{}", storage_root, app.name),
            container_path: "/var/log/app".to_string(),
            read_only: false,
        });
    }
    if app.mount_ldap_files != 0 {
        for (host, container) in &[
            ("/etc/nslcd.conf", "/etc/nslcd.conf"),
            ("/etc/nsswitch.conf", "/etc/nsswitch.conf"),
            ("/etc/passwd", "/etc/passwd"),
            ("/etc/group", "/etc/group"),
        ] {
            volumes.push(VolumeMount {
                host_path: host.to_string(),
                container_path: container.to_string(),
                read_only: true,
            });
        }
    }

    // ── File mounts ──────────────────────────────────────────────────────────
    let file_mount_rows: Vec<FileMountRow> = sqlx::query_as(
        "SELECT filename, mount_path, content FROM app_file_mounts WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;
    let file_mounts: Vec<FileMount> = file_mount_rows
        .into_iter()
        .map(|r| FileMount {
            filename: r.filename,
            mount_path: r.mount_path,
            content: r.content,
        })
        .collect();

    // ── Ports ────────────────────────────────────────────────────────────────
    let port_rows: Vec<PortRow> = sqlx::query_as(
        "SELECT id, container_port, protocol, nodeport FROM app_ports WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    let mut port_bindings: Vec<PortBinding> = Vec::new();
    for p in &port_rows {
        let host_port = if let Some(np) = p.nodeport {
            np
        } else {
            let np = crate::k8s::deployment::allocate_nodeport(state).await?;
            sqlx::query("UPDATE app_ports SET nodeport = ? WHERE id = ?")
                .bind(np)
                .bind(&p.id)
                .execute(&state.db)
                .await?;
            np
        };
        port_bindings.push(PortBinding {
            container_port: p.container_port as u16,
            host_port,
            protocol: p.protocol.clone(),
        });
    }

    // ── Health check ─────────────────────────────────────────────────────────
    let health_check = app.health_check_type.as_deref().and_then(|t| {
        if t == "NONE" || t.is_empty() {
            return None;
        }
        Some(HealthCheck {
            check_type: t.to_string(),
            port: app.health_check_port.map(|p| p as u16),
            path: app.health_check_path.clone(),
            cmd: None,
            interval_secs: app.health_check_period as u32,
            timeout_secs: app.health_check_timeout as u32,
            retries: app.health_check_failures as u32,
        })
    });

    // ── Registry auth ────────────────────────────────────────────────────────
    let registry_auth = if let (Some(user), Some(enc_pass)) =
        (&app.container_registry_user, &app.container_registry_pass)
    {
        let password = state.crypto.decrypt(enc_pass)?;
        let server = if app.source_type == "GIT" {
            registry.clone()
        } else {
            image.split('/').next().unwrap_or("docker.io").to_string()
        };
        Some(RegistryAuth {
            username: user.clone(),
            password,
            server_address: server,
        })
    } else {
        None
    };

    // ── Container command ────────────────────────────────────────────────────
    let container_command: Option<Vec<String>> = app
        .container_command
        .as_deref()
        .map(|s| vec![s.to_string()]);
    let container_args: Option<Vec<String>> = app
        .container_args
        .and_then(|v| serde_json::from_value(v).ok());

    // ── Network + IPAM ───────────────────────────────────────────────────────
    // Docker mode: pre-allocate IP from the cluster's single flat pool
    #[derive(sqlx::FromRow)]
    struct ClusterPool { ip_pool_id: Option<String>, pool_name: Option<String> }
    let cp: Option<ClusterPool> = sqlx::query_as(
        "SELECT c.ip_pool_id, p.name AS pool_name \
         FROM clusters c LEFT JOIN ip_pools p ON p.id = c.ip_pool_id \
         WHERE c.id = ?"
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?;

    let (primary_net_name, primary_ip) = if let Some(ref pool_id) = cp.as_ref().and_then(|c| c.ip_pool_id.clone()) {
        let ip = crate::k8s::network::allocate_ip_for_docker(state, app_id, pool_id).await?;
        let net_name = cp.as_ref()
            .and_then(|c| c.pool_name.as_deref())
            .map(|n| format!("qs-{}", n.replace('_', "-").to_lowercase()))
            .unwrap_or_else(|| "qs-default".to_string());
        (Some(net_name), Some(ip))
    } else {
        (None, None)
    };

    let extra_networks: Vec<ExtraNetwork> = Vec::new();

    // ── Agent ────────────────────────────────────────────────────────────────
    let agent_token = load_agent_token(&state.db).await;
    let agent = AgentClient::new(&agent_token);

    // ── Schedule nodes ───────────────────────────────────────────────────────
    let gpu_needed = app.gpu_enabled != 0;
    let gpu_count = app.gpu_count.unwrap_or(0) as u8;
    let targets = super::scheduler::pick_nodes(
        state,
        cluster_id,
        app.replicas as u32,
        app.cpu_limit_mcores.map(|v| v as u32),
        app.mem_limit_mb.map(|v| v as u32),
        gpu_needed,
        gpu_count,
    )
    .await?;

    // ── Remove existing containers (redeploy) ────────────────────────────────
    delete_app_containers(state, &agent, app_id).await?;

    let user = app.run_as_user.map(|u| u.to_string());

    // ── Create containers ────────────────────────────────────────────────────
    for (idx, target) in targets.iter().enumerate() {
        let container_name = format!("qs-{}-{}", app.name, idx);

        // Ensure Docker networks exist on this node
        if let Some(ref net) = primary_net_name {
            ensure_docker_network(state, &agent, &target.ip_address, target.agent_port, cluster_id, net).await?;
        }
        for extra in &extra_networks {
            ensure_docker_network(state, &agent, &target.ip_address, target.agent_port, cluster_id, &extra.name).await?;
        }

        let req = RunContainerRequest {
            container_name: container_name.clone(),
            image: image.clone(),
            command: container_command.clone(),
            args: container_args.clone(),
            working_dir: app.working_dir.clone(),
            env: env.clone(),
            cpu_limit_mcores: app.cpu_limit_mcores.map(|v| v as u32),
            mem_limit_mb: app.mem_limit_mb.map(|v| v as u32),
            gpu_count,
            network_name: primary_net_name.clone(),
            ip_address: primary_ip.clone(),
            extra_networks: extra_networks.clone(),
            port_bindings: port_bindings.clone(),
            volumes: volumes.clone(),
            file_mounts: file_mounts.clone(),
            health_check: health_check.clone(),
            restart_policy: "unless-stopped".to_string(),
            user: user.clone(),
            privileged: app.privileged != 0,
            read_only_rootfs: app.read_only_root_fs != 0,
            registry_auth: registry_auth.clone(),
        };

        let resp = agent.run_container(&target.ip_address, target.agent_port, &req).await?;

        // Build host_port_map JSON
        let host_port_map: serde_json::Value = port_bindings
            .iter()
            .map(|pb| {
                (
                    format!("{}/{}", pb.container_port, pb.protocol),
                    serde_json::json!(pb.host_port),
                )
            })
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let dc_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO docker_containers
               (id, app_id, node_id, container_id, container_name, image, status, ip_address, host_port_map)
               VALUES (?, ?, ?, ?, ?, ?, 'RUNNING', ?, ?)"#,
        )
        .bind(&dc_id)
        .bind(app_id)
        .bind(&target.node_id)
        .bind(&resp.container_id)
        .bind(&resp.container_name)
        .bind(&image)
        .bind(&primary_ip)
        .bind(host_port_map.to_string())
        .execute(&state.db)
        .await?;
    }

    sqlx::query("UPDATE apps SET status = 'DEPLOYING' WHERE id = ?")
        .bind(app_id)
        .execute(&state.db)
        .await?;

    tracing::info!(app_id, app_name = %app.name, "docker deploy submitted");
    Ok(())
}

/// Ensure a Docker network exists on a specific node.
async fn ensure_docker_network(
    state: &AppState,
    agent: &AgentClient,
    node_ip: &str,
    agent_port: u16,
    cluster_id: &str,
    net_name: &str,
) -> AppResult<()> {
    let pool: Option<PoolInfo> = sqlx::query_as(
        r#"SELECT p.cidr, p.gateway
           FROM ip_pools p
           JOIN clusters c ON c.ip_pool_id = p.id
           WHERE c.id = ?"#,
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?;

    let pool_name = net_name.trim_start_matches("qs-");

    let Some(pool) = pool else {
        return Ok(());
    };

    let bridge_name = format!("br-{}", pool_name.replace('_', "-").to_lowercase());

    agent
        .ensure_network(
            node_ip,
            agent_port,
            &EnsureNetworkRequest {
                name: net_name.to_string(),
                subnet: pool.cidr,
                gateway: pool.gateway,
                bridge_name: Some(bridge_name),
            },
        )
        .await
}

/// Delete all Docker containers for an app.
pub async fn delete_app_resources(
    state: &AppState,
    _cluster_id: &str,
    _ns: &str,
    _app_name: &str,
    app_id: &str,
) -> AppResult<()> {
    let agent_token = load_agent_token(&state.db).await;
    let agent = AgentClient::new(&agent_token);
    delete_app_containers(state, &agent, app_id).await
}

/// Internal helper: stop + remove all containers for an app, clean DB rows.
async fn delete_app_containers(
    state: &AppState,
    agent: &AgentClient,
    app_id: &str,
) -> AppResult<()> {
    let containers: Vec<ContainerRow> = sqlx::query_as(
        "SELECT id, container_id, node_id FROM docker_containers WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    for c in &containers {
        let node: Option<NodeInfo> = sqlx::query_as(
            "SELECT ip_address, agent_port FROM cluster_nodes WHERE id = ?",
        )
        .bind(&c.node_id)
        .fetch_optional(&state.db)
        .await?;

        if let Some(node) = node {
            let _ = agent
                .stop_container(&node.ip_address, node.agent_port, &c.container_id)
                .await;
            let _ = agent
                .remove_container(&node.ip_address, node.agent_port, &c.container_id)
                .await;
        }

        sqlx::query("DELETE FROM docker_containers WHERE id = ?")
            .bind(&c.id)
            .execute(&state.db)
            .await?;
    }

    Ok(())
}

/// Scale Docker containers up or down.
pub async fn scale_deployment(
    state: &AppState,
    cluster_id: &str,
    _ns: &str,
    app_name: &str,
    app_id: &str,
    replicas: i32,
) -> AppResult<()> {
    let existing: Vec<ContainerRow> = sqlx::query_as(
        "SELECT id, container_id, node_id FROM docker_containers WHERE app_id = ? ORDER BY created_at ASC",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    let current = existing.len() as i32;

    let agent_token = load_agent_token(&state.db).await;
    let agent = AgentClient::new(&agent_token);

    if replicas < current {
        // Scale down — remove excess containers (highest index first)
        let to_remove = &existing[replicas as usize..];
        for c in to_remove.iter().rev() {
            if let Some(node) = sqlx::query_as::<_, NodeInfo>(
                "SELECT ip_address, agent_port FROM cluster_nodes WHERE id = ?",
            )
            .bind(&c.node_id)
            .fetch_optional(&state.db)
            .await?
            {
                let _ = agent
                    .stop_container(&node.ip_address, node.agent_port, &c.container_id)
                    .await;
                let _ = agent
                    .remove_container(&node.ip_address, node.agent_port, &c.container_id)
                    .await;
            }
            sqlx::query("DELETE FROM docker_containers WHERE id = ?")
                .bind(&c.id)
                .execute(&state.db)
                .await?;
        }
    } else if replicas > current {
        // Scale up — redeploy with the new replica count (already set in DB by caller)
        let project_id: String = sqlx::query_scalar::<_, String>(
            "SELECT project_id FROM apps WHERE id = ?",
        )
        .bind(app_id)
        .fetch_one(&state.db)
        .await?;

        deploy_app(state, cluster_id, &project_id, app_id, "scale-up").await?;
    }

    Ok(())
}

/// Stream container logs from a Docker node agent as SSE.
pub async fn log_stream(
    state: &AppState,
    _cluster_id: &str,
    _ns: &str,
    _app_name: &str,
    app_id: &str,
) -> AppResult<
    impl futures::Stream<Item = Result<axum::response::sse::Event, Infallible>> + Send + 'static,
> {
    let cn: ContainerNode = sqlx::query_as(
        r#"SELECT dc.container_id, n.ip_address, n.agent_port
           FROM docker_containers dc
           JOIN cluster_nodes n ON n.id = dc.node_id
           WHERE dc.app_id = ?
           ORDER BY dc.created_at ASC LIMIT 1"#,
    )
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("no running containers".into()))?;

    let agent_token = load_agent_token(&state.db).await;

    // Proxy SSE from agent to client via reqwest streaming
    let url = format!(
        "http://{}:{}/containers/{}/logs?tail=100&follow=true",
        cn.ip_address, cn.agent_port, cn.container_id
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(&agent_token)
        .send()
        .await
        .map_err(|e| AppError::Docker(format!("log stream: {e}")))?;

    let byte_stream = resp.bytes_stream();

    use async_stream::stream;
    let sse_stream = stream! {
        futures::pin_mut!(byte_stream);
        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data:") {
                            yield Ok::<_, Infallible>(
                                axum::response::sse::Event::default().data(data.trim()),
                            );
                        }
                    }
                }
                Err(_) => break,
            }
        }
    };

    Ok(sse_stream)
}

/// Proxy a WebSocket terminal session to a Docker container via agent.
pub async fn handle_terminal(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    _cluster_id: String,
    _ns: String,
    _app_name: String,
    app_id: String,
) {
    use axum::extract::ws::Message;
    use tokio_tungstenite::tungstenite;

    let cn: ContainerNode = match sqlx::query_as(
        r#"SELECT dc.container_id, n.ip_address, n.agent_port
           FROM docker_containers dc
           JOIN cluster_nodes n ON n.id = dc.node_id
           WHERE dc.app_id = ?
           ORDER BY dc.created_at ASC LIMIT 1"#,
    )
    .bind(&app_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(r)) => r,
        _ => {
            tracing::error!("terminal: no container for app {app_id}");
            return;
        }
    };

    let agent_token = load_agent_token(&state.db).await;

    let ws_url = format!(
        "ws://{}:{}/containers/{}/exec",
        cn.ip_address, cn.agent_port, cn.container_id
    );

    let request = tungstenite::http::Request::builder()
        .uri(&ws_url)
        .header("Authorization", format!("Bearer {}", agent_token))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .unwrap();

    let (agent_ws, _) = match tokio_tungstenite::connect_async(request).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("terminal: agent ws connect: {e}");
            return;
        }
    };

    let (mut agent_tx, mut agent_rx) = agent_ws.split();
    let (mut client_tx, mut client_rx) = socket.split();

    tokio::select! {
        // Client → Agent
        _ = async {
            while let Some(Ok(msg)) = client_rx.next().await {
                match msg {
                    Message::Binary(data) => {
                        if agent_tx.send(tungstenite::Message::Binary(data.into())).await.is_err() {
                            break;
                        }
                    }
                    Message::Text(text) => {
                        if agent_tx.send(tungstenite::Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        } => {},
        // Agent → Client
        _ = async {
            while let Some(Ok(msg)) = agent_rx.next().await {
                match msg {
                    tungstenite::Message::Binary(data) => {
                        if client_tx.send(Message::Binary(data.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    tungstenite::Message::Text(text) => {
                        if client_tx.send(Message::Text(text.to_string())).await.is_err() {
                            break;
                        }
                    }
                    tungstenite::Message::Close(_) => break,
                    _ => {}
                }
            }
        } => {},
    }
}

/// List containers for a Docker app (equivalent to list_pods for K8s).
pub async fn list_containers(
    state: &AppState,
    app_id: &str,
) -> AppResult<Vec<serde_json::Value>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String,
        container_id: String,
        container_name: String,
        node_id: String,
        image: String,
        status: String,
        ip_address: Option<String>,
        host_port_map: Option<String>,
        created_at: chrono::NaiveDateTime,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT id, container_id, container_name, node_id, image, status,
                  ip_address, host_port_map, created_at
           FROM docker_containers WHERE app_id = ?
           ORDER BY created_at ASC"#,
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "container_id": r.container_id,
                "name": r.container_name,
                "node_id": r.node_id,
                "image": r.image,
                "status": r.status,
                "ip_address": r.ip_address,
                "host_port_map": r.host_port_map.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "created_at": r.created_at.to_string(),
            })
        })
        .collect();

    Ok(result)
}
