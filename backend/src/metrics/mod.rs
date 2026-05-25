pub mod collector;
pub mod names;
pub mod store;
pub mod types;

pub use store::{build_store, MetricsConfig, MetricsStore, NullStore, VictoriaMetricsStore};
pub use types::{
    AppDiskIoMetrics, AppGpuMetrics, AppMemoryMetrics, AppNetworkIoMetrics, AppPodCounts,
    AppSnapshot, CpuMetrics, DiskIoMetrics, FilesystemMetrics, GpuMetrics, MemoryMetrics,
    MetricPoint, MetricSelector, MetricSeries, NetworkIoMetrics, NodeSnapshot,
};

use crate::{error::AppResult, state::AppState};

/// Load MetricsConfig from platform_config and build the store.
/// Falls back to NullStore on any error.
pub async fn load_store(state: &AppState) -> Box<dyn MetricsStore> {
    match try_load_store(state).await {
        Ok(s)  => s,
        Err(e) => {
            tracing::warn!("failed to load metrics config: {e} — using NullStore");
            Box::new(NullStore)
        }
    }
}

async fn try_load_store(state: &AppState) -> AppResult<Box<dyn MetricsStore>> {
    let endpoint = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'metrics_endpoint'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_default();

    let token_enc = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'metrics_token'"#
    )
    .fetch_optional(&state.db)
    .await?;

    let token = match token_enc {
        Some(enc) if !enc.is_empty() => Some(state.crypto.decrypt(&enc)?),
        _ => None,
    };

    let interval: u32 = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'metrics_scrape_interval_secs'"#
    )
    .fetch_optional(&state.db)
    .await?
    .and_then(|v| v.parse().ok())
    .unwrap_or(30);

    Ok(build_store(MetricsConfig { endpoint, token, scrape_interval_secs: interval }))
}
