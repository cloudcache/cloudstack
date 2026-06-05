use k8s_openapi::api::core::v1::{
    Container, EnvVar, HostPathVolumeSource, LocalObjectReference, PodSpec,
    Probe, ResourceRequirements, SecurityContext, Toleration, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use std::collections::BTreeMap;

pub struct AppPodConfig {
    pub app_id: String,
    pub app_name: String,
    pub project_name: String,
    pub username: String,
    pub image: String,
    pub replicas: i32,
    pub container_command: Option<Vec<String>>,
    pub container_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env_vars: Vec<(String, String)>,
    pub cpu_request_mcores: Option<u32>,
    pub cpu_limit_mcores: Option<u32>,
    pub mem_request_mb: Option<u32>,
    pub mem_limit_mb: Option<u32>,
    pub run_as_uid: Option<u32>,
    pub run_as_gid: Option<u32>,
    pub fs_group: Option<u32>,
    pub privileged: bool,
    pub read_only_root_fs: bool,
    pub gpu_enabled: bool,
    pub gpu_count: u8,
    pub timezone: String,
    pub mount_ldap_files: bool,
    pub mount_etc_hosts: bool,
    pub mount_user_home: bool,
    pub mount_app_data: bool,
    pub mount_app_logs: bool,
    pub storage_root: String,
    // Health probes
    pub health_check_type: String,       // "HTTP" | "TCP" | "NONE"
    pub health_check_path: Option<String>,
    pub health_check_port: Option<u16>,
    pub health_check_scheme: String,     // "HTTP" | "HTTPS"
    pub health_check_period: u32,
    pub health_check_timeout: u32,
    pub health_check_failures: u32,
    // Registry auth secret name (pre-created K8s Secret in the namespace)
    pub image_pull_secret: Option<String>,
    /// Inline config files mounted via ConfigMap: (filename, mount_path, content)
    pub file_mounts: Vec<(String, String, String)>,
    /// Additional hostPath volumes: (host_path, mount_path, read_only)
    pub extra_volumes: Vec<(String, String, bool)>,
}

/// Build a complete PodSpec with all standard mounts and security context.
pub fn build_pod_spec(cfg: &AppPodConfig) -> PodSpec {
    let mut volumes: Vec<Volume> = Vec::new();
    let mut mounts: Vec<VolumeMount> = Vec::new();
    let mut init_commands: Vec<String> = Vec::new();

    // ─── Timezone ────────────────────────────────────────────────────────────
    volumes.push(host_path_volume("tz", &format!("/usr/share/zoneinfo/{}", cfg.timezone)));
    mounts.push(volume_mount("tz", "/etc/localtime", true));
    volumes.push(host_path_volume("timezone-file", "/etc/timezone"));
    mounts.push(volume_mount("timezone-file", "/etc/timezone", true));

    // ─── LDAP auth files ─────────────────────────────────────────────────────
    if cfg.mount_ldap_files {
        for (name, path) in ldap_file_mounts() {
            volumes.push(host_path_volume(&name, path));
            mounts.push(volume_mount(&name, path, true));
        }
        volumes.push(host_path_volume("nslcd-run", "/var/run/nslcd"));
        mounts.push(VolumeMount {
            name: "nslcd-run".to_string(),
            mount_path: "/var/run/nslcd".to_string(),
            read_only: Some(false),
            ..Default::default()
        });
    }

    // ─── /etc/hosts ──────────────────────────────────────────────────────────
    if cfg.mount_etc_hosts {
        volumes.push(host_path_volume("etc-hosts", "/etc/hosts"));
        mounts.push(volume_mount("etc-hosts", "/etc/hosts", true));
    }

    // ─── User home ───────────────────────────────────────────────────────────
    if cfg.mount_user_home {
        let home_path = format!("{}/{}/home", cfg.storage_root, cfg.username);
        init_commands.push(format!("mkdir -p {home_path}"));
        if let Some(uid) = cfg.run_as_uid {
            init_commands.push(format!("chown {uid}:{} {home_path}", cfg.run_as_gid.unwrap_or(uid)));
        }
        volumes.push(host_path_volume("user-home", &home_path));
        mounts.push(volume_mount("user-home", &format!("/home/{}", cfg.username), false));
    }

    // ─── App data / logs ─────────────────────────────────────────────────────
    if cfg.mount_app_data {
        let data_path = format!("{}/{}/{}/data", cfg.storage_root, cfg.username, cfg.app_name);
        init_commands.push(format!("mkdir -p {data_path}"));
        if let Some(uid) = cfg.run_as_uid {
            init_commands.push(format!("chown {uid}:{} {data_path}", cfg.run_as_gid.unwrap_or(uid)));
        }
        volumes.push(host_path_volume("app-data", &data_path));
        mounts.push(volume_mount("app-data", "/data", false));
    }

    if cfg.mount_app_logs {
        let logs_path = format!("{}/{}/{}/logs", cfg.storage_root, cfg.username, cfg.app_name);
        init_commands.push(format!("mkdir -p {logs_path}"));
        if let Some(uid) = cfg.run_as_uid {
            init_commands.push(format!("chown {uid}:{} {logs_path}", cfg.run_as_gid.unwrap_or(uid)));
        }
        volumes.push(host_path_volume("app-logs", &logs_path));
        mounts.push(volume_mount("app-logs", "/logs", false));
    }

    // ─── Extra hostPath volumes ──────────────────────────────────────────────
    for (i, (host_path, mount_path, read_only)) in cfg.extra_volumes.iter().enumerate() {
        let name = format!("extra-vol-{i}");
        volumes.push(host_path_volume(&name, host_path));
        mounts.push(volume_mount(&name, mount_path, *read_only));
    }

    // ─── Inline file mounts (ConfigMap) ──────────────────────────────────────
    // Group all inline files into a single ConfigMap volume with keyed projections.
    if !cfg.file_mounts.is_empty() {
        use k8s_openapi::api::core::v1::{
            ConfigMapVolumeSource, KeyToPath, ProjectedVolumeSource, VolumeProjection,
        };

        // Each file becomes a separate sub-path mount so directories can differ.
        for (i, (filename, mount_path, _content)) in cfg.file_mounts.iter().enumerate() {
            let key = format!("file-{i}");
            volumes.push(Volume {
                name: key.clone(),
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(format!("qs-files-{}", cfg.app_id)),
                    items: Some(vec![KeyToPath {
                        key: filename.clone(),
                        path: filename.clone(),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            });
            mounts.push(VolumeMount {
                name: key,
                mount_path: format!("{}/{}", mount_path.trim_end_matches('/'), filename),
                read_only: Some(true),
                sub_path: Some(filename.clone()),
                ..Default::default()
            });
        }
    }

    // ─── Init container ──────────────────────────────────────────────────────
    let init_containers = if !init_commands.is_empty() {
        vec![Container {
            name: "qs-init".to_string(),
            image: Some("busybox:1.36".to_string()),
            command: Some(vec!["sh".to_string(), "-c".to_string()]),
            args: Some(vec![init_commands.join(" && ")]),
            volume_mounts: Some(mounts.clone()),
            security_context: Some(SecurityContext {
                run_as_user: Some(0),
                run_as_group: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        }]
    } else {
        vec![]
    };

    // ─── Resources ───────────────────────────────────────────────────────────
    let mut resources = ResourceRequirements::default();
    let mut requests: BTreeMap<String, Quantity> = BTreeMap::new();
    let mut limits: BTreeMap<String, Quantity> = BTreeMap::new();

    if let Some(cpu) = cfg.cpu_request_mcores { requests.insert("cpu".into(), Quantity(format!("{cpu}m"))); }
    if let Some(mem) = cfg.mem_request_mb     { requests.insert("memory".into(), Quantity(format!("{mem}Mi"))); }
    if let Some(cpu) = cfg.cpu_limit_mcores   { limits.insert("cpu".into(), Quantity(format!("{cpu}m"))); }
    if let Some(mem) = cfg.mem_limit_mb       { limits.insert("memory".into(), Quantity(format!("{mem}Mi"))); }
    if cfg.gpu_enabled && cfg.gpu_count > 0   { limits.insert("nvidia.com/gpu".into(), Quantity(cfg.gpu_count.to_string())); }
    if !requests.is_empty() { resources.requests = Some(requests); }
    if !limits.is_empty()   { resources.limits   = Some(limits); }

    // ─── Health probes ───────────────────────────────────────────────────────
    // Readiness: starts checking sooner (10s delay) to gate traffic
    // Liveness:  starts checking later (30s delay) to avoid restarting during startup
    let readiness_probe = build_probe(cfg, 10);
    let liveness_probe  = build_probe(cfg, 30);

    // ─── Security context ────────────────────────────────────────────────────
    let security_ctx = SecurityContext {
        run_as_user:              cfg.run_as_uid.map(|v| v as i64),
        run_as_group:             cfg.run_as_gid.map(|v| v as i64),
        privileged:               if cfg.privileged { Some(true) } else { None },
        read_only_root_filesystem: if cfg.read_only_root_fs { Some(true) } else { None },
        allow_privilege_escalation: if cfg.privileged { None } else { Some(false) },
        ..Default::default()
    };

    let env: Vec<EnvVar> = cfg.env_vars.iter().map(|(k, v)| EnvVar {
        name: k.clone(),
        value: Some(v.clone()),
        ..Default::default()
    }).collect();

    let main_container = Container {
        name: "app".to_string(),
        image: Some(cfg.image.clone()),
        command: cfg.container_command.clone(),
        args: cfg.container_args.clone(),
        working_dir: cfg.working_dir.clone(),
        env: Some(env),
        volume_mounts: Some(mounts),
        resources: Some(resources),
        security_context: Some(security_ctx),
        readiness_probe,
        liveness_probe,
        ..Default::default()
    };

    // ─── Affinity ────────────────────────────────────────────────────────────
    let affinity = build_anti_affinity(&cfg.app_name);

    // ─── GPU scheduling ──────────────────────────────────────────────────────
    // Toleration lets the pod be scheduled on nodes tainted with nvidia.com/gpu:NoSchedule.
    // The nvidia.com/gpu resource request itself drives K8s to pick a GPU-capable node.
    let tolerations = if cfg.gpu_enabled {
        Some(vec![Toleration {
            key:      Some("nvidia.com/gpu".to_string()),
            operator: Some("Exists".to_string()),
            effect:   Some("NoSchedule".to_string()),
            ..Default::default()
        }])
    } else {
        None
    };

    // ─── Image pull secret ───────────────────────────────────────────────────
    let image_pull_secrets = cfg.image_pull_secret.as_ref().map(|name| {
        vec![LocalObjectReference { name: Some(name.clone()) }]
    });

    // ─── Pod security context ────────────────────────────────────────────────
    let pod_security_ctx = k8s_openapi::api::core::v1::PodSecurityContext {
        fs_group: cfg.fs_group.map(|v| v as i64),
        ..Default::default()
    };

    PodSpec {
        init_containers: if init_containers.is_empty() { None } else { Some(init_containers) },
        containers:          vec![main_container],
        volumes:             Some(volumes),
        affinity:            Some(affinity),
        security_context:    Some(pod_security_ctx),
        tolerations,
        image_pull_secrets,
        runtime_class_name:  if cfg.gpu_enabled { Some("nvidia".to_string()) } else { None },
        ..Default::default()
    }
}

// ─── Probe builder ────────────────────────────────────────────────────────────

fn build_probe(cfg: &AppPodConfig, initial_delay_secs: i32) -> Option<Probe> {
    let port = cfg.health_check_port? as i32;
    let period   = cfg.health_check_period   as i32;
    let timeout  = cfg.health_check_timeout  as i32;
    let failures = cfg.health_check_failures as i32;

    let handler = match cfg.health_check_type.as_str() {
        "HTTP" => {
            use k8s_openapi::api::core::v1::HTTPGetAction;
            let mut probe = Probe {
                http_get: Some(HTTPGetAction {
                    path:   Some(cfg.health_check_path.clone().unwrap_or_else(|| "/".into())),
                    port:   IntOrString::Int(port),
                    scheme: Some(cfg.health_check_scheme.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            probe.initial_delay_seconds = Some(initial_delay_secs);
            probe.period_seconds        = Some(period);
            probe.timeout_seconds       = Some(timeout);
            probe.failure_threshold     = Some(failures);
            probe.success_threshold     = Some(1);
            probe
        }
        "TCP" => {
            use k8s_openapi::api::core::v1::TCPSocketAction;
            let mut probe = Probe {
                tcp_socket: Some(TCPSocketAction {
                    port: IntOrString::Int(port),
                    ..Default::default()
                }),
                ..Default::default()
            };
            probe.initial_delay_seconds = Some(initial_delay_secs);
            probe.period_seconds        = Some(period);
            probe.timeout_seconds       = Some(timeout);
            probe.failure_threshold     = Some(failures);
            probe.success_threshold     = Some(1);
            probe
        }
        _ => return None,  // NONE
    };

    Some(handler)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn host_path_volume(name: &str, path: &str) -> Volume {
    Volume {
        name: name.to_string(),
        host_path: Some(HostPathVolumeSource { path: path.to_string(), type_: Some("".to_string()) }),
        ..Default::default()
    }
}

fn volume_mount(name: &str, path: &str, read_only: bool) -> VolumeMount {
    VolumeMount {
        name: name.to_string(),
        mount_path: path.to_string(),
        read_only: Some(read_only),
        ..Default::default()
    }
}

fn ldap_file_mounts() -> Vec<(String, &'static str)> {
    vec![
        ("passwd".into(), "/etc/passwd"),
        ("group".into(), "/etc/group"),
        ("shadow".into(), "/etc/shadow"),
        ("gshadow".into(), "/etc/gshadow"),
        ("pamd".into(), "/etc/pam.d"),
        ("nslcd-conf".into(), "/etc/nslcd.conf"),
        ("nsswitch".into(), "/etc/nsswitch.conf"),
        ("sudoers".into(), "/etc/sudoers"),
    ]
}

fn build_anti_affinity(app_name: &str) -> k8s_openapi::api::core::v1::Affinity {
    use k8s_openapi::api::core::v1::{Affinity, PodAffinityTerm, PodAntiAffinity, WeightedPodAffinityTerm};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;

    let mut match_labels = BTreeMap::new();
    match_labels.insert("qs-app".to_string(), app_name.to_string());

    Affinity {
        pod_anti_affinity: Some(PodAntiAffinity {
            preferred_during_scheduling_ignored_during_execution: Some(vec![
                WeightedPodAffinityTerm {
                    weight: 100,
                    pod_affinity_term: PodAffinityTerm {
                        label_selector: Some(LabelSelector {
                            match_labels: Some(match_labels),
                            ..Default::default()
                        }),
                        topology_key: "kubernetes.io/hostname".to_string(),
                        ..Default::default()
                    },
                }
            ]),
            ..Default::default()
        }),
        ..Default::default()
    }
}

// ─── Pod listing ──────────────────────────────────────────────────────────────

/// List running pods for an app deployment.
/// Returns a JSON array with pod name, phase, node, ready status, and restart count.
pub async fn list_pods_for_app(
    state: &crate::state::AppState,
    cluster_id: &str,
    namespace: &str,
    app_name: &str,
) -> crate::error::AppResult<Vec<serde_json::Value>> {
    use k8s_openapi::api::core::v1::Pod;
    use kube::api::{Api, ListParams};

    let client = crate::k8s::client_for_cluster(state, cluster_id).await?;
    let pod_api: Api<Pod> = Api::namespaced(client, namespace);

    let lp = ListParams::default()
        .labels(&format!("qs-app={app_name}"));

    let pod_list = pod_api.list(&lp).await
        .map_err(|e| crate::error::AppError::Kubernetes(e.into()))?;

    let pods = pod_list.items.into_iter().map(|pod| {
        let name = pod.metadata.name.clone().unwrap_or_default();
        let node = pod.spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .unwrap_or_default();
        let phase = pod.status.as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let container_statuses = pod.status.as_ref()
            .and_then(|s| s.container_statuses.as_ref());
        let ready = container_statuses
            .and_then(|cs| cs.first())
            .map(|c| c.ready)
            .unwrap_or(false);
        let restarts = container_statuses
            .and_then(|cs| cs.first())
            .map(|c| c.restart_count)
            .unwrap_or(0);

        serde_json::json!({
            "name": name,
            "phase": phase,
            "node": node,
            "ready": ready,
            "restarts": restarts,
        })
    }).collect();

    Ok(pods)
}
