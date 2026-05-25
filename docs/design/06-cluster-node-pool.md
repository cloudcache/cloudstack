# 06 — Resource Pool / Cluster / Node / App Hierarchy

## Overview

CloudStack organises its compute infrastructure into a three-level hierarchy:

```
resource_pools  (geographic / logical zone, e.g. "nyc", "eu-west")
  └── clusters  (one K3s cluster per entry; stores its own kubeconfig)
        └── cluster_nodes  (physical/VM hosts in that cluster)

apps  ──── pool_id ──► resource_pools   (user workloads land in a pool)
database_clusters ──── pool_id ──► resource_pools
```

A **resource pool** is an admin-defined zone (region, data-centre rack, GPU island, …).  
A **cluster** is a K3s control plane that lives inside one pool.  
A **node** belongs to exactly one cluster.  
An **app** targets a pool; at deploy-time the system picks the active cluster for that pool.

---

## Data Model

### `resource_pools`

| Column | Type | Notes |
|--------|------|-------|
| id | CHAR(36) PK | UUID |
| name | VARCHAR(64) UNIQUE | slug, alphanumeric + `-` + `_` |
| display_name | VARCHAR(128) | human label |
| region | VARCHAR(64) NULL | optional geo tag |
| description | TEXT NULL | |
| is_active | TINYINT(1) | default 1 |
| created_at | DATETIME | |

### `clusters`

| Column | Type | Notes |
|--------|------|-------|
| id | CHAR(36) PK | UUID |
| pool_id | CHAR(36) FK | → `resource_pools.id` |
| name | VARCHAR(64) UNIQUE | slug |
| display_name | VARCHAR(128) | |
| description | TEXT NULL | |
| k3s_token | VARCHAR(512) NULL | AES-256-GCM encrypted |
| kubeconfig | MEDIUMTEXT NULL | AES-256-GCM encrypted; written after first master node joins |
| is_active | TINYINT(1) | default 1 |
| created_at | DATETIME | |

`k3s_token` is auto-generated (32-char hex) at cluster creation if not supplied.  
`kubeconfig` is populated by the node-provisioning flow after the master node is ready.

### `cluster_nodes`

| Column | Type | Notes |
|--------|------|-------|
| id | CHAR(36) PK | UUID |
| cluster_id | CHAR(36) FK NULL | → `clusters.id` |
| hostname | VARCHAR(255) | |
| ip_address | VARCHAR(45) | |
| node_role | VARCHAR(32) | `MASTER` / `WORKER` |
| node_status | ENUM | `PROVISIONING`, `READY`, `NOT_READY` |
| has_gpu | TINYINT(1) | |
| gpu_model, gpu_count | … | |
| cpu_capacity_mcores, mem_capacity_mb | INT NULL | synced from K8s |
| storage_available | TINYINT(1) | |
| storage_path | VARCHAR(512) NULL | local FS path for hostPath volumes |
| pod_cidr | VARCHAR(32) NULL | filled after K8s Ready |
| k8s_labels | TEXT NULL | JSON map, kept in sync |
| ldap_auth_active | TINYINT(1) | |
| last_seen_at | DATETIME NULL | |
| created_at | DATETIME | |

### `apps` (relevant columns)

| Column | Notes |
|--------|-------|
| pool_id | CHAR(36) FK → `resource_pools.id`; chooses which pool the app runs in |

### `database_clusters` (relevant columns)

| Column | Notes |
|--------|-------|
| pool_id | CHAR(36) FK → `resource_pools.id` |
| manager_url | VARCHAR(512) NULL — DB manager HTTP endpoint (e.g. `http://db-mgr.nyc:8080`) |

---

## Cluster Routing

Every K8s operation requires a `kube::Client`. The client is built from the per-cluster encrypted kubeconfig:

