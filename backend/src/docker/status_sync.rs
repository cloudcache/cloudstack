//! Background task: polls Docker agents for container health and updates
//! app status (DEPLOYING → RUNNING/FAILED). Called every 30s from main.rs.

use crate::{
    docker::agent_client::AgentClient,
    error::AppResult,
    state::AppState,
};

/// Scan DEPLOYING apps on DOCKER clusters, check container status, update DB.
pub async fn sync_docker_app_statuses(state: &AppState) -> AppResult<()> {
    #[derive(sqlx::FromRow)]
    struct DeployingApp {
        id: String,
        name: String,
        cluster_id: Option<String>,
    }

    let apps: Vec<DeployingApp> = sqlx::query_as(
        r#"SELECT a.id, a.name, a.cluster_id
           FROM apps a
           JOIN clusters c ON c.id = a.cluster_id
           WHERE a.status = 'DEPLOYING' AND c.orchestrator = 'DOCKER'"#,
    )
    .fetch_all(&state.db)
    .await?;

    if apps.is_empty() {
        return Ok(());
    }

    let agent_token = sqlx::query_scalar::<_, String>(
        "SELECT `value` FROM platform_config WHERE `key` = 'agent_token'",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "changeme".to_string());

    let agent = AgentClient::new(&agent_token);

    for app in &apps {
        let Some(ref _cluster_id) = app.cluster_id else {
            continue;
        };

        #[derive(sqlx::FromRow)]
        struct ContainerStatus {
            id: String,
            container_id: String,
            node_id: String,
            status: String,
        }

        let containers: Vec<ContainerStatus> = sqlx::query_as(
            "SELECT id, container_id, node_id, status FROM docker_containers WHERE app_id = ?",
        )
        .bind(&app.id)
        .fetch_all(&state.db)
        .await?;

        if containers.is_empty() {
            let _ = sqlx::query("UPDATE apps SET status = 'FAILED' WHERE id = ? AND status = 'DEPLOYING'")
                .bind(&app.id)
                .execute(&state.db)
                .await;
            continue;
        }

        let mut all_running = true;
        let mut any_failed = false;

        for c in &containers {
            if c.status == "RUNNING" {
                continue;
            }
            if c.status == "FAILED" || c.status == "ERROR" {
                any_failed = true;
                continue;
            }

            #[derive(sqlx::FromRow)]
            struct NodeInfo {
                ip_address: String,
                agent_port: u16,
            }

            let node: Option<NodeInfo> = sqlx::query_as(
                "SELECT ip_address, agent_port FROM cluster_nodes WHERE id = ?",
            )
            .bind(&c.node_id)
            .fetch_optional(&state.db)
            .await?;

            let Some(node) = node else {
                any_failed = true;
                continue;
            };

            match agent.health(&node.ip_address, node.agent_port).await {
                Ok(status) if status.ok => {
                    all_running = false;
                }
                _ => {
                    let _ = sqlx::query(
                        "UPDATE docker_containers SET status = 'FAILED', error_message = 'agent unreachable' WHERE id = ?",
                    )
                    .bind(&c.id)
                    .execute(&state.db)
                    .await;
                    any_failed = true;
                }
            }
        }

        let new_status = if any_failed {
            Some("FAILED")
        } else if all_running {
            Some("RUNNING")
        } else {
            None
        };

        if let Some(status) = new_status {
            sqlx::query("UPDATE apps SET status = ? WHERE id = ? AND status = 'DEPLOYING'")
                .bind(status)
                .bind(&app.id)
                .execute(&state.db)
                .await?;
            tracing::info!(app_id = %app.id, app_name = %app.name, %status, "docker app status updated");
        }
    }

    Ok(())
}
