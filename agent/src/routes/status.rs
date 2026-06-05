use axum::{extract::State, Json};
use bollard::system::Version;

use crate::state::AgentState;
use crate::types::AgentStatus;

/// GET /status — agent health and basic node info.
pub async fn health(State(state): State<AgentState>) -> Json<AgentStatus> {
    let docker_version = state
        .docker
        .version()
        .await
        .ok()
        .and_then(|v: Version| v.version)
        .unwrap_or_else(|| "unknown".into());

    let running = state
        .docker
        .list_containers(Some(bollard::container::ListContainersOptions::<String> {
            ..Default::default()
        }))
        .await
        .map(|c| c.len())
        .unwrap_or(0);

    // Read system info for CPU/mem (best-effort)
    let info = state.docker.info().await.ok();
    let cpu_count = info.as_ref().and_then(|i| i.ncpu).unwrap_or(0) as usize;
    let mem_total_mb = info
        .as_ref()
        .and_then(|i| i.mem_total)
        .map(|m| m as u64 / 1024 / 1024)
        .unwrap_or(0);

    Json(AgentStatus {
        ok: true,
        node_id: state.node_id.clone(),
        docker_version,
        containers_running: running,
        cpu_count,
        mem_total_mb,
    })
}
