/// Collector trait and type stubs — not yet implemented.
///
/// Phases:
///   Phase 2: NodeCollector  — scrapes node_exporter for full metrics (disk-IO, net-IO)
///   Phase 3: AppCollector (metrics-server) — K8s metrics API for CPU + mem
///   Phase 4: AppCollector (cAdvisor)       — kubelet cadvisor for disk-IO + net-IO

use async_trait::async_trait;

use crate::error::AppResult;
use super::types::{NodeSnapshot, AppSnapshot};

// ── Collector trait ───────────────────────────────────────────────────────────

/// Collects a single snapshot from an external source.
#[async_trait]
pub trait NodeCollector: Send + Sync {
    /// Scrape the node and return a complete snapshot.
    /// `prev_counters` holds the raw counter values from the last scrape so the
    /// implementation can compute rates (bytes/sec, IOPS).
    async fn collect(
        &self,
        node_ip: &str,
        prev: Option<&NodeCounterCache>,
    ) -> AppResult<(NodeSnapshot, NodeCounterCache)>;
}

#[async_trait]
pub trait AppCollector: Send + Sync {
    /// Collect metrics for one app. `pod_ips` is the list of host node IPs
    /// where the app's pods are scheduled (for cAdvisor scraping).
    async fn collect(
        &self,
        namespace: &str,
        app_name: &str,
        pod_node_ips: &[String],
    ) -> AppResult<AppSnapshot>;
}

// ── Counter cache ─────────────────────────────────────────────────────────────

/// Raw counter values saved between scrapes so rates can be computed as
/// `(current - previous) / elapsed_secs`.
#[derive(Debug, Clone, Default)]
pub struct NodeCounterCache {
    pub timestamp: i64,
    /// device → (read_bytes, write_bytes, reads_completed, writes_completed)
    pub disk: std::collections::BTreeMap<String, (u64, u64, u64, u64)>,
    /// iface → (rx_bytes, tx_bytes, rx_pkts, tx_pkts, rx_errs, tx_errs)
    pub net: std::collections::BTreeMap<String, (u64, u64, u64, u64, u64, u64)>,
}

// ── Collector source registry (planned) ──────────────────────────────────────

/// Specifies which source to use for app-level metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMetricsSource {
    MetricsServer,
    CAdvisor,
}

// ── Scrape task (runs in background) — stub ───────────────────────────────────

/// Background task that periodically collects all node + app metrics and writes
/// them to the MetricsStore. One task per backend server process.
///
/// Not yet implemented. Start stub wired into main.rs in Phase 2.
pub struct ScrapeTask;

impl ScrapeTask {
    /// Spawn the background scrape loop.
    /// `interval_secs` is read from `platform_config.metrics_scrape_interval_secs`.
    pub fn spawn(
        _state: crate::state::AppState,
        _interval_secs: u32,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // Phase 2+: iterate READY cluster nodes, collect node snapshots;
            // iterate running apps, collect app snapshots; write to store.
            tracing::debug!("metrics scrape task placeholder — not yet collecting");
        })
    }
}
