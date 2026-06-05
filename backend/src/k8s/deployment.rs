use std::collections::BTreeMap;

use base64::Engine as _;
use futures::{SinkExt, StreamExt};
use k8s_openapi::api::{
    apps::v1::{Deployment, DeploymentSpec},
    core::v1::{Secret, Service, ServicePort, ServiceSpec},
    networking::v1::{NetworkPolicy, NetworkPolicySpec},
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::api::{Api, DeleteParams, ListParams, Patch, PatchParams, PostParams};

use crate::{
    error::{AppError, AppResult},
    k8s::pod_spec::{build_pod_spec, AppPodConfig},
    state::AppState,
};

/// Full deploy: create/update Deployment + NodePort Service + NetworkPolicy.
pub async fn deploy_app(
    state: &AppState,
    cluster_id: &str,
    project_id: &str,
    app_id: &str,
    triggered_by: &str,
) -> AppResult<()> {
    // Load full app config including health check and source fields
    let app = sqlx::query!(
        r#"SELECT a.name, a.display_name, a.replicas,
                  a.container_image, a.container_registry_user, a.container_registry_pass,
                  a.container_command, a.container_args, a.working_dir,
                  a.cpu_reservation_mcores, a.cpu_limit_mcores,
                  a.mem_reservation_mb, a.mem_limit_mb,
                  a.run_as_user, a.run_as_group, a.fs_group,
                  a.privileged, a.read_only_root_fs,
                  a.gpu_enabled, a.gpu_count,
                  a.timezone, a.mount_ldap_files, a.mount_etc_hosts,
                  a.mount_user_home, a.mount_app_data, a.mount_app_logs,
                  a.anti_affinity_enabled,
                  a.use_network_policy, a.ingress_network_policy, a.egress_network_policy,
                  a.source_type,
                  a.health_check_type, a.health_check_path, a.health_check_port,
                  a.health_check_scheme, a.health_check_period, a.health_check_timeout,
                  a.health_check_failures,
                  u.username, u.ldap_uid, u.ldap_gid
           FROM apps a
           JOIN users u ON u.id = a.owner_id
           WHERE a.id = ? AND a.project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    let ns = sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, project_id)
        .fetch_one(&state.db)
        .await?;

    // Load + decrypt env vars
    let env_vars = sqlx::query!(
        r#"SELECT key_name, value, is_secret FROM app_env_vars WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    let mut resolved_env: Vec<(String, String)> = Vec::new();
    for e in env_vars {
        let val = if e.is_secret != 0 {
            e.value
                .as_deref()
                .map(|v| state.crypto.decrypt(v))
                .transpose()?
                .unwrap_or_default()
        } else {
            e.value.unwrap_or_default()
        };
        resolved_env.push((e.key_name, val));
    }

    // Try 'storage_root' first (the canonical key set by admin UI),
    // fall back to legacy 'shared_storage_path' for backwards compatibility.
    let storage_root = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'storage_root'"#
    )
    .fetch_optional(&state.db)
    .await?;
    let storage_root = match storage_root {
        Some(v) if !v.is_empty() => v,
        _ => sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'shared_storage_path'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "/storage".to_string()),
    };

    let run_as_uid = app.run_as_user.or(app.ldap_uid.map(|v| v));
    let run_as_gid = app.run_as_group.or(app.ldap_gid.map(|v| v));

    let registry =
        sqlx::query_scalar!(r#"SELECT `value` FROM platform_config WHERE `key` = 'registry_host'"#)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_default();

    let image = if app.source_type == "GIT" {
        format!("{}/{}:{}", registry, app.name, "latest")
    } else {
        app.container_image.clone().unwrap_or_default()
    };

    let container_command: Option<Vec<String>> = app
        .container_command
        .as_deref()
        .map(|s| vec![s.to_string()]);
    let container_args: Option<Vec<String>> = app
        .container_args
        .and_then(|v| serde_json::from_value(v).ok());

    let client = super::client_for_cluster(state, cluster_id).await?;

    // Ensure namespace exists
    super::namespace::ensure_namespace_with_client(client.clone(), &ns).await?;

    // ── Image pull secret (for private registries) ────────────────────────────
    // Determine the registry host: for GIT source use the internal registry,
    // for CONTAINER source use the domain of the image.
    let pull_secret_name = if let (Some(user), Some(enc_pass)) =
        (&app.container_registry_user, &app.container_registry_pass)
    {
        let password = state.crypto.decrypt(enc_pass)?;
        let reg_host = if app.source_type == "GIT" {
            registry.clone()
        } else {
            image.split('/').next().unwrap_or("docker.io").to_string()
        };
        Some(ensure_registry_secret(&client, &ns, app_id, &reg_host, user, &password).await?)
    } else {
        None
    };

    // ── Load file mounts ──────────────────────────────────────────────────────
    let file_mount_rows = sqlx::query!(
        r#"SELECT filename, mount_path, content FROM app_file_mounts WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;
    let file_mounts: Vec<(String, String, String)> = file_mount_rows
        .into_iter()
        .map(|r| (r.filename, r.mount_path, r.content))
        .collect();

    // Create / update the ConfigMap for inline files
    if !file_mounts.is_empty() {
        ensure_file_configmap(&client, &ns, app_id, &file_mounts).await?;
    }

    // ── Load extra volumes ────────────────────────────────────────────────────
    let extra_vol_rows = sqlx::query!(
        r#"SELECT host_path, mount_path, read_only FROM app_extra_volumes WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;
    let extra_volumes: Vec<(String, String, bool)> = extra_vol_rows
        .into_iter()
        .map(|r| (r.host_path, r.mount_path, r.read_only != 0))
        .collect();

    let cfg = AppPodConfig {
        app_id: app_id.to_string(),
        app_name: app.name.clone(),
        project_name: ns.clone(),
        username: app.username.clone(),
        image,
        replicas: app.replicas as i32,
        container_command,
        container_args,
        working_dir: app.working_dir.clone(),
        env_vars: resolved_env,
        cpu_request_mcores: app.cpu_reservation_mcores,
        cpu_limit_mcores: app.cpu_limit_mcores,
        mem_request_mb: app.mem_reservation_mb,
        mem_limit_mb: app.mem_limit_mb,
        run_as_uid,
        run_as_gid,
        fs_group: app.fs_group,
        privileged: app.privileged != 0,
        read_only_root_fs: app.read_only_root_fs != 0,
        gpu_enabled: app.gpu_enabled != 0,
        gpu_count: app.gpu_count,
        timezone: app.timezone.clone(),
        mount_ldap_files: app.mount_ldap_files != 0,
        mount_etc_hosts: app.mount_etc_hosts != 0,
        mount_user_home: app.mount_user_home != 0,
        mount_app_data: app.mount_app_data != 0,
        mount_app_logs: app.mount_app_logs != 0,
        storage_root,
        health_check_type: app.health_check_type.clone(),
        health_check_path: app.health_check_path.clone(),
        health_check_port: app.health_check_port.map(|p| p as u16),
        health_check_scheme: app
            .health_check_scheme
            .clone()
            .unwrap_or_else(|| "HTTP".into()),
        health_check_period: app.health_check_period as u32,
        health_check_timeout: app.health_check_timeout as u32,
        health_check_failures: app.health_check_failures as u32,
        image_pull_secret: pull_secret_name,
        file_mounts,
        extra_volumes,
    };

    let pod_spec = build_pod_spec(&cfg);

    let deploy_api: Api<Deployment> = Api::namespaced(client.clone(), &ns);

    let mut labels = BTreeMap::new();
    labels.insert("qs-app".to_string(), app.name.clone());
    labels.insert(
        "app.kubernetes.io/managed-by".to_string(),
        "quickstack".to_string(),
    );

    let deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(app.name.clone()),
            namespace: Some(ns.clone()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(app.replicas as i32),
            selector: LabelSelector {
                match_labels: Some(labels.clone()),
                ..Default::default()
            },
            template: k8s_openapi::api::core::v1::PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels.clone()),
                    annotations: None,
                    ..Default::default()
                }),
                spec: Some(pod_spec),
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    deploy_api
        .patch(
            &app.name,
            &PatchParams::apply("quickstack"),
            &Patch::Apply(&deployment),
        )
        .await
        .or(deploy_api
            .create(&PostParams::default(), &deployment)
            .await
            .map(|_| deployment.clone()))?;

    // NodePort Service
    ensure_app_service(state, &client, &ns, app_id, &app.name, &labels).await?;

    // NetworkPolicy (only when enabled)
    if app.use_network_policy != 0 {
        ensure_network_policy(
            &client,
            &ns,
            &app.name,
            &app.ingress_network_policy,
            &app.egress_network_policy,
        )
        .await?;
    } else {
        // Remove any existing policy — K8s default is allow-all
        let api: Api<NetworkPolicy> = Api::namespaced(client.clone(), &ns);
        let _ = api
            .delete(&format!("qs-{}", app.name), &DeleteParams::default())
            .await;
    }

    // Mark DEPLOYING; background status_sync will transition to RUNNING/FAILED
    sqlx::query!(
        r#"UPDATE apps SET status = 'DEPLOYING' WHERE id = ?"#,
        app_id
    )
    .execute(&state.db)
    .await?;

    tracing::info!(
        app_id, app_name = %app.name, triggered_by,
        "deploy submitted to K8s"
    );

    Ok(())
}

// ── Inline file ConfigMap ─────────────────────────────────────────────────────

async fn ensure_file_configmap(
    client: &kube::Client,
    ns: &str,
    app_id: &str,
    files: &[(String, String, String)],
) -> AppResult<()> {
    use k8s_openapi::api::core::v1::ConfigMap;

    let name = format!("qs-files-{}", &app_id[..8.min(app_id.len())]);
    let data: BTreeMap<String, String> = files
        .iter()
        .map(|(filename, _mount, content)| (filename.clone(), content.clone()))
        .collect();

    let cm = ConfigMap {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(ns.to_string()),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    };

    let api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);
    api.patch(&name, &PatchParams::apply("quickstack"), &Patch::Apply(&cm))
        .await?;

    Ok(())
}

