//! Background task: watches K8s Deployments for apps in DEPLOYING state and
//! transitions them to RUNNING, FAILED, or back to STOPPED based on pod health.
//!
//! Called every 30 s from main.rs. Non-fatal — errors are logged and skipped.

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, ListParams};

use crate::{error::AppResult, state::AppState};

/// Scan all DEPLOYING apps, query K8s, update status in DB.
pub async fn sync_app_statuses(state: &AppState) -> AppResult<()> {
    // Find all apps that are in a transitional state across all clusters
    let deploying = sqlx::query!(
        r#"SELECT a.id, a.name, a.project_id, a.cluster_id,
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
        let Some(ref cluster_id) = app.cluster_id else {
            continue;
        };
        let cluster_id: &str = cluster_id.as_str();

        let client = match super::client_for_cluster(state, cluster_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("status_sync: cluster {cluster_id} unreachable: {e}");
                continue;
            }
        };

        let deploy_api: Api<Deployment> = Api::namespaced(client.clone(), &app.ns);
        let deployment = match deploy_api.get_opt(&app.name).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                // Deployment was deleted externally — mark STOPPED
                let _ = sqlx::query!(r#"UPDATE apps SET status = 'STOPPED' WHERE id = ?"#, app.id)
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

            // Record pod IP from CNI when app reaches RUNNING
            if status == "RUNNING" {
                let pod_api: Api<Pod> = Api::namespaced(client.clone(), &app.ns);
                if let Ok(pods) = pod_api
                    .list(&ListParams::default().labels(&format!("qs-app={}", app.name)))
                    .await
                {
                    if let Some(ip) = pods
                        .items
                        .first()
                        .and_then(|p| p.status.as_ref()?.pod_ip.clone())
                    {
                        if let Err(e) =
                            super::network::record_pod_ip(state, &app.id, cluster_id, &ip).await
                        {
                            tracing::warn!(app_id = %app.id, "record_pod_ip: {e}");
                        }
                    }
                }
            }
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
    let ready = status.ready_replicas.unwrap_or(0);

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
