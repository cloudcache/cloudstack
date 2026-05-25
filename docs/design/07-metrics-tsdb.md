# 07 — Metrics & TSDB

## Goals

1. Collect CPU / memory / disk-space / disk-IO / network-IO / GPU metrics at two levels:
   - **Node** — physical/VM host, sourced from node_exporter (port 9100)
   - **App** — per-user application, sourced from cAdvisor (kubelet) or K8s metrics-server; aggregated across all pods
2. Store time-series data in a configurable TSDB backend (set later via admin config pages).
3. Expose query endpoints for dashboards.

---

## Data Model

### Metric Point (generic TSDB row)

```
name      string          e.g. "qs_node_cpu_used_pct"
labels    map<str, str>   e.g. { node_id, cluster_id, hostname }
timestamp i64             Unix seconds (UTC)
value     f64
```

Multiple points collected at one scrape become a **batch write** to the store.

### Node snapshot (one scrape interval)

| Category | Fields | Source |
|----------|--------|--------|
| CPU | `used_pct`, `load1 / load5 / load15`, `capacity_mcores` | node_exporter |
| Memory | `used_bytes`, `total_bytes` | node_exporter |
| Disk space | per-mountpoint: `used_bytes`, `total_bytes` | node_exporter |
| Disk I/O | per-device: `read_bytes_rate`, `write_bytes_rate`, `read_iops`, `write_iops` | node_exporter |
| Network I/O | per-interface: `rx_bytes_rate`, `tx_bytes_rate`, `rx_packets_rate`, `tx_packets_rate`, `rx_errors`, `tx_errors` | node_exporter |
| GPU | per-GPU-index: `util_pct`, `mem_used_bytes`, `mem_total_bytes` | node_exporter (dcgm-exporter) |

### App snapshot (one scrape interval, all pods aggregated)

| Category | Fields | Source |
|----------|--------|--------|
| CPU | `used_mcores_total` (sum across pods) | cAdvisor / metrics-server |
| Memory | `used_bytes_total` (sum across pods) | cAdvisor / metrics-server |
| Disk I/O | `read_bytes_rate`, `write_bytes_rate` (aggregate) | cAdvisor |
| Network I/O | `rx_bytes_rate`, `tx_bytes_rate` (aggregate) | cAdvisor |
| GPU | `util_pct`, `mem_used_bytes` (aggregate, if pods have GPU) | DCGM exporter |
| Pods | `running`, `pending`, `failed` counts | K8s pod list |

---

## Metric Names

All names follow the pattern `qs_{scope}_{subsystem}_{unit}[_rate]`.

### Node metrics

```
qs_node_cpu_used_pct                   labels: node_id, cluster_id, hostname
qs_node_cpu_load1                      labels: node_id, cluster_id, hostname
qs_node_cpu_load5                      labels: node_id, cluster_id, hostname
qs_node_cpu_load15                     labels: node_id, cluster_id, hostname

qs_node_mem_used_bytes                 labels: node_id, cluster_id, hostname
qs_node_mem_total_bytes                labels: node_id, cluster_id, hostname

qs_node_fs_used_bytes                  labels: node_id, cluster_id, hostname, mountpoint
qs_node_fs_total_bytes                 labels: node_id, cluster_id, hostname, mountpoint

qs_node_disk_read_bytes_rate           labels: node_id, cluster_id, hostname, device
qs_node_disk_write_bytes_rate          labels: node_id, cluster_id, hostname, device
qs_node_disk_read_iops                 labels: node_id, cluster_id, hostname, device
qs_node_disk_write_iops                labels: node_id, cluster_id, hostname, device

qs_node_net_rx_bytes_rate              labels: node_id, cluster_id, hostname, iface
qs_node_net_tx_bytes_rate              labels: node_id, cluster_id, hostname, iface
qs_node_net_rx_packets_rate            labels: node_id, cluster_id, hostname, iface
qs_node_net_tx_packets_rate            labels: node_id, cluster_id, hostname, iface
qs_node_net_rx_errors_rate             labels: node_id, cluster_id, hostname, iface
qs_node_net_tx_errors_rate             labels: node_id, cluster_id, hostname, iface

qs_node_gpu_util_pct                   labels: node_id, cluster_id, hostname, gpu_index
qs_node_gpu_mem_used_bytes             labels: node_id, cluster_id, hostname, gpu_index
qs_node_gpu_mem_total_bytes            labels: node_id, cluster_id, hostname, gpu_index
```

### App metrics

```
qs_app_cpu_used_mcores                 labels: app_id, project_id, pool_id
qs_app_mem_used_bytes                  labels: app_id, project_id, pool_id
qs_app_disk_read_bytes_rate            labels: app_id, project_id, pool_id
qs_app_disk_write_bytes_rate           labels: app_id, project_id, pool_id
qs_app_net_rx_bytes_rate               labels: app_id, project_id, pool_id
qs_app_net_tx_bytes_rate               labels: app_id, project_id, pool_id
qs_app_gpu_util_pct                    labels: app_id, project_id, pool_id
qs_app_gpu_mem_used_bytes              labels: app_id, project_id, pool_id
qs_app_pod_count                       labels: app_id, project_id, pool_id, phase (running|pending|failed)
```

---

## TSDB Storage Abstraction

The backend is selected via `platform_config`:

| Key | Values |
|-----|--------|
| `metrics_backend` | `none` (default), `victoria_metrics`, `influxdb`, `redis_timeseries` |
| `metrics_endpoint` | URL of the backend |
| `metrics_token` | Auth token (stored encrypted) |
| `metrics_scrape_interval_secs` | Integer, default 30 |

### `MetricsStore` trait

