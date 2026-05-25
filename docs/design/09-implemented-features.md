# QuickStack 已实现功能汇总（补充设计文档）

版本：1.0  
日期：2026-05-23  
状态：正式

> **说明**：本文档补充记录了在正式设计文档编写之前已落地实现的功能，包括 Worker 节点初始化与 GPU 支持、固定 IP 容器网络、资源配额强制执行和指标采集与时序数据库。内容基于代码实现整理，与运行中的系统保持一致。

---

## 目录

1. [Worker 节点初始化与 GPU 支持](#1-worker-节点初始化与-gpu-支持)
2. [固定 IP 容器网络（Multus + macvlan）](#2-固定-ip-容器网络multus--macvlan)
3. [资源配额强制执行](#3-资源配额强制执行)
4. [指标采集与时序数据库（VictoriaMetrics）](#4-指标采集与时序数据库victoriametrics)

---

## 1. Worker 节点初始化与 GPU 支持

### 1.1 概述

节点初始化（Provisioning）流程通过 SSH 远程自动完成。管理员只需提供节点 IP 和 SSH 密码，系统自动完成 k3s 安装、GPU 检测、node_exporter 部署、Multus CNI 安装等全部步骤。

### 1.2 SSH 初始化流程

```
POST /admin/nodes  { cluster_id, ip_address, ssh_password, node_role }
  │
  ├─ INSERT cluster_nodes (node_status = PROVISIONING)
  └─ tokio::spawn ─► provision_node()
        │
        ├─ SSH 连接（密码认证）
        │
        ├─ 检测主网卡
        │    ip route get 1.1.1.1
        │    → 解析 "dev <iface>" 获得主网卡名，如 eth0
        │    → 写入 clusters.node_main_iface（首次 MASTER 节点时）
        │    → 写入 cluster_nodes.main_iface
        │
        ├─ 安装 k3s
        │    ├─ [MASTER] curl -sfL https://get.k3s.io | sh -s - server
        │    │    --flannel-backend=host-gw
        │    │    --cluster-cidr=10.244.0.0/16
        │    │    --service-cidr=10.96.0.0/12
        │    │    --node-ip={ip} --disable traefik --disable servicelb
        │    │    等待 /etc/rancher/k3s/k3s.yaml（最长 60 秒）
        │    │    读取 kubeconfig → 替换 127.0.0.1 为节点 IP
        │    │    → AES-256-GCM 加密后写入 clusters.kubeconfig
        │    │    configure_local_path_storage()
        │    │
        │    └─ [WORKER] curl -sfL https://get.k3s.io | K3S_URL=... K3S_TOKEN=... sh -s - agent
        │         --node-ip={ip}
        │
        ├─ 安装 node_exporter 1.8.2
        │    下载二进制，systemd 服务，监听 :9100
        │    → 验证：curl http://{ip}:9100/metrics
        │
        ├─ 等待节点就绪
        │    每 10 秒轮询 K8s 节点状态，最长 5 分钟
        │    就绪后写入：node_status=READY, pod_cidr, last_seen_at
        │
        ├─ GPU 检测
        │    检查 /dev/nvidia0 是否存在
        │    若存在：执行 nvidia-smi --query-gpu=name,count --format=csv,noheader
        │    → 写入 cluster_nodes.(has_gpu, gpu_model, gpu_count)
        │    → 若有 GPU：部署 NVIDIA k8s device plugin DaemonSet
        │
        └─ 安装 Multus CNI DaemonSet（所有节点）
             kubectl apply -f multus-daemonset.yaml（针对 k3s 的精简版本）
```

### 1.3 InstallResult 结构

```rust
pub struct InstallResult {
    pub kubeconfig:  Option<String>,  // 仅 MASTER 节点返回
    pub has_gpu:     bool,
    pub gpu_model:   Option<String>,  // 如 "NVIDIA GeForce RTX 4090"
    pub gpu_count:   u32,
    pub main_iface:  String,          // 如 "eth0"
}
```

### 1.4 数据库字段变更

#### `cluster_nodes` 新增字段

| 列名 | 类型 | 说明 |
|------|------|------|
| `main_iface` | VARCHAR(32) NULL | 主网卡名，初始化时探测 |
| `has_gpu` | TINYINT(1) NOT NULL DEFAULT 0 | 是否有 GPU |
| `gpu_model` | VARCHAR(256) NULL | GPU 型号字符串 |
| `gpu_count` | SMALLINT UNSIGNED NOT NULL DEFAULT 0 | GPU 数量 |
| `cpu_used_pct` | DECIMAL(5,2) NULL | CPU 使用率（缓存自 node_exporter） |
| `mem_used_mb` | INT UNSIGNED NULL | 内存使用量（MB） |
| `disk_used_gb` | DECIMAL(10,2) NULL | 磁盘使用量（GB） |
| `disk_total_gb` | DECIMAL(10,2) NULL | 磁盘总量（GB） |
| `load1` | DECIMAL(6,2) NULL | 1分钟负载均值 |
| `metrics_updated_at` | DATETIME NULL | 最后一次指标缓存时间 |

#### `clusters` 新增字段

| 列名 | 类型 | 说明 |
|------|------|------|
| `vpc_pool_id` | CHAR(36) NULL | 集群关联的 VPC IP 池（用于固定 IP 网络） |
| `pub_pool_id` | CHAR(36) NULL | 集群关联的公共 IP 池 |
| `node_main_iface` | VARCHAR(32) NULL | 集群主网卡名（从首个 MASTER 节点探测） |

### 1.5 管理员 API

```
GET /admin/nodes/:id/metrics
```

从该节点的 `node_exporter`（`http://{node_ip}:9100/metrics`）抓取实时指标，解析后写入 `cluster_nodes` 的指标缓存字段，同时返回给调用方。

**响应 200：**

```json
{
  "node_id": "uuid",
  "cpu_used_pct": 23.5,
  "mem_used_mb": 4096,
  "mem_total_mb": 16384,
  "disk_used_gb": 32.1,
  "disk_total_gb": 200.0,
  "load1": 0.72,
  "updated_at": "2026-05-23T10:00:00"
}
```

### 1.6 GPU 支持现状

| GPU 类型 | 支持状态 | 机制 |
|---------|---------|------|
| NVIDIA | 已支持 | NVIDIA k8s device plugin；通过 `nvidia.com/gpu` 资源声明 |
| AMD ROCm | 待实现 | 预留 `gpu_model` 字段，检测 `/dev/kfd` |
| 其他 | 不支持 | — |

---

## 2. 固定 IP 容器网络（Multus + macvlan）

### 2.1 问题背景

k3s 默认使用 Flannel 为 Pod 分配浮动 IP（每次重启 IP 可能变化）。对于需要被平台内其他服务（如数据库管理器、对象存储、LB）稳定访问的用户应用，浮动 IP 无法满足需求。

### 2.2 解决方案

使用 **Multus CNI** 为每个 Pod 附加一个（或两个）辅助网卡，通过 **macvlan**（bridge 模式）将 Pod 接入宿主机所在的物理网络，从而获得宿主机网段内的稳定 IP。

```
┌─────────────────────────────────────────────┐
│  物理网络 (e.g. 10.10.0.0/24)               │
│                                             │
│  ┌────────────┐    ┌────────────────────┐   │
│  │  宿主机    │    │      Pod           │   │
│  │  eth0      │    │  eth0 (flannel)    │   │
│  │  10.10.0.5 │    │  10.244.1.3/32     │   │
│  │            │    │                   │   │
│  │            │    │  net1 (macvlan)    │   │
│  │            │    │  10.10.0.50/24 ◄──┼───┼── 固定 IP，稳定可达
│  └────────────┘    └────────────────────┘   │
└─────────────────────────────────────────────┘
```

### 2.3 双 IP 池设计

每个集群配置两个 IP 池：

| 池类型 | 字段 | 说明 |
|--------|------|------|
| VPC 池 | `clusters.vpc_pool_id` | 用户私有网络段，用于应用间互访 |
| 公共池 | `clusters.pub_pool_id` | 公共区域网络段，平台基础设施（DB、LB、对象存储）可直接访问 |

应用部署时，QuickStack 从两个池各分配一个 IP，Pod 获得两个 macvlan 辅助网卡（`net1`=vpc，`net2`=pub）。

### 2.4 NetworkAttachmentDefinition（NAD）

每个 IP 池在集群创建时（或绑定时）在 K8s `default` 命名空间创建一个 NAD，命名规则为 `qs-{pool_name}`：

```yaml
apiVersion: k8s.cni.cncf.io/v1
kind: NetworkAttachmentDefinition
metadata:
  name: qs-vpc
  namespace: default
spec:
  config: |
    {
      "cniVersion": "0.3.1",
      "type": "macvlan",
      "master": "eth0",          # 从 clusters.node_main_iface 读取
      "mode": "bridge",
      "ipam": { "type": "static" }
    }
```

> NAD 每个池只创建一次，所有命名空间的 Pod 均可引用（`namespace: default` 中的 NAD 可跨命名空间使用）。

### 2.5 IP 分配流程

```
deploy_app()
  │
  ├─ 查询 vpc_pool_id / pub_pool_id（来自 clusters）
  │
  ├─ 对每个 pool：
  │    SELECT ip_address FROM app_ip_allocations
  │     WHERE app_id = ? AND pool_id = ?
  │
  │    ├─ 若已存在 → 复用（幂等重部署安全）
  │    └─ 若不存在 → first-fit 分配
  │         SELECT ip FROM ip_pool_addresses
  │          WHERE pool_id = ? AND is_allocated = 0 LIMIT 1 FOR UPDATE
  │         INSERT INTO app_ip_allocations ...
  │         UPDATE ip_pool_addresses SET is_allocated = 1 WHERE ...
  │
  └─ 将分配的 IP 写入 Pod annotations
```

**释放时机**：应用删除（`DELETE /apps/:id`）时，批量释放该应用的所有 IP 分配记录。

### 2.6 Pod Annotation

```json
{
  "k8s.v1.cni.cncf.io/networks": "[
    {
      \"name\": \"qs-vpc\",
      \"namespace\": \"default\",
      \"ips\": [\"10.10.0.50/24\"],
      \"gateway\": \"10.10.0.1\"
    },
    {
      \"name\": \"qs-pub\",
      \"namespace\": \"default\",
      \"ips\": [\"192.168.1.100/24\"],
      \"gateway\": \"192.168.1.1\"
    }
  ]"
}
```

### 2.7 数据模型

#### `app_ip_allocations`

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | BIGINT UNSIGNED AUTO_INCREMENT | 主键 |
| `app_id` | CHAR(36) NOT NULL | 关联 apps.id |
| `pool_id` | CHAR(36) NOT NULL | 关联 ip_pools.id |
| `ip_address` | VARCHAR(45) NOT NULL | 分配的 IP（不含掩码） |
| `alloc_ref_id` | CHAR(36) NULL | 预留：多副本时可扩展为 per-pod 分配 |
| `created_at` | DATETIME NOT NULL | 分配时间 |

索引：
```sql
UNIQUE KEY uq_app_pool (app_id, pool_id)
KEY idx_pool_ip (pool_id, ip_address)
```

### 2.8 注意事项

- macvlan bridge 模式要求宿主机网卡开启混杂模式（`ip link set eth0 promisc on`），初始化脚本自动执行。
- 宿主机本身无法通过 macvlan 接口与 Pod 通信（macvlan 限制）；平台基础设施访问 Pod 需走 pub_pool 的物理网络，不经过宿主机。
- 固定 IP 仅在同一 L2 网段内有效；跨数据中心部署需要额外的路由配置。

---

## 3. 资源配额强制执行

### 3.1 概述

配额维度涵盖 CPU（毫核）、内存（MB）和应用数量（app_count）。系统在三个层面进行配额管控：

1. **预检（Pre-deploy check）**：部署/扩容/恢复操作前同步校验，超限返回 `429 QuotaExceeded`。
2. **后台强制执行（Background Enforcer）**：每 60 秒扫描所有活跃项目，将超限项目的应用挂起。
3. **告警（WARN）**：用量超过警戒阈值（默认 80%）时记录告警，不停服。

### 3.2 配额维度与状态区分

| 维度 | 存储位置 | 检查逻辑 |
|------|---------|---------|
| `cpu_mcores` | `projects.quota_cpu_mcores` | 所有 RUNNING 应用的 `cpu_limit` 之和 ≤ quota |
| `mem_mb` | `projects.quota_mem_mb` | 所有 RUNNING 应用的 `mem_limit_mb` 之和 ≤ quota |
| `app_count` | `projects.quota_apps` | RUNNING + PAUSED + SUSPENDED 应用数量 ≤ quota |

**PAUSED vs SUSPENDED 的区别：**

| 状态 | 触发来源 | 恢复条件 |
|------|---------|---------|
| `PAUSED` | 用户主动暂停 | 用户随时可恢复，无配额检查 |
| `SUSPENDED` | 系统配额强制执行 | 必须先释放配额（删除或缩减其他应用），再由系统或管理员恢复 |

### 3.3 阈值配置

| 配置键 | 默认值 | 说明 |
|--------|-------|------|
| `quota_warn_pct` | `80` | 用量超过此百分比时记录 WARN 告警 |
| `quota_hard_pct` | `100` | 用量超过此百分比时阻止部署；后台超限时挂起应用 |

### 3.4 预检函数

所有部署、扩容、恢复操作均调用 `check_deploy_allowed()`：

```rust
pub async fn check_deploy_allowed(
    db: &MySqlPool,
    project_id: &str,
    extra_cpu_mcores: i64,   // 本次操作新增的 CPU 请求量（可为负数，如缩容）
    extra_mem_mb: i64,
    extra_app_count: i64,
) -> AppResult<()> {
    let project = fetch_project_quota(db, project_id).await?;
    let used = fetch_project_used(db, project_id).await?;

    let new_cpu = used.cpu_mcores + extra_cpu_mcores;
    let new_mem = used.mem_mb + extra_mem_mb;
    let new_apps = used.app_count + extra_app_count;

    let hard_pct = platform_config_int(db, "quota_hard_pct", 100).await? as i64;

    if project.quota_cpu_mcores > 0
       && new_cpu * 100 > project.quota_cpu_mcores * hard_pct {
        return Err(AppError::QuotaExceeded("CPU 配额已用尽".into()));
    }
    // mem_mb、app_count 同理 ...
    Ok(())
}
```

### 3.5 后台强制执行流程

```
quota_enforcer（每 60 秒）
  │
  ├─ SELECT * FROM projects WHERE is_active = 1
  │
  └─ 对每个项目：
        计算 used_cpu, used_mem, used_apps
        │
        ├─ used / quota ≥ hard_pct?
        │    → 按 CPU 消耗降序选取 RUNNING 应用
        │    → 逐一将其 status 改为 SUSPENDED
        │    → 执行 K8s scale down（replicas=0）
        │    → 记录 quota_violations(action='suspend')
        │
        └─ used / quota ≥ warn_pct?
             → 记录 quota_violations(action='warn')
             → 不停服
```

挂起顺序（CPU 消耗降序）：优先挂起消耗资源最多的应用，以最少的挂起次数恢复配额合规。

### 3.6 数据模型

#### `quota_violations`

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | BIGINT UNSIGNED AUTO_INCREMENT | 主键 |
| `project_id` | CHAR(36) NOT NULL | 关联 projects.id |
| `app_id` | CHAR(36) NULL | 关联 apps.id（suspend 时填写；warn 无特定 app） |
| `dimension` | VARCHAR(32) NOT NULL | `cpu_mcores` / `mem_mb` / `app_count` |
| `used_value` | BIGINT NOT NULL | 触发时的实际用量 |
| `quota_value` | BIGINT NOT NULL | 触发时的配额上限 |
| `pct_used` | DECIMAL(6,2) NOT NULL | 使用率百分比 |
| `action` | ENUM('warn','suspend','block') NOT NULL | 触发的动作 |
| `resolved_at` | DATETIME NULL | 解除时间（配额释放后更新） |
| `created_at` | DATETIME NOT NULL | 记录时间 |

索引：
```sql
KEY idx_project_time (project_id, created_at)
KEY idx_app_unresolved (app_id, resolved_at)
```

### 3.7 API 接口

#### 用户侧

| 方法 | 路径 | 权限 | 说明 |
|------|------|------|------|
| `GET` | `/api/v1/projects/:pid/quota` | OBSERVER | 查看项目配额与当前用量 |
| `GET` | `/api/v1/projects/:pid/quota/violations` | OBSERVER | 查看历史配额违规记录 |

**`GET /projects/:pid/quota` 响应 200：**

```json
{
  "project_id": "uuid",
  "quota": {
    "cpu_mcores": 8000,
    "mem_mb": 16384,
    "app_count": 20
  },
  "used": {
    "cpu_mcores": 6400,
    "mem_mb": 12288,
    "app_count": 15
  },
  "pct": {
    "cpu": 80.0,
    "mem": 75.0,
    "apps": 75.0
  },
  "status": "WARN"
}
```

`status` 字段值：`OK`（< warn_pct）/ `WARN`（≥ warn_pct）/ `EXCEEDED`（≥ hard_pct）

#### 管理员侧

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/api/v1/admin/apps/:id/suspend` | 手动挂起应用（写入 SUSPENDED 状态） |
| `POST` | `/api/v1/admin/apps/:id/unsuspend` | 手动恢复被系统挂起的应用（跳过配额检查） |
| `POST` | `/api/v1/admin/projects/:pid/quota/enforce` | 立即对指定项目执行一次配额检查（不等待60秒定时器） |

---

## 4. 指标采集与时序数据库（VictoriaMetrics）

### 4.1 概述

QuickStack 目前使用 **VictoriaMetrics** 作为唯一支持的 TSDB 后端。指标分两个层级：节点级（来自 node_exporter）和应用级（来自 K8s metrics-server / cAdvisor）。

### 4.2 后端选择与配置

系统通过 `platform_config` 配置 TSDB：

| 键名 | 默认值 | 说明 |
|------|-------|------|
| `metrics_backend` | `none` | 当前仅支持 `victoria_metrics`；`none` = 使用 NullStore（丢弃所有写入） |
| `metrics_endpoint` | — | VictoriaMetrics 实例地址，如 `http://vm:8428` |
| `metrics_token` | — | 认证 Token（加密存储） |
| `metrics_scrape_interval_secs` | `30` | 采集间隔（秒） |

**VictoriaMetrics 接口：**

| 操作 | 接口 | 协议 |
|------|------|------|
| 写入 | `POST {vm}/api/v1/import/prometheus` | Prometheus 文本格式 |
| 范围查询 | `GET {vm}/api/v1/query_range?query=...&start=...&end=...&step=...` | PromQL |
| 即时查询 | `GET {vm}/api/v1/query?query=...&time=...` | PromQL |

未配置 `metrics_endpoint` 时使用 `NullStore`，写入操作静默丢弃。

### 4.3 指标命名规范

所有指标名遵循 `qs_{scope}_{subsystem}_{unit}[_rate]` 格式。

#### 节点指标（`qs_node_*`）

| 指标名 | 标签 | 说明 |
|--------|------|------|
| `qs_node_cpu_used_pct` | node_id, cluster_id, hostname | CPU 使用率（%） |
| `qs_node_cpu_load1` | node_id, cluster_id, hostname | 1分钟负载 |
| `qs_node_cpu_load5` | node_id, cluster_id, hostname | 5分钟负载 |
| `qs_node_cpu_load15` | node_id, cluster_id, hostname | 15分钟负载 |
| `qs_node_mem_used_bytes` | node_id, cluster_id, hostname | 内存使用量 |
| `qs_node_mem_total_bytes` | node_id, cluster_id, hostname | 内存总量 |
| `qs_node_fs_used_bytes` | node_id, cluster_id, hostname, **mountpoint** | 文件系统使用量（按挂载点） |
| `qs_node_fs_total_bytes` | node_id, cluster_id, hostname, **mountpoint** | 文件系统总量（按挂载点） |
| `qs_node_disk_read_bytes_rate` | node_id, cluster_id, hostname, **device** | 磁盘读速率（Bytes/s） |
| `qs_node_disk_write_bytes_rate` | node_id, cluster_id, hostname, **device** | 磁盘写速率 |
| `qs_node_disk_read_iops` | node_id, cluster_id, hostname, **device** | 磁盘读 IOPS |
| `qs_node_disk_write_iops` | node_id, cluster_id, hostname, **device** | 磁盘写 IOPS |
| `qs_node_net_rx_bytes_rate` | node_id, cluster_id, hostname, **iface** | 网络接收速率 |
| `qs_node_net_tx_bytes_rate` | node_id, cluster_id, hostname, **iface** | 网络发送速率 |
| `qs_node_net_rx_packets_rate` | node_id, cluster_id, hostname, **iface** | 网络接收包速率 |
| `qs_node_net_tx_packets_rate` | node_id, cluster_id, hostname, **iface** | 网络发送包速率 |
| `qs_node_net_rx_errors_rate` | node_id, cluster_id, hostname, **iface** | 接收错误速率 |
| `qs_node_net_tx_errors_rate` | node_id, cluster_id, hostname, **iface** | 发送错误速率 |
| `qs_node_gpu_util_pct` | node_id, cluster_id, hostname, **gpu_index** | GPU 使用率（%） |
| `qs_node_gpu_mem_used_bytes` | node_id, cluster_id, hostname, **gpu_index** | GPU 显存使用量 |
| `qs_node_gpu_mem_total_bytes` | node_id, cluster_id, hostname, **gpu_index** | GPU 显存总量 |

#### 应用指标（`qs_app_*`）

| 指标名 | 标签 | 说明 |
|--------|------|------|
| `qs_app_cpu_used_mcores` | app_id, project_id, pool_id | CPU 用量（毫核，所有 Pod 聚合） |
| `qs_app_mem_used_bytes` | app_id, project_id, pool_id | 内存用量（所有 Pod 聚合） |
| `qs_app_disk_read_bytes_rate` | app_id, project_id, pool_id | 磁盘读速率（聚合） |
| `qs_app_disk_write_bytes_rate` | app_id, project_id, pool_id | 磁盘写速率（聚合） |
| `qs_app_net_rx_bytes_rate` | app_id, project_id, pool_id | 网络接收速率（聚合） |
| `qs_app_net_tx_bytes_rate` | app_id, project_id, pool_id | 网络发送速率（聚合） |
| `qs_app_gpu_util_pct` | app_id, project_id, pool_id | GPU 使用率（聚合） |
| `qs_app_gpu_mem_used_bytes` | app_id, project_id, pool_id | GPU 显存使用量（聚合） |
| `qs_app_pod_count` | app_id, project_id, pool_id, **phase** | Pod 数量（按 running / pending / failed 分类） |

### 4.4 采集架构

```
                ┌──────────────────────────────────────────┐
                │         指标采集后台任务（tokio）          │
                │                                          │
  每 N 秒 ──── ►│  MetricsCollectorTask                    │
                │   │                                      │
                │   ├── NodeCollector                      │
                │   │    对每个 READY 节点：                │
                │   │    HTTP GET http://{node_ip}:9100/metrics
                │   │    解析 Prometheus 文本格式           │
                │   │    → cpu, mem, disk-io, net-io, gpu  │
                │   │    保留上次快照计算速率指标（delta/dt）│
                │   │                                      │
                │   └── AppCollector（stub，待实现完整）    │
                │        ├─ Phase 1: metrics-server        │
                │        │  GET /apis/metrics.k8s.io/...   │
                │        │  → CPU + mem per pod            │
                │        └─ Phase 2: cAdvisor              │
                │           GET {kubelet}:10255/metrics/cadvisor
                │           → disk-io + net-io per container
                │                                          │
                │   MetricsStore.write(batch)              │
                └──────────────────┬───────────────────────┘
                                   │
                                   ▼
                     ┌─────────────────────────┐
                     │     VictoriaMetrics      │
                     │  (或 NullStore，若未配置) │
                     └─────────────────────────┘
```

### 4.5 节点指标数据源

从 node_exporter Prometheus 文本解析：

| 数据类别 | node_exporter 原始指标 | 转换方式 |
|---------|----------------------|---------|
| CPU 使用率 | `node_cpu_seconds_total{mode}` | `1 - idle_rate` |
| 负载均值 | `node_load1/5/15` | 直接映射 |
| 内存 | `node_memory_MemTotal_bytes`, `node_memory_MemAvailable_bytes` | used = Total - Available |
| 文件系统 | `node_filesystem_{size,avail}_bytes{mountpoint}` | used = size - avail |
| 磁盘 I/O | `node_disk_{read,written}_bytes_total{device}` | rate = delta / interval |
| 磁盘 IOPS | `node_disk_{reads,writes}_completed_total{device}` | rate = delta / interval |
| 网络 I/O | `node_network_{receive,transmit}_bytes_total{device}` | rate = delta / interval |
| 网络包 | `node_network_{receive,transmit}_packets_total{device}` | rate = delta / interval |
| 网络错误 | `node_network_{receive,transmit}_errs_total{device}` | rate = delta / interval |
| GPU | `DCGM_FI_DEV_GPU_UTIL`, `DCGM_FI_DEV_FB_USED/FREE` | 直接映射（可选） |

> 速率指标（`_rate`）需要两次连续采集才能计算。收集器在内存中保留上一次快照，首次采集时速率类指标值为 0。

### 4.6 应用指标多节点聚合

```
对 app_id = X 的采集流程：

1. 列出所有 labels 含 qs-app=X 的 Pod（跨所有节点）
2. 从每个 Pod 所在节点的 cAdvisor 拉取该 Pod 的容器指标
3. 对所有容器的 CPU、内存、磁盘 I/O、网络 I/O 求和
4. 写入一条聚合的 AppSnapshot 到 TSDB
```

**K8s 标签约定**：所有 QuickStack 管理的 Pod 均带有 `qs-app={app_name}` 标签，用于指标关联和聚合。

### 4.7 实现进度

| 阶段 | 内容 | 状态 |
|------|------|------|
| 1 | 数据结构定义（NodeSnapshot, AppSnapshot）+ MetricsStore trait + NullStore | 已完成 |
| 2 | 节点采集器：解析 node_exporter 全量指标（磁盘 I/O + 网络 I/O） | 已完成 |
| 3 | 应用采集器：metrics-server（CPU + 内存） | 待实现 |
| 4 | 应用采集器：cAdvisor（磁盘 I/O + 网络 I/O） | 待实现 |
| 5 | VictoriaMetrics 后端写入 | 已完成 |
| 6 | InfluxDB 后端 | 不计划（已选定 VM） |
| 7 | 管理员配置页 + 后端热切换 | 待实现 |
| 8 | 仪表盘查询 API | 已完成（见下） |

### 4.8 API 接口

#### 节点指标（管理员）

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/v1/admin/nodes/:id/metrics` | 当前快照（从缓存或实时抓取） |
| `GET` | `/api/v1/admin/nodes/:id/metrics/history` | 历史查询，参数：`metric`、`range`（如 `1h`）、`step`（秒） |

#### 应用指标（项目成员）

| 方法 | 路径 | 权限 | 说明 |
|------|------|------|------|
| `GET` | `/api/v1/projects/:pid/apps/:aid/metrics` | OBSERVER | 最新快照 |
| `GET` | `/api/v1/projects/:pid/apps/:aid/metrics/history` | OBSERVER | 历史数据，参数：`metric`、`range`、`step` |

**`/history` 响应格式：**

```json
{
  "metric": "qs_app_cpu_used_mcores",
  "labels": {
    "app_id": "uuid",
    "project_id": "uuid",
    "pool_id": "uuid"
  },
  "data": [
    [1716000000, 245.3],
    [1716000030, 312.1],
    [1716000060, 298.7]
  ]
}
```

`data` 中每个元素为 `[unix_timestamp_seconds, value]`，与 Prometheus/VictoriaMetrics 原生响应格式一致。
