use std::collections::BTreeMap;

// ── Generic time-series point ──────────────────────────────────────────────────

/// A single metric observation, ready to be written to any TSDB backend.
#[derive(Debug, Clone)]
pub struct MetricPoint {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub timestamp: i64,   // Unix seconds (UTC)
    pub value: f64,
}

impl MetricPoint {
    pub fn new(name: &'static str, value: f64, timestamp: i64) -> Self {
        Self { name: name.to_string(), labels: BTreeMap::new(), timestamp, value }
    }

    pub fn label(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.labels.insert(key.to_string(), value.into());
        self
    }
}

/// A named time series: one label-set → [(timestamp, value)].
#[derive(Debug, Clone)]
pub struct MetricSeries {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub points: Vec<(i64, f64)>,   // (unix_secs, value)
}

/// Selector for querying: match by metric name + zero or more label matchers.
#[derive(Debug, Clone, Default)]
pub struct MetricSelector {
    pub name: String,
    pub labels: BTreeMap<String, String>,
}

impl MetricSelector {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), labels: BTreeMap::new() }
    }

    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

// ── Node snapshot ─────────────────────────────────────────────────────────────

/// One complete reading of all node-level metrics, collected at `timestamp`.
#[derive(Debug, Clone, Default)]
pub struct NodeSnapshot {
    pub timestamp: i64,

    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub filesystems: Vec<FilesystemMetrics>,
    pub disks: Vec<DiskIoMetrics>,
    pub networks: Vec<NetworkIoMetrics>,
    pub gpus: Vec<GpuMetrics>,
}

#[derive(Debug, Clone, Default)]
pub struct CpuMetrics {
    /// Percentage of CPU time NOT idle (0–100).
    pub used_pct: f32,
    /// Capacity reported via DB (filled by caller).
    pub capacity_mcores: Option<u32>,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryMetrics {
    pub used_bytes: u64,
    pub total_bytes: u64,
}

impl MemoryMetrics {
    pub fn used_pct(&self) -> f32 {
        if self.total_bytes == 0 { return 0.0; }
        (self.used_bytes as f32 / self.total_bytes as f32) * 100.0
    }
}

/// Disk space per mountpoint.
#[derive(Debug, Clone)]
pub struct FilesystemMetrics {
    pub mountpoint: String,
    pub fstype: String,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

/// Disk I/O rates for one block device (bytes/sec and IOPS since last scrape).
#[derive(Debug, Clone)]
pub struct DiskIoMetrics {
    pub device: String,
    /// Bytes read per second since last scrape.
    pub read_bytes_rate: f64,
    /// Bytes written per second since last scrape.
    pub write_bytes_rate: f64,
    pub read_iops: f64,
    pub write_iops: f64,
}

/// Network I/O rates for one interface (bytes/sec since last scrape).
#[derive(Debug, Clone)]
pub struct NetworkIoMetrics {
    pub iface: String,
    pub rx_bytes_rate: f64,
    pub tx_bytes_rate: f64,
    pub rx_packets_rate: f64,
    pub tx_packets_rate: f64,
    pub rx_errors_rate: f64,
    pub tx_errors_rate: f64,
}

/// Per-GPU metrics (one entry per physical GPU on the node).
#[derive(Debug, Clone)]
pub struct GpuMetrics {
    /// 0-based GPU index on the host.
    pub index: u32,
    pub model: String,
    /// Compute utilization, 0–100.
    pub util_pct: f32,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
}

// ── Previous-reading state for rate computation ────────────────────────────────

/// Stored between scrapes; needed to compute byte/packet rates from counters.
#[derive(Debug, Clone, Default)]
pub struct NodeCounterState {
    pub timestamp: i64,
    /// disk device → (read_bytes_total, write_bytes_total, reads_total, writes_total)
    pub disk: BTreeMap<String, (u64, u64, u64, u64)>,
    /// iface → (rx_bytes_total, tx_bytes_total, rx_pkts_total, tx_pkts_total, rx_errs_total, tx_errs_total)
    pub net: BTreeMap<String, (u64, u64, u64, u64, u64, u64)>,
}

// ── App snapshot ──────────────────────────────────────────────────────────────

/// One complete reading of all metrics for one user application, aggregated
/// across every pod belonging to that app (potentially on different nodes).
#[derive(Debug, Clone, Default)]
pub struct AppSnapshot {
    pub timestamp: i64,