```rust
#[async_trait]
pub trait MetricsStore: Send + Sync {
    /// Write a batch of metric points (fire-and-forget semantics — log errors, don't propagate).
    async fn write(&self, points: &[MetricPoint]) -> AppResult<()>;

    /// Query a metric over a time range. Returns [(timestamp, value)] pairs.
    async fn query_range(
        &self,
        selector: MetricSelector,
        start: i64,
        end: i64,
        step_secs: u32,
    ) -> AppResult<Vec<MetricSeries>>;

    /// Query the most recent value(s) matching a selector.
    async fn query_latest(&self, selector: MetricSelector) -> AppResult<Vec<MetricPoint>>;
}
```

### Backend

VictoriaMetrics only. Single-binary, low footprint, PromQL-compatible, drop-in Prometheus replacement.

| Operation | Endpoint | Protocol |
|-----------|----------|----------|
| Write     | `POST {vm}/api/v1/import/prometheus` | Prometheus text exposition |
| Range query | `GET {vm}/api/v1/query_range?query=...&start=...&end=...&step=...` | PromQL |
| Instant query | `GET {vm}/api/v1/query?query=...&time=...` | PromQL |

`NullStore` is used until `metrics_endpoint` is set in platform_config.

---

## Collection Architecture

```
                    ┌────────────────────────────┐
                    │   Backend (tokio tasks)     │
                    │                             │
  Every N seconds ──►  MetricsCollectorTask       │
                    │   ├── NodeCollector          │
                    │   │   └─ HTTP GET :9100/metrics (each READY node)
                    │   │      parse: cpu, mem, disk-io, net-io, gpu
                    │   │                         │
                    │   └── AppCollector           │
                    │       ├─ K8s metrics-server  │
                    │       │  GET /apis/metrics.k8s.io/v1beta1/...
                    │       │  → CPU + mem per pod │
                    │       └─ cAdvisor (kubelet)  │
                    │          GET :10255/metrics/cadvisor (per node)
                    │          → disk-io, net-io per container
                    │                             │
                    │   MetricsStore.write(batch) │
                    └──────────────┬──────────────┘
                                   │
                          configured backend
                    ┌──────────────▼──────────────┐
                    │  VictoriaMetrics / InfluxDB  │
                    │  / RedisTimeSeries / NullStore│
                    └─────────────────────────────┘
```

### Node collector sources (node_exporter)

From node_exporter Prometheus text:
- `node_cpu_seconds_total{mode}` → CPU %
- `node_load{1,5,15}` → load average
- `node_memory_MemTotal_bytes`, `node_memory_MemAvailable_bytes` → memory
- `node_filesystem_{size,avail}_bytes{mountpoint}` → disk space
- `node_disk_{read,written}_bytes_total{device}` → disk I/O (rate = delta/interval)
- `node_disk_{reads,writes}_completed_total{device}` → IOPS (rate)
- `node_network_{receive,transmit}_bytes_total{device}` → net I/O (rate)
- `node_network_{receive,transmit}_packets_total{device}` → packet rate
- `node_network_{receive,transmit}_errs_total{device}` → error rate
- DCGM exporter (optional): `DCGM_FI_DEV_GPU_UTIL`, `DCGM_FI_DEV_MEM_COPY_UTIL`, `DCGM_FI_DEV_FB_USED`, `DCGM_FI_DEV_FB_FREE`

Rate metrics need two consecutive readings — the collector keeps the **previous snapshot** in memory to compute deltas.

### App collector sources

**Phase 1 (CPU + mem only) — metrics-server:**
```
GET /apis/metrics.k8s.io/v1beta1/namespaces/{namespace}/pods
→ { items: [{ metadata.name, containers: [{ usage: { cpu, memory } }] }] }
```
Aggregate by pod label `qs-app={app_name}`.

**Phase 2 (full) — cAdvisor:**
Each kubelet exposes cAdvisor metrics at `http://{node_ip}:10255/metrics/cadvisor` (or via the Kubernetes API proxy). Scrape each node, filter by container label `io.kubernetes.pod.namespace` + `io.kubernetes.container.name`.

### Multi-node aggregation

An app with multiple replicas may have pods on different nodes. The collector:
1. Lists all pods for the app across all nodes (`qs-app={name}` label selector).
2. Fetches cAdvisor data from each pod's host node.
3. Sums CPU, memory, disk-IO, net-IO across all containers.
4. Writes one aggregated `AppSnapshot` per app per scrape interval.

---

## API Endpoints

### Node metrics (admin only)

```
GET /admin/nodes/:id/metrics              → current snapshot (from cache or live)
GET /admin/nodes/:id/metrics/history      → query: ?metric=qs_node_cpu_used_pct&range=1h&step=60
```

### App metrics (project member)

```
GET /projects/:pid/apps/:aid/metrics          → current snapshot
GET /projects/:pid/apps/:aid/metrics/history  → query: ?metric=qs_app_cpu_used_mcores&range=1h
```

Response shape for `/history`:
```json
{
  "metric": "qs_app_cpu_used_mcores",
  "labels": { "app_id": "...", "project_id": "..." },
  "data": [[1716000000, 245.3], [1716000030, 312.1], ...]
}
```

---

## Implementation Phases

| Phase | What | Status |
|-------|------|--------|
| 1 | Data structs + MetricsStore trait + NullStore | **done (this PR)** |
| 2 | Node collector: parse node_exporter for disk-IO + net-IO (extends existing parser) | pending |
| 3 | App collector: metrics-server for CPU + mem | pending |
| 4 | App collector: cAdvisor for disk-IO + net-IO | pending |
| 5 | VictoriaMetrics backend | pending |
| 6 | InfluxDB backend | pending |
| 7 | Admin config page + backend hot-swap | pending |
| 8 | Dashboard query API | pending |
