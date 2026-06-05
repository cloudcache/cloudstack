# QuickStack 网络方案：去 Flannel + 去 Multus，纯 Bridge 扁平二层

版本：4.1  
日期：2026-05-28  
状态：设计中

---

## 1. 目标

去掉 flannel overlay 和 Multus 元插件，bridge 作为唯一默认 CNI。所有 pod、节点、基础设施（DB/S3/LB）处于同一个扁平 L2 网段，零隔离、零 NAT。

---

## 2. 之前 vs 之后

| 环节 | 之前 | 之后 |
|------|------|------|
| K3s CNI | flannel host-gw (10.244.x.x overlay) | **无**，`--flannel-backend=none` |
| 二层接入 | Multus thick + bridge NAD | **无 Multus**，bridge 直接作为默认 CNI |
| Pod 网卡 | eth0 (flannel) + net1 (bridge VPC) + net2 (bridge PUB) | eth0 (bridge，物理 IP) |
| IP 池 | 双 pool：vpc_pool_id + pub_pool_id | **单 pool**：`clusters.pool_id` |
| IPAM | Backend 预分配 → Multus annotation 注入 | host-local 自动分配 → Backend 事后从 pod status 读取 |
| NodePort / Pingora | node_ip:nodeport | **不变** |
| status_sync | DEPLOYING → RUNNING | **不变**（新增：RUNNING 时记录 pod IP） |

---

## 3. 网络拓扑

```
              扁平 L2 网段 (e.g. 10.100.0.0/16)
┌──────────────────────────────────────────────────────┐
│                                                      │
│  Node A (10.100.0.1)        Node B (10.100.0.2)      │
│  ┌─────────┐                ┌─────────┐              │
│  │ app-x   │  ← L2 直通 →  │ app-y   │              │
│  │10.100.  │                │10.100.  │              │
│  │  1.10   │                │  1.12   │              │
│  └────┬────┘                └────┬────┘              │
│       │                          │                   │
│  MySQL(10.100.0.10)  MinIO(10.100.0.20)  Pingora    │
│                                          → nodeport  │
└──────────────────────────────────────────────────────┘

所有实体在同一 L2，互通无需路由。
Pingora 仍走 nodeport: domain → node_ip:nodeport → kube-proxy → pod
```

---

## 4. 变更详情

### 4.1 DB migration — `016_flatten_network.sql`

clusters 表：双 pool 合并为单 pool。

```sql
-- clusters.pool_id: 唯一的扁平 IP 池（替代 vpc_pool_id + pub_pool_id）
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='clusters' AND COLUMN_NAME='pool_id') = 0,
  'ALTER TABLE clusters ADD COLUMN `pool_id` CHAR(36) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- 迁移已有数据: vpc_pool_id 优先
UPDATE clusters SET pool_id = COALESCE(vpc_pool_id, pub_pool_id)
WHERE pool_id IS NULL AND (vpc_pool_id IS NOT NULL OR pub_pool_id IS NOT NULL);
```

> 不删 vpc_pool_id / pub_pool_id 列（避免破坏已有代码），只是新代码统一用 pool_id。

### 4.2 ssh/mod.rs — 节点初始化

#### Step 8: K3s 安装

```diff
  curl -sfL https://get.k3s.io | \
    INSTALL_K3S_EXEC='server \
      --disable traefik \
      --disable servicelb \
-     --flannel-backend=host-gw \
-     --cluster-cidr=10.244.0.0/16 \
+     --flannel-backend=none \
      --service-cidr=10.96.0.0/12 \
      --node-ip={ip} \
      --write-kubeconfig-mode=644' \
    K3S_TOKEN={k3s_token} sh -
```

#### 前置校验 (node.rs)

初始化前检查：
1. **cluster 必须绑定 ip_pool_id**（且 pool 有 CIDR + gateway），否则拒绝并提示
2. **检测节点物理网口数量**（step 7.5，`ls /sys/class/net` 排除虚拟接口）

#### 新增 Step 7.5: 检测网口数量

根据网口数量决定 bridge 模式：

| 网口数 | bridge 模式 | master | ipMasq | 效果 |
|--------|------------|--------|--------|------|
| ≥ 2 | L2 直通 | `{main_iface}` | false | pod IP 在物理网段，直达基础设施 |
| = 1 | 独立 bridge | 无 | true | pod 在隔离子网，出去走节点 NAT |

#### 新增 Step 10.5: 写入 bridge 默认 CNI 配置

**重要**：K3s `--flannel-backend=none` 不会设置 containerd 的 CNI 路径，containerd 使用默认路径：
- 配置：`/etc/cni/net.d/`
- 二进制：`/opt/cni/bin/`

因此 CNI 配置写入 `/etc/cni/net.d/10-bridge.conflist`，并将 K3s 自带的 CNI 二进制 symlink 到 `/opt/cni/bin/`：
```bash
K3S_BIN=/var/lib/rancher/k3s/data/current/bin
for p in bridge host-local loopback portmap; do ln -sf $K3S_BIN/$p /opt/cni/bin/$p; done
```

