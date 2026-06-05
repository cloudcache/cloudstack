use serde::{Deserialize, Serialize};

// ── Container lifecycle ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RunContainerRequest {
    pub container_name: String,
    pub image: String,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    pub cpu_limit_mcores: Option<u32>,
    pub mem_limit_mb: Option<u32>,
    #[serde(default)]
    pub gpu_count: u8,
    /// Primary Docker network to attach (e.g. "qs-vpc")
    pub network_name: Option<String>,
    /// Fixed IP within the primary network
    pub ip_address: Option<String>,
    /// Additional networks to connect after creation (e.g. public pool)
    #[serde(default)]
    pub extra_networks: Vec<ExtraNetwork>,
    #[serde(default)]
    pub port_bindings: Vec<PortBinding>,
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,
    /// Inline files to write to disk and mount into the container
    #[serde(default)]
    pub file_mounts: Vec<FileMount>,
    pub health_check: Option<HealthCheck>,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    pub user: Option<String>,
    #[serde(default)]
    pub privileged: bool,
    #[serde(default)]
    pub read_only_rootfs: bool,
    pub registry_auth: Option<RegistryAuth>,
}

fn default_restart_policy() -> String {
    "unless-stopped".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ExtraNetwork {
    pub name: String,
    pub ip: String,
}

#[derive(Debug, Deserialize)]
pub struct PortBinding {
    pub container_port: u16,
    pub host_port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

#[derive(Debug, Deserialize)]
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct FileMount {
    pub filename: String,
    pub mount_path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct HealthCheck {
    /// "HTTP", "TCP", or "CMD"
    pub check_type: String,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub cmd: Option<String>,
    #[serde(default = "default_interval")]
    pub interval_secs: u32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

fn default_interval() -> u32 { 30 }
fn default_timeout() -> u32 { 5 }
fn default_retries() -> u32 { 3 }

#[derive(Debug, Deserialize)]
pub struct RegistryAuth {
    pub username: String,
    pub password: String,
    pub server_address: String,
}

#[derive(Debug, Serialize)]
pub struct RunContainerResponse {
    pub container_id: String,
    pub container_name: String,
}

// ── Network ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EnsureNetworkRequest {
    pub name: String,
    pub subnet: String,
    pub gateway: Option<String>,
    /// Linux bridge name on the host (e.g. "br-vpc")
    pub bridge_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EnsureNetworkResponse {
    pub network_id: String,
    pub created: bool,
}

// ── Status ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AgentStatus {
    pub ok: bool,
    pub node_id: String,
    pub docker_version: String,
    pub containers_running: usize,
    pub cpu_count: usize,
    pub mem_total_mb: u64,
}

// ── Container info ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
}