// ── Registry secret ───────────────────────────────────────────────────────────

async fn ensure_registry_secret(
    client: &kube::Client,
    ns: &str,
    app_id: &str,
    registry: &str,
    username: &str,
    password: &str,
) -> AppResult<String> {
    let secret_name = format!("qs-reg-{}", &app_id[..8]);

    let auth_b64 =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    let docker_cfg = serde_json::json!({
        "auths": {
            registry: { "username": username, "password": password, "auth": auth_b64 }
        }
    });

    let mut data = BTreeMap::new();
    data.insert(
        ".dockerconfigjson".to_string(),
        k8s_openapi::ByteString(docker_cfg.to_string().into_bytes()),
    );

    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(secret_name.clone()),
            namespace: Some(ns.to_string()),
            ..Default::default()
        },
        type_: Some("kubernetes.io/dockerconfigjson".to_string()),
        data: Some(data),
        ..Default::default()
    };

    let api: Api<Secret> = Api::namespaced(client.clone(), ns);
    let _ = api
        .patch(
            &secret_name,
            &PatchParams::apply("quickstack"),
            &Patch::Apply(&secret),
        )
        .await;

    Ok(secret_name)
}

// ── NetworkPolicy ─────────────────────────────────────────────────────────────

async fn ensure_network_policy(
    client: &kube::Client,
    ns: &str,
    app_name: &str,
    ingress_policy: &str,
    egress_policy: &str,
) -> AppResult<()> {
    let api: Api<NetworkPolicy> = Api::namespaced(client.clone(), ns);
    let name = format!("qs-{}", app_name);

    // If both are ALLOW_ALL, no NetworkPolicy needed
    if ingress_policy == "ALLOW_ALL" && egress_policy == "ALLOW_ALL" {
        let _ = api.delete(&name, &DeleteParams::default()).await;
        return Ok(());
    }

    let mut match_labels = BTreeMap::new();
    match_labels.insert("qs-app".to_string(), app_name.to_string());

    use k8s_openapi::api::networking::v1::{
        NetworkPolicyEgressRule, NetworkPolicyIngressRule, NetworkPolicyPeer,
    };

    let mut policy_types = Vec::new();

    // ── Ingress rules ────────────────────────────────────────────────────
    let ingress = match ingress_policy {
        "ALLOW_ALL" => None,
        "NAMESPACE_ONLY" => {
            policy_types.push("Ingress".to_string());
            let mut ns_label = BTreeMap::new();
            ns_label.insert("kubernetes.io/metadata.name".to_string(), ns.to_string());
            Some(vec![NetworkPolicyIngressRule {
                from: Some(vec![NetworkPolicyPeer {
                    namespace_selector: Some(LabelSelector {
                        match_labels: Some(ns_label),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                ports: None,
            }])
        }
        "DENY_ALL" => {
            policy_types.push("Ingress".to_string());
            Some(vec![]) // empty = deny all
        }
        "INTERNET_ONLY" => {
            policy_types.push("Ingress".to_string());
            Some(vec![]) // deny all cluster ingress
        }
        _ => None,
    };

    // ── Egress rules ─────────────────────────────────────────────────────
    let egress = match egress_policy {
        "ALLOW_ALL" => None,
        "NAMESPACE_ONLY" => {
            policy_types.push("Egress".to_string());
            let mut ns_label = BTreeMap::new();
            ns_label.insert("kubernetes.io/metadata.name".to_string(), ns.to_string());
            Some(vec![NetworkPolicyEgressRule {
                to: Some(vec![NetworkPolicyPeer {
                    namespace_selector: Some(LabelSelector {
                        match_labels: Some(ns_label),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                ports: None,
            }])
        }
        "DENY_ALL" => {
            policy_types.push("Egress".to_string());
            Some(vec![]) // empty = deny all
        }
        "INTERNET_ONLY" => {
            // Allow all egress (internet access ok)
            None
        }
        _ => None,
    };

    let np = NetworkPolicy {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(ns.to_string()),
            ..Default::default()
        },
        spec: Some(NetworkPolicySpec {
            pod_selector: LabelSelector {
                match_labels: Some(match_labels),
                ..Default::default()
            },
            ingress,
            egress,
            policy_types: Some(policy_types),
        }),
    };

    api.patch(&name, &PatchParams::apply("quickstack"), &Patch::Apply(&np))
        .await?;

    Ok(())
}

// ── NodePort Service ──────────────────────────────────────────────────────────

async fn ensure_app_service(
    state: &AppState,
    client: &kube::Client,
    ns: &str,
    app_id: &str,
    app_name: &str,
    labels: &BTreeMap<String, String>,
) -> AppResult<()> {
    let ports = sqlx::query!(
        r#"SELECT id, container_port, protocol, nodeport FROM app_ports WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    if ports.is_empty() {
        return Ok(());
    }

    let mut service_ports: Vec<ServicePort> = Vec::new();
    for port in &ports {
        let nodeport = if let Some(np) = port.nodeport {
            np
        } else {
            let np = allocate_nodeport(state).await?;
            sqlx::query!(
                r#"UPDATE app_ports SET nodeport = ? WHERE id = ?"#,
                np,
                port.id
            )
            .execute(&state.db)
            .await?;
            np
        };

        service_ports.push(ServicePort {
            name: Some(format!("port-{}", port.container_port)),
            port: port.container_port as i32,
            node_port: Some(nodeport as i32),
            protocol: Some(port.protocol.clone()),
            ..Default::default()
        });
    }

    let svc_api: Api<Service> = Api::namespaced(client.clone(), ns);
    let service = Service {
        metadata: ObjectMeta {
            name: Some(app_name.to_string()),
            namespace: Some(ns.to_string()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            type_: Some("NodePort".to_string()),
            selector: Some(labels.clone()),
            ports: Some(service_ports),
            ..Default::default()
        }),
        ..Default::default()
    };

    svc_api
        .patch(
            app_name,
            &PatchParams::apply("quickstack"),
            &Patch::Apply(&service),
        )
        .await?;

    Ok(())
}

// ── Other operations ──────────────────────────────────────────────────────────

pub async fn delete_app_resources(
    state: &AppState,
    cluster_id: &str,
    ns: &str,
    app_name: &str,
    app_id: &str,
) -> AppResult<()> {
    use k8s_openapi::api::core::v1::ConfigMap;

    let client = super::client_for_cluster(state, cluster_id).await?;
    let deploy_api: Api<Deployment> = Api::namespaced(client.clone(), ns);
    let svc_api: Api<Service> = Api::namespaced(client.clone(), ns);
    let np_api: Api<NetworkPolicy> = Api::namespaced(client.clone(), ns);
    let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);

    let _ = deploy_api.delete(app_name, &DeleteParams::default()).await;
    let _ = svc_api.delete(app_name, &DeleteParams::default()).await;
    let _ = np_api
        .delete(&format!("qs-{app_name}"), &DeleteParams::default())
        .await;
    let _ = cm_api
        .delete(
            &format!("qs-files-{}", &app_id[..8.min(app_id.len())]),
            &DeleteParams::default(),
        )
        .await;

    Ok(())
}

pub async fn scale_deployment(
    state: &AppState,
    cluster_id: &str,
    ns: &str,
    app_name: &str,
    replicas: i32,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let deploy_api: Api<Deployment> = Api::namespaced(client, ns);
    let patch = serde_json::json!({ "spec": { "replicas": replicas } });
    deploy_api
        .patch(app_name, &PatchParams::default(), &Patch::Merge(patch))
        .await?;
    Ok(())
}

/// NodePort allocation: find lowest unused port in the admin-configurable range.
///
/// Reads from `platform_config`:
///   - `nodeport_range_start` (default 30000)
///   - `nodeport_range_end`   (default 32767)
///   - `nodeport_reserved`    (default "30100", comma-separated list)
pub async fn allocate_nodeport(state: &AppState) -> AppResult<u16> {
    let range_start = load_config_u16(&state.db, "nodeport_range_start", 30000).await;
    let range_end = load_config_u16(&state.db, "nodeport_range_end", 32767).await;
    let reserved = load_config_reserved_ports(&state.db).await;

    let used: std::collections::HashSet<u16> =
        sqlx::query_scalar!(r#"SELECT nodeport FROM app_ports WHERE nodeport IS NOT NULL"#)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .filter_map(|v| v)
            .collect();

    for port in range_start..=range_end {
        if !used.contains(&port) && !reserved.contains(&port) {
            return Ok(port);
        }
    }
    Err(AppError::QuotaExceeded("NodePort range exhausted".into()))
}

async fn load_config_u16(db: &sqlx::MySqlPool, key: &str, default: u16) -> u16 {
    sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = ?"#,
        key
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<u16>().ok())
    .unwrap_or(default)
}

async fn load_config_reserved_ports(db: &sqlx::MySqlPool) -> std::collections::HashSet<u16> {
    sqlx::query_scalar!(r#"SELECT `value` FROM platform_config WHERE `key` = 'nodeport_reserved'"#)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|v| {
            v.split(',')
                .filter_map(|s| s.trim().parse::<u16>().ok())
                .collect()
        })
        .unwrap_or_else(|| [30100u16].into())
}

/// Stream pod logs as SSE events.
pub async fn log_stream(
    state: &AppState,
    cluster_id: &str,
    ns: &str,
    app_name: &str,
) -> AppResult<
    impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>
        + Send
        + 'static,
> {
    use k8s_openapi::api::core::v1::Pod;
    use kube::api::LogParams;

    let client = super::client_for_cluster(state, cluster_id).await?;
    let pod_api: Api<Pod> = Api::namespaced(client, ns);

    let pods = pod_api
        .list(&ListParams::default().labels(&format!("qs-app={app_name}")))
        .await?;

    let pod_name = pods
        .items
        .into_iter()
        .next()
        .and_then(|p| p.metadata.name)
        .ok_or_else(|| AppError::NotFound("no running pods".into()))?;

    use async_stream::stream;
    use futures::io::AsyncBufReadExt;

    let log_reader = pod_api
        .log_stream(
            &pod_name,
            &LogParams {
                follow: true,
                tail_lines: Some(100),
                ..Default::default()
            },
        )
        .await?;

    let sse_stream = stream! {
        let mut lines = log_reader.lines();
        loop {
            match futures::StreamExt::next(&mut lines).await {
                Some(Ok(line)) => {
                    yield Ok::<_, std::convert::Infallible>(
                        axum::response::sse::Event::default().data(line)
                    );
                }
                _ => break,
            }
        }
    };

    Ok(sse_stream)
}

/// Handle a WebSocket terminal session to a pod.
pub async fn handle_terminal(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    cluster_id: String,
    ns: String,
    app_name: String,
) {
    use axum::extract::ws::Message;
    use k8s_openapi::api::core::v1::Pod;

    let client = match super::client_for_cluster(&state, &cluster_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("terminal: k8s client error: {e}");
            return;
        }
    };

    let pod_api: Api<Pod> = Api::namespaced(client, &ns);
    let pods = match pod_api
        .list(&ListParams::default().labels(&format!("qs-app={app_name}")))
        .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("terminal: list pods error: {e}");
            return;
        }
    };

    let pod_name = match pods.items.into_iter().next().and_then(|p| p.metadata.name) {
        Some(n) => n,
        None => {
            tracing::warn!("terminal: no pods for {app_name}");
            return;
        }
    };

    let mut attached = match pod_api
        .exec(
            &pod_name,
            vec!["sh"],
            &kube::api::AttachParams {
                stdin: true,
                stdout: true,
                stderr: true,
                tty: true,
                ..Default::default()
            },
        )
        .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("terminal: exec error: {e}");
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut stdin_writer = attached.stdin().unwrap();
    let mut stdout = attached.stdout().unwrap();

    tokio::select! {
        _ = async {
            while let Some(Ok(msg)) = ws_receiver.next().await {
                if let Message::Binary(data) = msg {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin_writer.write_all(&data).await;
                }
            }
        } => {},
        _ = async {
            use tokio::io::AsyncReadExt;
            let mut buf = vec![0u8; 1024];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => { let _ = ws_sender.send(Message::Binary(buf[..n].to_vec())).await; }
                }
            }
        } => {},
    }
}