**多网口模式** (L2 直通)：
```json
{
  "type": "bridge",
  "bridge": "br-qs",
  "isGateway": true,
  "ipMasq": false,
  "master": "{main_iface}",
  "ipam": {
    "type": "host-local",
    "ranges": [[{"subnet": "{cidr}", "gateway": "{gw}"}]],
    "routes": [{"dst": "0.0.0.0/0"}]
  }
}
```

**单网口模式** (独立 bridge + NAT)：
```json
{
  "type": "bridge",
  "bridge": "br-qs",
  "isGateway": true,
  "ipMasq": true,
  "ipam": {
    "type": "host-local",
    "ranges": [[{"subnet": "{cidr}", "gateway": "{gw}"}]],
    "routes": [{"dst": "0.0.0.0/0"}]
  }
}
```

> `"routes": [{"dst": "0.0.0.0/0"}]` 必须存在！否则 pod 没有默认路由，无法访问 ServiceCIDR (kube-proxy iptables)。

`{pool_cidr}` / `{pool_gw}` 从 `clusters.ip_pool_id → ip_pools.cidr/gateway` 查出。

#### 删除 Step 11: deploy_multus

整块删除（L125-139），不再安装 Multus。

#### Step 11 改为: coredns hostNetwork

```bash
k3s kubectl -n kube-system patch deploy coredns --type=json \
  -p='[{"op":"add","path":"/spec/template/spec/hostNetwork","value":true},
       {"op":"add","path":"/spec/template/spec/dnsPolicy","value":"ClusterFirstWithHostNet"}]'
```

### 4.3 k8s/network.rs — 大幅精简

**整块删除：**

| 函数 | 说明 |
|------|------|
| `nad_api_resource()` | NAD API 定义 |
| `nad_name_for_pool()` | NAD 命名 |
| `ensure_network_attachment_def()` | 创建 bridge NAD |
| `ensure_cluster_nads()` | 双 pool NAD 同步 |
| `AppNetworkIps` / `IpAssignment` | 双 pool 结构体 |
| `get_or_allocate_app_ips()` | 预分配双 pool IP |
| `build_network_annotation()` | Multus 注解构建 |

**保留：**

| 函数 | 说明 |
|------|------|
| `get_or_allocate_ip()` | 核心 IPAM，Docker 模式仍用 |
| `release_app_ips()` | 删 app 时释放 IP |
| `app_ip_summary()` | 日志辅助 |

**新增 `record_pod_ip()`：**

app 变为 RUNNING 后，从 pod status 读取 IP 写入 DB：

```rust
/// 从 pod status 读取 IP，记录到 app_ip_allocations。
/// status_sync 在 app 转为 RUNNING 时调用。
pub async fn record_pod_ip(
    state: &AppState,
    app_id: &str,
    cluster_id: &str,
    pod_ip: &str,
) -> AppResult<()> {
    let pool_id: Option<String> = sqlx::query_scalar(
        "SELECT pool_id FROM clusters WHERE id = ?"
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    let Some(pool_id) = pool_id else { return Ok(()) };

    // 幂等：已存在则跳过
    let exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM app_ip_allocations WHERE app_id = ? AND pool_id = ?"
    )
    .bind(app_id).bind(&pool_id)
    .fetch_one(&state.db).await?;

    if exists { return Ok(()) }

    // 写入 ip_allocations + app_ip_allocations
    let alloc_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ip_allocations (id, pool_id, ip_address, allocated_to, purpose) \
         VALUES (?, ?, ?, ?, 'app-network')"
    )
    .bind(&alloc_id).bind(&pool_id).bind(pod_ip).bind(app_id)
    .execute(&state.db).await?;

    let aia_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO app_ip_allocations (id, app_id, pool_id, ip_address, alloc_ref_id) \
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&aia_id).bind(app_id).bind(&pool_id).bind(pod_ip).bind(&alloc_id)
    .execute(&state.db).await?;

    Ok(())
}
```

### 4.4 k8s/deployment.rs

```diff
- let net_ips = super::network::get_or_allocate_app_ips(state, app_id, cluster_id).await?;
- let multus_annotation = super::network::build_network_annotation(&net_ips);
  ...
- let pod_annotations = multus_annotation.map(|ann| {
-     let mut m = BTreeMap::new();
-     m.insert("k8s.v1.cni.cncf.io/networks".to_string(), ann);
-     m
- });
  ...
      metadata: Some(ObjectMeta {
          labels: Some(labels.clone()),
-         annotations: pod_annotations,
+         annotations: None,
```

### 4.5 k8s/status_sync.rs — RUNNING 时记录 pod IP

```rust
if new_status == Some("RUNNING") {
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), &app.ns);
    if let Ok(pods) = pod_api.list(
        &ListParams::default().labels(&format!("qs-app={}", app.name))
    ).await {
        if let Some(ip) = pods.items.first()
            .and_then(|p| p.status.as_ref()?.pod_ip.clone())
        {
            let _ = super::network::record_pod_ip(state, &app.id, cluster_id, &ip).await;
        }
    }
}
```