    pub cpu: AppCpuMetrics,
    pub memory: AppMemoryMetrics,
    pub disk_io: AppDiskIoMetrics,
    pub network_io: AppNetworkIoMetrics,
    pub gpu: Option<AppGpuMetrics>,
    pub pods: AppPodCounts,
}

#[derive(Debug, Clone, Default)]
pub struct AppCpuMetrics {
    /// Total milli-cores used, summed across all running pods.
    pub used_mcores: u64,
    /// Configured CPU limit, from DB.
    pub limit_mcores: Option<u32>,
    /// Configured CPU request, from DB.
    pub request_mcores: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct AppMemoryMetrics {
    pub used_bytes: u64,
    pub limit_bytes: Option<u64>,
    pub request_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct AppDiskIoMetrics {
    pub read_bytes_rate: f64,
    pub write_bytes_rate: f64,
}

#[derive(Debug, Clone, Default)]
pub struct AppNetworkIoMetrics {
    pub rx_bytes_rate: f64,
    pub tx_bytes_rate: f64,
}

#[derive(Debug, Clone, Default)]
pub struct AppGpuMetrics {
    /// Aggregate utilisation across all GPU pods.
    pub util_pct: f32,
    pub mem_used_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct AppPodCounts {
    pub running: u32,
    pub pending: u32,
    pub failed: u32,
    pub total: u32,
}

// ── Conversion helpers ────────────────────────────────────────────────────────

impl NodeSnapshot {
    /// Flatten the snapshot into a batch of MetricPoints ready to write to the store.
    pub fn to_metric_points(
        &self,
        node_id: &str,
        cluster_id: &str,
        hostname: &str,
    ) -> Vec<MetricPoint> {
        use crate::metrics::names::*;
        let ts = self.timestamp;
        let mut pts: Vec<MetricPoint> = Vec::with_capacity(64);
        let node_label = |name| {
            MetricPoint::new(name, 0.0, ts)
                .label("node_id", node_id)
                .label("cluster_id", cluster_id)
                .label("hostname", hostname)
        };

        // CPU
        pts.push(node_label(NODE_CPU_USED_PCT).with_value(self.cpu.used_pct as f64));
        pts.push(node_label(NODE_CPU_LOAD1).with_value(self.cpu.load1 as f64));
        pts.push(node_label(NODE_CPU_LOAD5).with_value(self.cpu.load5 as f64));
        pts.push(node_label(NODE_CPU_LOAD15).with_value(self.cpu.load15 as f64));

        // Memory
        pts.push(node_label(NODE_MEM_USED_BYTES).with_value(self.memory.used_bytes as f64));
        pts.push(node_label(NODE_MEM_TOTAL_BYTES).with_value(self.memory.total_bytes as f64));

        // Filesystems
        for fs in &self.filesystems {
            let base = MetricPoint::new(NODE_FS_USED_BYTES, fs.used_bytes as f64, ts)
                .label("node_id", node_id)
                .label("cluster_id", cluster_id)
                .label("hostname", hostname)
                .label("mountpoint", fs.mountpoint.clone());
            pts.push(base);
            pts.push(
                MetricPoint::new(NODE_FS_TOTAL_BYTES, fs.total_bytes as f64, ts)
                    .label("node_id", node_id)
                    .label("cluster_id", cluster_id)
                    .label("hostname", hostname)
                    .label("mountpoint", fs.mountpoint.clone()),
            );
        }

        // Disk I/O
        for disk in &self.disks {
            let base = |name, val| {
                MetricPoint::new(name, val, ts)
                    .label("node_id", node_id)
                    .label("cluster_id", cluster_id)
                    .label("hostname", hostname)
                    .label("device", disk.device.clone())
            };
            pts.push(base(NODE_DISK_READ_BYTES_RATE, disk.read_bytes_rate));
            pts.push(base(NODE_DISK_WRITE_BYTES_RATE, disk.write_bytes_rate));
            pts.push(base(NODE_DISK_READ_IOPS, disk.read_iops));
            pts.push(base(NODE_DISK_WRITE_IOPS, disk.write_iops));
        }

        // Network I/O
        for nic in &self.networks {
            let base = |name, val| {
                MetricPoint::new(name, val, ts)
                    .label("node_id", node_id)
                    .label("cluster_id", cluster_id)
                    .label("hostname", hostname)
                    .label("iface", nic.iface.clone())
            };
            pts.push(base(NODE_NET_RX_BYTES_RATE, nic.rx_bytes_rate));
            pts.push(base(NODE_NET_TX_BYTES_RATE, nic.tx_bytes_rate));
            pts.push(base(NODE_NET_RX_PACKETS_RATE, nic.rx_packets_rate));
            pts.push(base(NODE_NET_TX_PACKETS_RATE, nic.tx_packets_rate));
            pts.push(base(NODE_NET_RX_ERRORS_RATE, nic.rx_errors_rate));
            pts.push(base(NODE_NET_TX_ERRORS_RATE, nic.tx_errors_rate));
        }

        // GPU
        for gpu in &self.gpus {
            let base = |name, val| {
                MetricPoint::new(name, val, ts)
                    .label("node_id", node_id)
                    .label("cluster_id", cluster_id)
                    .label("hostname", hostname)
                    .label("gpu_index", gpu.index.to_string())
            };
            pts.push(base(NODE_GPU_UTIL_PCT, gpu.util_pct as f64));
            pts.push(base(NODE_GPU_MEM_USED_BYTES, gpu.mem_used_bytes as f64));
            pts.push(base(NODE_GPU_MEM_TOTAL_BYTES, gpu.mem_total_bytes as f64));
        }

        pts
    }
}

impl AppSnapshot {
    /// Flatten the snapshot into MetricPoints.
    pub fn to_metric_points(
        &self,
        app_id: &str,
        project_id: &str,
        pool_id: &str,
    ) -> Vec<MetricPoint> {
        use crate::metrics::names::*;
        let ts = self.timestamp;
        let base = |name, val| {
            MetricPoint::new(name, val, ts)
                .label("app_id", app_id)
                .label("project_id", project_id)
                .label("pool_id", pool_id)
        };
        let mut pts = vec![
            base(APP_CPU_USED_MCORES, self.cpu.used_mcores as f64),
            base(APP_MEM_USED_BYTES, self.memory.used_bytes as f64),
            base(APP_DISK_READ_BYTES_RATE, self.disk_io.read_bytes_rate),
            base(APP_DISK_WRITE_BYTES_RATE, self.disk_io.write_bytes_rate),
            base(APP_NET_RX_BYTES_RATE, self.network_io.rx_bytes_rate),
            base(APP_NET_TX_BYTES_RATE, self.network_io.tx_bytes_rate),
            MetricPoint::new(APP_POD_COUNT, self.pods.running as f64, ts)
                .label("app_id", app_id).label("project_id", project_id)
                .label("pool_id", pool_id).label("phase", "running"),
            MetricPoint::new(APP_POD_COUNT, self.pods.pending as f64, ts)
                .label("app_id", app_id).label("project_id", project_id)
                .label("pool_id", pool_id).label("phase", "pending"),
            MetricPoint::new(APP_POD_COUNT, self.pods.failed as f64, ts)
                .label("app_id", app_id).label("project_id", project_id)
                .label("pool_id", pool_id).label("phase", "failed"),
        ];
        if let Some(gpu) = &self.gpu {
            pts.push(base(APP_GPU_UTIL_PCT, gpu.util_pct as f64));
            pts.push(base(APP_GPU_MEM_USED_BYTES, gpu.mem_used_bytes as f64));
        }
        pts
    }
}

// Helper: set value on a MetricPoint (builder shorthand)
impl MetricPoint {
    pub fn with_value(mut self, v: f64) -> Self {
        self.value = v;
        self
    }
}
