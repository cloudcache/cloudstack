//! Round-robin scheduler that picks Docker nodes for container placement.

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

/// Target node selected by the scheduler.
pub struct NodeTarget {
    pub node_id: String,
    pub ip_address: String,
    pub agent_port: u16,
}

/// Pick `count` nodes from the cluster's READY Docker nodes.
///
/// Strategy: order by fewest running containers (round-robin with load awareness),
/// then filter by resource capacity (CPU, memory, GPU).
pub async fn pick_nodes(
    state: &AppState,
    cluster_id: &str,
    count: u32,
    cpu_mcores: Option<u32>,
    mem_mb: Option<u32>,
    gpu_needed: bool,
    gpu_count: u8,
) -> AppResult<Vec<NodeTarget>> {
    #[derive(sqlx::FromRow)]
    struct CandidateRow {
        id: String,
        ip_address: String,
        agent_port: u16,
        cpu_capacity_mcores: Option<u32>,
        mem_capacity_mb: Option<u32>,
        has_gpu: i8,
        gpu_count: u8,
        running: i64,
    }

    // Left-join docker_containers to count running containers per node.
    let candidates: Vec<CandidateRow> = sqlx::query_as(
        r#"SELECT n.id, n.ip_address, n.agent_port,
                  n.cpu_capacity_mcores, n.mem_capacity_mb,
                  n.has_gpu, n.gpu_count,
                  COALESCE(dc.cnt, 0) AS running
           FROM cluster_nodes n
           LEFT JOIN (
               SELECT node_id, COUNT(*) AS cnt
               FROM docker_containers
               WHERE status IN ('RUNNING','CREATING')
               GROUP BY node_id
           ) dc ON dc.node_id = n.id
           WHERE n.cluster_id = ? AND n.node_status = 'READY'
           ORDER BY running ASC, n.created_at ASC"#,
    )
    .bind(cluster_id)
    .fetch_all(&state.db)
    .await?;

    let mut selected: Vec<NodeTarget> = Vec::new();

    for c in &candidates {
        if selected.len() >= count as usize {
            break;
        }

        // CPU capacity check (skip if node clearly can't fit)
        if let (Some(need), Some(cap)) = (cpu_mcores, c.cpu_capacity_mcores) {
            if cap < need {
                continue;
            }
        }

        // Memory capacity check
        if let (Some(need), Some(cap)) = (mem_mb, c.mem_capacity_mb) {
            if cap < need {
                continue;
            }
        }

        // GPU check
        if gpu_needed && (c.has_gpu == 0 || c.gpu_count < gpu_count) {
            continue;
        }

        selected.push(NodeTarget {
            node_id: c.id.clone(),
            ip_address: c.ip_address.clone(),
            agent_port: c.agent_port,
        });
    }

    if selected.is_empty() {
        return Err(AppError::BadRequest(
            "no eligible Docker nodes found — check node status and resource capacity".into(),
        ));
    }

    // If we couldn't find enough unique nodes, cycle back to least-loaded
    // (allow multiple containers on the same node)
    if (selected.len() as u32) < count && !candidates.is_empty() {
        let mut idx = 0;
        while (selected.len() as u32) < count {
            let c = &candidates[idx % candidates.len()];
            selected.push(NodeTarget {
                node_id: c.id.clone(),
                ip_address: c.ip_address.clone(),
                agent_port: c.agent_port,
            });
            idx += 1;
        }
    }

    Ok(selected)
}
