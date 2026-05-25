use k8s_openapi::api::core::v1::Namespace;
use kube::{api::{Api, DeleteParams, ObjectMeta, PostParams}, Client};
use std::collections::BTreeMap;

use crate::{error::AppResult, state::AppState};

pub async fn ensure_namespace(
    state: &AppState,
    cluster_id: &str,
    name: &str,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    ensure_namespace_with_client(client, name).await
}

pub async fn delete_namespace(
    state: &AppState,
    cluster_id: &str,
    name: &str,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let ns_api: Api<Namespace> = Api::all(client);
    let _ = ns_api.delete(name, &DeleteParams::default()).await;
    Ok(())
}

pub async fn ensure_namespace_with_client(client: Client, name: &str) -> AppResult<()> {
    let ns_api: Api<Namespace> = Api::all(client);

    if ns_api.get_opt(name).await?.is_some() {
        return Ok(());
    }

    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/managed-by".to_string(), "quickstack".to_string());

    let ns = Namespace {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            labels: Some(labels),
            ..Default::default()
        },
        ..Default::default()
    };

    ns_api.create(&PostParams::default(), &ns).await?;
    Ok(())
}
