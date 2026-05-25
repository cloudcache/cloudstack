//! Background task: watches K8s Deployments for apps in DEPLOYING state and
//! transitions them to RUNNING, FAILED, or back to STOPPED based on pod health.
//!
//! Called every 30 s from main.rs. Non-fatal — errors are logged and skipped.

use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{Api, ListParams};

use crate::{error::AppResult, state::AppState};

/// Scan all DEPLOYING apps, query K8s, update status in DB.
pub async fn sync_app_statuses(state: &AppState) -> AppResult<()> {
    // Find all apps that are in a transitional state across all clusters
    let deploying = sqlx::query!(
        r#"SELECT a.id, a.name, a.project_id, a.pool_id,
                  p.name AS ns
           FROM apps a
           JOIN projects p ON p.id = a.project_id
           WHERE a.status = 'DEPLOYING'"#
    )
    .fetch_all(&state.db)
    .await?;

    if deploying.is_empty() {
        return Ok(());
    }

    for app in deploying {
        let Some(pool_id) = app.pool_id else { continue };

        // Find the active cluster for this pool
        let cluster_id = sqlx::query_scalar!(
            r#"SELECT id FROM clusters WHERE pool_id = ? AND is_active = 1 ORDER BY created_at LIMIT 1"#,
            pool_id
        )
        .fetch_optional(&state.db)
        .await?;

        let Some(cluster_id) = cluster_id else { continue };

        let client = match super::client_for_cluster(state, &cluster_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("status_sync: cluster {cluster_id} unreachable: {e}");
                continue;
            }
        };

        let deploy_api: Api<Deployment> = Api::namespaced(client, &app.ns);
        let deployment = match deploy_api.get_opt(&app.name).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                // Deployment was deleted externally — mark STOPPED
                let _ = sqlx::query!(
                    r#"UPDATE apps SET status = 'STOPPED' WHERE id = ?"#, app.id
                )
                .execute(&state.db)
                .await;
                continue;
            }
            Err(e) => {
                tracing::debug!("status_sync: get deployment {}: {e}", app.name);
                continue;
            }
        };

        let new_status = classify_deployment(&deployment);
        if let Some(status) = new_status {
            sqlx::query!(
                r#"UPDATE apps SET status = ? WHERE id = ? AND status = 'DEPLOYING'"#,
                status,
                app.id
            )
            .execute(&state.db)
            .await?;
            tracing::info!(app_id = %app.id, app_name = %app.name, %status, "app status updated");
        }
    }

    Ok(())
}

/// Maps K8s Deployment conditions to QuickStack app status.
/// Returns None if the deployment is still converging (stay DEPLOYING).
fn classify_deployment(d: &Deployment) -> Option<&'static str> {
    let spec_replicas = d.spec.as_ref()?.replicas.unwrap_or(1);
    let status = d.status.as_ref()?;

    let available = status.available_replicas.unwrap_or(0);
    let ready     = status.ready_replicas.unwrap_or(0);

    // Check for a "ReplicaFailure" or "Progressing=False" condition
    if let Some(conditions) = &status.conditions {
        for cond in conditions {
            if cond.type_ == "ReplicaFailure" && cond.status == "True" {
                return Some("FAILED");
            }
            // Deployment exceeded its progress deadline
            if cond.type_ == "Progressing"
                && cond.status == "False"
                && cond.reason.as_deref() == Some("ProgressDeadlineExceeded")
            {
                return Some("FAILED");
            }
        }
    }

    // All desired replicas available and ready → RUNNING
    if spec_replicas > 0 && available >= spec_replicas && ready >= spec_replicas {
        return Some("RUNNING");
    }

    // Zero-replica deployment (e.g. scaled-down) — shouldn't reach DEPLOYING, but guard it
    if spec_replicas == 0 {
        return Some("STOPPED");
    }

    // Still converging
    None
}