```rust
// src/k8s/mod.rs
pub async fn client_for_cluster(state: &AppState, cluster_id: &str) -> AppResult<Client> {
    let row = sqlx::query_scalar!(
        "SELECT kubeconfig FROM clusters WHERE id = ?", cluster_id
    ).fetch_one(&state.db).await?;
    let yaml = state.crypto.decrypt(&row)?;
    build_client_from_yaml(&yaml).await
}
```

For app operations the cluster is resolved from the app's `pool_id`:

```rust
// src/api/apps.rs
async fn resolve_cluster_for_app(state: &AppState, app_id: &str) -> AppResult<String> {
    let pool_id = sqlx::query_scalar!(
        "SELECT pool_id FROM apps WHERE id = ?", app_id
    ).fetch_one(&state.db).await?;
    sqlx::query_scalar!(
        "SELECT id FROM clusters WHERE pool_id = ? AND is_active = 1 LIMIT 1", pool_id
    ).fetch_one(&state.db).await?
    .ok_or_else(|| AppError::NotFound("no active cluster for pool".into()))
}
```

---

## Node Provisioning Flow

```
POST /admin/nodes
  │
  ├─ validate cluster exists + is_active
  ├─ INSERT cluster_nodes (status = PROVISIONING)
  └─ tokio::spawn ─► provision_node()
       │
       ├─ read k3s_token from clusters (decrypt)
       ├─ find master_ip FROM cluster_nodes WHERE cluster_id=? AND node_role='MASTER'
       │
       ├─ SSH connect (password auth)
       │    ├─ [MASTER] install K3s server
       │    │    flags: --flannel-backend=host-gw
       │    │           --cluster-cidr=10.244.0.0/16
       │    │           --service-cidr=10.96.0.0/12
       │    │           --node-ip={ip}
       │    │           --disable traefik --disable servicelb
       │    │    wait for /etc/rancher/k3s/k3s.yaml (up to 60 s)
       │    │    cat kubeconfig → replace 127.0.0.1 with node IP
       │    │    → store encrypted in clusters.kubeconfig
       │    │    → configure_local_path_storage()
       │    │
       │    └─ [WORKER] install K3s agent
       │         K3S_URL=https://{master_ip}:6443
       │         K3S_TOKEN={token}
       │         --node-ip={ip}
       │
       ├─ wait_for_node_ready()   (poll every 10 s, max 5 min)
       │    → on Ready: update node_status=READY, pod_cidr, last_seen_at
       │
       └─ on error: node_status = NOT_READY
```

### `--flannel-backend=host-gw` (Bridge / L3 routing)

Each node gets a unique pod CIDR (e.g. `10.244.1.0/24`).  
K3s adds static routes between nodes — no VXLAN encapsulation, minimal overhead.  
Requires all nodes to be on the **same L2 segment** (same VLAN / broadcast domain).

### hostPath Storage

K3s ships a built-in `local-path` StorageClass backed by a directory on each node.  
After master install, `configure_local_path_storage()` patches the `local-path-config` ConfigMap in `kube-system` to use the node's `storage_path` (e.g. `/data/quickstack`):

```rust
async fn configure_local_path_storage(
    state: &AppState, cluster_id: &str, storage_path: &str,
) -> AppResult<()> {
    let client = client_for_cluster(state, cluster_id).await?;
    let cm_api: Api<ConfigMap> = Api::namespaced(client, "kube-system");
    let config_json = serde_json::json!({
        "nodePathMap": [{ "node": "DEFAULT_PATH_FOR_NON_LISTED_NODES",
                          "paths": [storage_path] }]
    });
    cm_api.patch("local-path-config",
        &PatchParams::apply("quickstack"),
        &Patch::Apply(/* ConfigMap with config.json key */),
    ).await?;
    Ok(())
}
```

App volumes (user home, app data, logs, LDAP files, /etc/hosts) are **HostPath** mounts built in `src/k8s/pod_spec.rs` — no PVC required.

---

## Namespace Lifecycle