### 4.6 api/clusters_admin.rs

- 创建/编辑 cluster 时用 `pool_id` 替代 `vpc_pool_id` + `pub_pool_id`
- 删除 `ensure_cluster_nads()` 调用（不再有 NAD）

### 4.7 api/network.rs

- 用户侧查询网络信息：返回单 pool 而非双 pool
- `reassign_ip` 等逻辑适配单 pool

### 4.8 docker/deployment.rs

Docker 模式的 `get_or_allocate_app_ips()` 调用改为只查 `clusters.pool_id` 的单 pool 逻辑，去掉 `pub_zone` 分支。

---

## 5. 不变的部分

| 模块 | 说明 |
|------|------|
| **NodePort Service** | `ensure_app_service()` + `allocate_nodeport()`，不变 |
| **Pingora 路由** | `api/domains.rs` — node_ip:nodeport，不变 |
| **proxy/pingora.rs** | CreateHostRequest，不变 |
| **api/apps.rs** | deploy/scale/delete/pause/resume，不变 |
| **NetworkPolicy** | per-app 策略，不变 |
| **ip_pools / ip_allocations / app_ip_allocations** | 表结构不变，写入时机从部署前变为运行后（K8s），Docker 模式不变 |

---

## 6. 变更总结

| 操作 | 文件 | 行数估算 |
|------|------|---------|
| **新增** | `db/migrations/016_flatten_network.sql` | ~15 行 |
| **新增** | `k8s/network.rs` — `record_pod_ip()` | ~35 行 |
| **新增** | `ssh/mod.rs` — step 10.5 写 CNI 配置 | ~20 行 |
| **新增** | `ssh/mod.rs` — step 11 coredns patch | ~4 行 |
| **新增** | `k8s/status_sync.rs` — 读 pod IP | ~12 行 |
| **删除** | `k8s/network.rs` — NAD/Multus/双pool 全套 | ~250 行 |
| **删除** | `ssh/mod.rs` — Multus 安装 | ~14 行 |
| **删除** | `k8s/deployment.rs` — 注解注入 | ~7 行 |
| **修改** | `ssh/mod.rs` step 8 — flannel=none | ~2 行 |
| **修改** | `api/clusters_admin.rs` — 单 pool | ~20 行 |
| **修改** | `api/network.rs` — 单 pool | ~15 行 |
| **修改** | `docker/deployment.rs` — 去 pub_zone | ~10 行 |

**净效果**：删 ~270 行，增 ~86 行，改 ~47 行。

---

## 7. 验证计划

| # | 验证内容 | 通过条件 |
|---|---------|---------|
| 1 | Master provision | K3s 无 flannel pod，无 Multus pod |
| 2 | CNI 配置 | `10-bridge.conflist` 存在，子网正确 |
| 3 | coredns | hostNetwork Running，DNS 正常 |
| 4 | 部署 app | Pod Running，eth0 获得池内 IP |
| 5 | Pod ↔ 基础设施 | pod 内直连 MySQL / S3 |
| 6 | 跨节点 pod | 不同节点 pod 互 ping |
| 7 | IP 记录 | RUNNING 后 `app_ip_allocations` 有记录 |
| 8 | NodePort + Pingora | domain → nodeport → pod 正常 |
| 9 | Docker 模式 | 不受影响 |

---

## 8. 迁移

- **新集群**：直接用新方案
- **已有集群**：需重新初始化（去 flannel + 去 Multus），管理员触发"重新初始化"。DB migration 自动把已有 vpc_pool_id 迁移到 pool_id

---

## 9. 多发行版支持

provisioning 兼容 **Debian/Ubuntu** 与 **RHEL/Rocky/CentOS/AlmaLinux**。

### Step 0 — 发行版探测
通过 `/etc/os-release` 的 `ID_LIKE`（或 `ID`）判断包管理器：
- 包含 `debian` → apt
- 包含 `rhel` / `fedora` / `centos` / `rocky` / `alma` → dnf

### 包管理器分支

| 用途 | Debian/Ubuntu | RHEL/Rocky |
|------|---------------|-----------|
| 基础包 | `apt-get install wget curl` | `dnf install -y wget curl` |
| LDAP | `nslcd libnss-ldap libpam-ldap nscd` | `nss-pam-ldapd nscd` |
| NVIDIA toolkit | `.deb` 仓库 + apt | `.repo` 文件 + dnf |

### RHEL 系特殊处理（step 2）
- `systemctl disable --now firewalld` — K3s 直接管理 iptables，firewalld 会干扰
- `setenforce 0` + 修改 `/etc/selinux/config` 为 permissive — 避免 SELinux 阻挡 CNI 二进制/容器运行

### CNI 路径（跨发行版统一）
`/etc/cni/net.d/` + `/opt/cni/bin/` 是 containerd 编译时的默认路径，与发行版无关。

### POSIX 兼容性
- NIC 检测改用 `awk` 而非 `grep -P`（CentOS 7 等老版本可能没编译 PCRE）
