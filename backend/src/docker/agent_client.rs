//! HTTP client for communicating with qs-agent instances on Docker nodes.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub struct AgentClient {
    http: Client,
    token: String,
}

impl AgentClient {
    pub fn new(agent_token: &str) -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            token: agent_token.to_string(),
        }
    }

    fn url(ip: &str, port: u16, path: &str) -> String {
        format!("http://{}:{}{}", ip, port, path)
    }

    /// POST /containers/run
    pub async fn run_container(
        &self,
        node_ip: &str,
        agent_port: u16,
        req: &RunContainerRequest,
    ) -> AppResult<RunContainerResponse> {
        let resp = self
            .http
            .post(Self::url(node_ip, agent_port, "/containers/run"))
            .bearer_auth(&self.token)
            .json(req)
            .send()
            .await
            .map_err(|e| AppError::Docker(format!("agent {node_ip}: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Docker(format!(
                "agent {node_ip} run_container failed: {body}"
            )));
        }

        resp.json()
            .await
            .map_err(|e| AppError::Docker(format!("parse run response: {e}")))
    }

    /// POST /containers/{id}/stop
    pub async fn stop_container(
        &self,
        node_ip: &str,
        agent_port: u16,
        container_id: &str,
    ) -> AppResult<()> {
        let resp = self
            .http
            .post(Self::url(
                node_ip,
                agent_port,
                &format!("/containers/{container_id}/stop"),
            ))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| AppError::Docker(format!("agent {node_ip} stop: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Docker(format!("stop failed: {body}")));
        }
        Ok(())
    }

    /// DELETE /containers/{id}
    pub async fn remove_container(
        &self,
        node_ip: &str,
        agent_port: u16,
        container_id: &str,
    ) -> AppResult<()> {
        let resp = self
            .http
            .delete(Self::url(
                node_ip,
                agent_port,
                &format!("/containers/{container_id}"),
            ))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| AppError::Docker(format!("agent {node_ip} remove: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Docker(format!("remove failed: {body}")));
        }
        Ok(())
    }

    /// POST /networks/ensure
    pub async fn ensure_network(
        &self,
        node_ip: &str,
        agent_port: u16,
        req: &EnsureNetworkRequest,
    ) -> AppResult<()> {
        let resp = self
            .http
            .post(Self::url(node_ip, agent_port, "/networks/ensure"))
            .bearer_auth(&self.token)
            .json(req)
            .send()
            .await
            .map_err(|e| AppError::Docker(format!("agent {node_ip} ensure_network: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Docker(format!("ensure_network failed: {body}")));
        }
        Ok(())
    }

    /// GET /status — check if agent is alive
    pub async fn health(&self, node_ip: &str, agent_port: u16) -> AppResult<AgentStatus> {
        let resp = self
            .http
            .get(Self::url(node_ip, agent_port, "/status"))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::Docker(format!("agent {node_ip} health: {e}")))?;

        resp.json()
            .await
            .map_err(|e| AppError::Docker(format!("parse health: {e}")))
    }
}

// ── Request/Response types (mirroring agent types) ──────────────────────────

#[derive(Debug, Serialize)]
pub struct RunContainerRequest {
    pub container_name: String,
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    pub env: Vec<(String, String)>,
    pub cpu_limit_mcores: Option<u32>,
    pub mem_limit_mb: Option<u32>,
    pub gpu_count: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub extra_networks: Vec<ExtraNetwork>,
    pub port_bindings: Vec<PortBinding>,
    pub volumes: Vec<VolumeMount>,
    pub file_mounts: Vec<FileMount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheck>,
    pub restart_policy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    pub privileged: bool,
    pub read_only_rootfs: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_auth: Option<RegistryAuth>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtraNetwork {
    pub name: String,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortBinding {
    pub container_port: u16,
    pub host_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileMount {
    pub filename: String,
    pub mount_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthCheck {
    pub check_type: String,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub cmd: Option<String>,
    pub interval_secs: u32,
    pub timeout_secs: u32,
    pub retries: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegistryAuth {
    pub username: String,
    pub password: String,
    pub server_address: String,
}

#[derive(Debug, Serialize)]
pub struct EnsureNetworkRequest {
    pub name: String,
    pub subnet: String,
    pub gateway: Option<String>,
    pub bridge_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunContainerResponse {
    pub container_id: String,
    pub container_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentStatus {
    pub ok: bool,
    pub node_id: String,
    pub docker_version: String,
    pub containers_running: usize,
    pub cpu_count: usize,
    pub mem_total_mb: u64,
}