K8s namespaces map 1-to-1 with projects (using `projects.name` as the namespace name).  
Namespaces are created **lazily** on first app deploy — the cluster is already known at that point:

```rust
// src/k8s/deployment.rs  deploy_app()
super::namespace::ensure_namespace_with_client(client.clone(), &ns).await?;
```

On project deletion, CloudStack iterates all clusters referenced by apps in that project and calls `delete_namespace(state, cluster_id, ns)` on each (best-effort, errors are logged and ignored).

---

## Admin API Endpoints

### Resource Pools

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/resource-pools` | list all pools |
| POST | `/admin/resource-pools` | create pool |
| GET | `/admin/resource-pools/:id` | get pool |
| PUT | `/admin/resource-pools/:id` | update pool |
| DELETE | `/admin/resource-pools/:id` | delete pool (guard: no clusters) |

### Clusters (K3s)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/clusters` | list (with pool info, node_count, ready_count) |
| POST | `/admin/clusters` | create cluster (auto-generates k3s_token if absent) |
| GET | `/admin/clusters/:id` | get cluster |
| PUT | `/admin/clusters/:id` | update cluster |
| DELETE | `/admin/clusters/:id` | delete (guard: no nodes) |

### Nodes

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/nodes` | list all nodes (cluster + pool join) |
| POST | `/admin/nodes` | add node → triggers async SSH provisioning |
| GET | `/admin/nodes/:id` | get node |
| DELETE | `/admin/nodes/:id` | drain + delete |
| PUT | `/admin/nodes/:id/labels` | update K8s node labels |
| GET | `/admin/nodes/:id/health` | live K8s status sync |

### Cluster-wide Storage

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/cluster/storage` | read `storage_root` platform config |
| PUT | `/admin/cluster/storage` | update `storage_root` |

### Image Registries

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/registries` | list (ordered by priority) |
| POST | `/admin/registries` | create (password encrypted) |
| GET | `/admin/registries/:id` | get |
| PUT | `/admin/registries/:id` | update |
| DELETE | `/admin/registries/:id` | delete |

### IPAM

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/ip-pools` | list pools |
| POST | `/admin/ip-pools` | create pool (CIDR validated) |
| GET | `/admin/ip-pools/:id` | get pool |
| PUT | `/admin/ip-pools/:id` | update pool |
| DELETE | `/admin/ip-pools/:id` | delete pool |
| GET | `/admin/ip-pools/:id/allocations` | list allocations |
| POST | `/admin/ip-pools/:id/allocations` | allocate IP (first-fit or specific) |
| DELETE | `/admin/ip-pools/:id/allocations/:ip` | release IP |

---

## Security Notes

- `k3s_token` and `kubeconfig` are stored encrypted (AES-256-GCM) via `state.crypto`.
- Image registry passwords are encrypted at rest.
- All admin endpoints require `auth.is_global_admin = true`.
- K3s API server is reachable only from the backend; no NodePort exposure.
- SSH provisioning uses password auth for first contact; key-based auth can be layered on top.

---

## Example: Deploy an App to the "nyc" Pool

```
Admin:  POST /admin/resource-pools  { name: "nyc", display_name: "New York" }
Admin:  POST /admin/clusters        { pool_id: "<nyc-id>", name: "nyc-k3s-01" }
Admin:  POST /admin/nodes           { cluster_id: "<nyc-k3s-01-id>",
                                      hostname: "nyc-node-01", ip_address: "10.0.1.10",
                                      node_role: "MASTER", ssh_password: "…" }
        → async: K3s installed, kubeconfig stored in clusters.kubeconfig

User:   POST /projects/:id/apps     { …, pool_id: "<nyc-id>" }
User:   POST /projects/:id/apps/:app_id/deploy
        → resolve_cluster_for_app → "nyc-k3s-01"
        → client_for_cluster("nyc-k3s-01")
        → ensure_namespace, apply Deployment + Service
```
