// ── Node metric names ─────────────────────────────────────────────────────────

pub const NODE_CPU_USED_PCT: &str        = "qs_node_cpu_used_pct";
pub const NODE_CPU_LOAD1: &str           = "qs_node_cpu_load1";
pub const NODE_CPU_LOAD5: &str           = "qs_node_cpu_load5";
pub const NODE_CPU_LOAD15: &str          = "qs_node_cpu_load15";

pub const NODE_MEM_USED_BYTES: &str      = "qs_node_mem_used_bytes";
pub const NODE_MEM_TOTAL_BYTES: &str     = "qs_node_mem_total_bytes";

pub const NODE_FS_USED_BYTES: &str       = "qs_node_fs_used_bytes";
pub const NODE_FS_TOTAL_BYTES: &str      = "qs_node_fs_total_bytes";

pub const NODE_DISK_READ_BYTES_RATE: &str  = "qs_node_disk_read_bytes_rate";
pub const NODE_DISK_WRITE_BYTES_RATE: &str = "qs_node_disk_write_bytes_rate";
pub const NODE_DISK_READ_IOPS: &str        = "qs_node_disk_read_iops";
pub const NODE_DISK_WRITE_IOPS: &str       = "qs_node_disk_write_iops";

pub const NODE_NET_RX_BYTES_RATE: &str    = "qs_node_net_rx_bytes_rate";
pub const NODE_NET_TX_BYTES_RATE: &str    = "qs_node_net_tx_bytes_rate";
pub const NODE_NET_RX_PACKETS_RATE: &str  = "qs_node_net_rx_packets_rate";
pub const NODE_NET_TX_PACKETS_RATE: &str  = "qs_node_net_tx_packets_rate";
pub const NODE_NET_RX_ERRORS_RATE: &str   = "qs_node_net_rx_errors_rate";
pub const NODE_NET_TX_ERRORS_RATE: &str   = "qs_node_net_tx_errors_rate";

pub const NODE_GPU_UTIL_PCT: &str         = "qs_node_gpu_util_pct";
pub const NODE_GPU_MEM_USED_BYTES: &str   = "qs_node_gpu_mem_used_bytes";
pub const NODE_GPU_MEM_TOTAL_BYTES: &str  = "qs_node_gpu_mem_total_bytes";

// ── App metric names ──────────────────────────────────────────────────────────

pub const APP_CPU_USED_MCORES: &str        = "qs_app_cpu_used_mcores";
pub const APP_MEM_USED_BYTES: &str         = "qs_app_mem_used_bytes";
pub const APP_DISK_READ_BYTES_RATE: &str   = "qs_app_disk_read_bytes_rate";
pub const APP_DISK_WRITE_BYTES_RATE: &str  = "qs_app_disk_write_bytes_rate";
pub const APP_NET_RX_BYTES_RATE: &str      = "qs_app_net_rx_bytes_rate";
pub const APP_NET_TX_BYTES_RATE: &str      = "qs_app_net_tx_bytes_rate";
pub const APP_GPU_UTIL_PCT: &str           = "qs_app_gpu_util_pct";
pub const APP_GPU_MEM_USED_BYTES: &str     = "qs_app_gpu_mem_used_bytes";
pub const APP_POD_COUNT: &str              = "qs_app_pod_count";
