pub mod namespace;
pub mod deployment;
pub mod pod_spec;
pub mod node;
pub mod network;
pub mod database;
pub mod status_sync;

use kube::Client;

use crate::{error::{AppError, AppResult}, state::AppState};

pub use status_sync::sync_app_statuses;

/// Returns a K8s client for a specific cluster by cluster_id.
/// Reads the encrypted kubeconfig from the `clusters` table.
pub async fn client_for_cluster(state: &AppState, cluster_id: &str) -> AppResult<Client> {
    let enc = sqlx::query_scalar!(
        r#"SELECT kubeconfig FROM clusters WHERE id = ?"#,
        cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .ok_or_else(|| AppError::Internal(
        format!("cluster {cluster_id} has no kubeconfig — master not yet provisioned")
    ))?;

    let yaml = state.crypto.decrypt(&enc)?;
    build_client_from_yaml(&yaml).await
}

/// Fallback: reads kubeconfig from `platform_config` (legacy single-cluster path)
/// then from the environment / in-cluster config.
pub async fn client(state: &AppState) -> AppResult<Client> {
    let stored = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'kubeconfig'"#
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(enc) = stored {
        let yaml = state.crypto.decrypt(&enc)?;
        return build_client_from_yaml(&yaml).await;
    }

    Client::try_default()
        .await
        .map_err(|e| AppError::Kubernetes(e.into()))
}

async fn build_client_from_yaml(yaml: &str) -> AppResult<Client> {
    let kc = kube::config::Kubeconfig::from_yaml(yaml)
        .map_err(|e| AppError::Internal(format!("kubeconfig parse: {e}")))?;
    let config = kube::Config::from_custom_kubeconfig(kc, &Default::default())
        .await
        .map_err(|e| AppError::Internal(format!("kubeconfig build: {e}")))?;
    Ok(Client::try_from(config)?)
}
