//! Translates agent request types into bollard Docker API calls.

use std::collections::HashMap;

use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::models::{
    DeviceRequest, HostConfig, HealthConfig, PortBinding as DockerPortBinding, RestartPolicy,
    RestartPolicyNameEnum,
};
use bollard::Docker;

use crate::types::{RunContainerRequest, RunContainerResponse};

/// Pull the image (if needed) and create+start a container from the request spec.
pub async fn run_container(
    docker: &Docker,
    req: &RunContainerRequest,
    files_dir: &str,
) -> anyhow::Result<RunContainerResponse> {
    // ── Pull image ───────────────────────────────────────────────────────
    let credentials = req.registry_auth.as_ref().map(|auth| {
        bollard::auth::DockerCredentials {
            username: Some(auth.username.clone()),
            password: Some(auth.password.clone()),
            serveraddress: Some(auth.server_address.clone()),
            ..Default::default()
        }
    });

    use bollard::image::CreateImageOptions;
    use futures::StreamExt;

    let mut pull = docker.create_image(
        Some(CreateImageOptions {
            from_image: req.image.clone(),
            ..Default::default()
        }),
        None,
        credentials,
    );
    while let Some(info) = pull.next().await {
        if let Err(e) = info {
            tracing::warn!("pull warning: {e}");
        }
    }

    // ── Write inline file mounts to disk ─────────────────────────────────
    let mut extra_binds: Vec<String> = Vec::new();
    if !req.file_mounts.is_empty() {
        let dir = format!("{}/{}", files_dir, req.container_name);
        tokio::fs::create_dir_all(&dir).await?;
        for fm in &req.file_mounts {
            let host_file = format!("{}/{}", dir, fm.filename);
            tokio::fs::write(&host_file, &fm.content).await?;
            extra_binds.push(format!("{}:{}:ro", host_file, fm.mount_path));
        }
    }

    // ── Build container config ───────────────────────────────────────────
    let mut env: Vec<String> = req
        .env
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();

    // Timezone via TZ env (simpler than volume mount)
    // Callers can pass TZ in env list if needed.

    let mut binds: Vec<String> = req
        .volumes
        .iter()
        .map(|v| {
            if v.read_only {
                format!("{}:{}:ro", v.host_path, v.container_path)
            } else {
                format!("{}:{}", v.host_path, v.container_path)
            }
        })
        .collect();
    binds.extend(extra_binds);

    // Port bindings
    let mut exposed_ports: HashMap<String, HashMap<(), ()>> = HashMap::new();
    let mut port_bindings: HashMap<String, Option<Vec<DockerPortBinding>>> = HashMap::new();
    for pb in &req.port_bindings {
        let key = format!("{}/{}", pb.container_port, pb.protocol);
        exposed_ports.insert(key.clone(), HashMap::new());
        port_bindings.insert(
            key,
            Some(vec![DockerPortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(pb.host_port.to_string()),
            }]),
        );
    }

    // Resource limits
    let nano_cpus = req
        .cpu_limit_mcores
        .map(|m| m as i64 * 1_000_000); // 1 mcore = 1e6 nanocpus
    let memory = req.mem_limit_mb.map(|m| m as i64 * 1024 * 1024);

    // GPU
    let device_requests = if req.gpu_count > 0 {
        Some(vec![DeviceRequest {
            driver: Some("nvidia".to_string()),
            count: Some(req.gpu_count as i64),
            capabilities: Some(vec![vec!["gpu".to_string()]]),
            ..Default::default()
        }])
    } else {
        None
    };

    // Health check
    let healthcheck = req.health_check.as_ref().map(|hc| {
        let test = match hc.check_type.as_str() {
            "HTTP" => {
                let port = hc.port.unwrap_or(80);
                let path = hc.path.as_deref().unwrap_or("/");
                vec![
                    "CMD".to_string(),
                    "curl".to_string(),
                    "-sf".to_string(),
                    format!("http://localhost:{}{}", port, path),
                ]
            }
            "TCP" => {
                let port = hc.port.unwrap_or(80);
                vec![
                    "CMD".to_string(),
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("cat < /dev/null > /dev/tcp/localhost/{}", port),
                ]
            }
            _ => {
                // CMD — use the raw command string
                vec![
                    "CMD-SHELL".to_string(),
                    hc.cmd.clone().unwrap_or_else(|| "true".into()),
                ]
            }
        };
        HealthConfig {
            test: Some(test),
            interval: Some(hc.interval_secs as i64 * 1_000_000_000),
            timeout: Some(hc.timeout_secs as i64 * 1_000_000_000),
            retries: Some(hc.retries as i64),
            ..Default::default()
        }
    });

    // Restart policy
    let restart_policy = Some(RestartPolicy {
        name: Some(match req.restart_policy.as_str() {
            "always" => RestartPolicyNameEnum::ALWAYS,
            "on-failure" => RestartPolicyNameEnum::ON_FAILURE,
            "no" => RestartPolicyNameEnum::NO,
            _ => RestartPolicyNameEnum::UNLESS_STOPPED,
        }),
        maximum_retry_count: None,
    });

    let host_config = HostConfig {
        binds: Some(binds),
        port_bindings: Some(port_bindings),
        nano_cpus,
        memory,
        device_requests,
        restart_policy,
        privileged: Some(req.privileged),
        readonly_rootfs: Some(req.read_only_rootfs),
        ..Default::default()
    };

    // Networking config — attach to primary network at creation time
    let networking_config = req.network_name.as_ref().map(|net_name| {
        use bollard::models::{EndpointSettings, EndpointIpamConfig};
        let mut endpoints: HashMap<String, EndpointSettings> = HashMap::new();
        let ipam = req.ip_address.as_ref().map(|ip| EndpointIpamConfig {
            ipv4_address: Some(ip.clone()),
            ..Default::default()
        });
        endpoints.insert(
            net_name.clone(),
            EndpointSettings {
                ipam_config: ipam,
                ..Default::default()
            },
        );
        bollard::container::NetworkingConfig {
            endpoints_config: endpoints,
        }
    });

    let mut cmd = Vec::new();
    if let Some(ref c) = req.command {
        cmd.extend(c.iter().cloned());
    }
    if let Some(ref a) = req.args {
        cmd.extend(a.iter().cloned());
    }
    let cmd = if cmd.is_empty() { None } else { Some(cmd) };

    let config = Config {
        image: Some(req.image.clone()),
        cmd,
        working_dir: req.working_dir.clone(),
        env: Some(env),
        exposed_ports: Some(exposed_ports),
        host_config: Some(host_config),
        networking_config,
        healthcheck,
        user: req.user.clone(),
        ..Default::default()
    };

    let create_opts = CreateContainerOptions {
        name: req.container_name.as_str(),
        platform: None,
    };

    let created = docker.create_container(Some(create_opts), config).await?;
    let container_id = created.id.clone();

    // ── Attach extra networks (multi-network support) ────────────────────
    for extra in &req.extra_networks {
        use bollard::network::ConnectNetworkOptions;
        use bollard::models::{EndpointSettings, EndpointIpamConfig};
        docker
            .connect_network(
                &extra.name,
                ConnectNetworkOptions {
                    container: &container_id,
                    endpoint_config: EndpointSettings {
                        ipam_config: Some(EndpointIpamConfig {
                            ipv4_address: Some(extra.ip.clone()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                },
            )
            .await?;
    }

    // ── Start ────────────────────────────────────────────────────────────
    docker
        .start_container(&container_id, None::<StartContainerOptions<String>>)
        .await?;

    Ok(RunContainerResponse {
        container_id,
        container_name: req.container_name.clone(),
    })
}

pub async fn stop_container(docker: &Docker, container_id: &str) -> anyhow::Result<()> {
    docker
        .stop_container(
            container_id,
            Some(StopContainerOptions { t: 30 }),
        )
        .await?;
    Ok(())
}

pub async fn remove_container(docker: &Docker, container_id: &str) -> anyhow::Result<()> {
    docker
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await?;
    Ok(())
}
