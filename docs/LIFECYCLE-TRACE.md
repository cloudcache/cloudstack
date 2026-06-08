# Lifecycle Trace: K3S vs Docker

End-to-end trace of four lifecycles — **node init, app, app-template, database** — across the two
orchestrators a cluster can run (`clusters.orchestrator` = `K3S` | `DOCKER`). Dispatch happens at
`cluster_orchestrator(state, cluster_id)` (`api/apps.rs:1664`, defaults to `K3S` when NULL); handlers
branch on the returned string. `src/k8s/*` and `src/docker/*` are the two parallel backends. Docker
nodes run a **qs-agent** (`/agent`, `src/docker/agent_client.rs`) the backend drives over HTTP.

> Material gap claims below were spot-verified against source. Line numbers are approximate.

---

## 1. Node initialization

Entry: `api/nodes.rs::add` (validate cluster + orchestrator → insert `cluster_nodes` row → spawn async
provision task). Status machine: `PROVISIONING → READY | NOT_READY`; `provision_error/step/attempt/
cancel` columns (migration 017); SSE progress via `node_provision_logs` + `/admin/nodes/:id/
provision-stream`.

**K3S** (`k8s/node.rs::provision_node` + `ssh/mod.rs`): SSH in with `ssh_password` → install platform
SSH key → common steps (distro, tz, LDAP nslcd, storage dir, NIC/GPU detect, node_exporter:9100) →
write **bridge CNI** conflist (multi-NIC aware) → `curl get.k3s.io` install **server** (`--disable
traefik/servicelb --flannel-backend=none`) or **agent** (`K3S_URL`/`K3S_TOKEN`) → symlink CNI bins →
master persists encrypted kubeconfig + patches CoreDNS + optional nvidia plugin → `wait_for_node_ready`
polls kube API for `Ready` + reads `pod_cidr` → `node_status=READY`.

**Docker** (`k8s/node.rs::provision_docker_node` + `ssh/mod.rs::run_docker`): SSH common steps →
install Docker Engine (`get.docker.com`) → optional nvidia-container-toolkit → install **qs-agent**
binary + systemd unit (`--port agent_port --node-id --agent-token --backend-url`) → poll agent
`GET /status` (120s) → store cpu/mem from agent → `node_status=READY`. Agent (`/agent/src/main.rs`)
connects to local Docker via bollard, exposes `/containers/*`, `/networks/ensure`, `/status`.

**Divergence / gaps:**
- Readiness: kube API `Ready` (K3S, 5min) vs HTTP `/status` ok (Docker, 2min).
- `pod_cidr`, kubeconfig, CoreDNS, CNI → K3S only; Docker has no pod-network isolation.
- **Cordon/uncordon: K3S patches the node; Docker has NO equivalent** (agent_client has no cordon/drain;
  docker scheduler ignores a "cordoned" state).
- **Agent has no heartbeat/callback** — `--backend-url` is passed but unused; backend learns state only
  by polling. No container-state recovery if the agent restarts.
- Docker node metrics not refreshed post-provision (node_exporter installed but never scraped on the
  Docker path).
- Provision **cancel is advisory** — checked between steps, doesn't kill an in-flight SSH command.
- Agent binary assumed served at `{backend_url}/static/qs-agent` (confirm static serving exists).

---

## 2. App

**Create** (`api/apps.rs::create_app`, identical both): one `apps` row; `cluster_id` NULL until first
deploy (`resolve_cluster_for_app`, least-loaded active cluster in pool); `webhook_id` generated.

**Deploy** (`api/apps.rs::deploy`, branches at ~line 701):
- **K3S** (`k8s/deployment.rs::deploy_app`): decrypt env → resolve image (GIT→built image, else
  container_image) → imagePullSecret (app-level creds **or platform `image_registries` via
  `image_registry_id`**) → ConfigMap for file mounts → hostPath extra volumes → `pod_spec.rs` (tz mount,
  LDAP files, home/appdata/logs, security ctx, resources, probes, GPU `nvidia.com/gpu`, anti-affinity) →
  server-side apply **Deployment** + NodePort **Service** + **NetworkPolicy** (ingress/egress) →
  `status=DEPLOYING`. Reconciled by `k8s/status_sync.rs` (30s, classify_deployment).
- **Docker** (`docker/deployment.rs::deploy_app`): decrypt env → image → volumes (home/appdata/logs/LDAP
  via hostPath; **tz via `TZ` env, not a mount**) → file mounts sent as `FileMount` to agent → ports
  (allocate_nodeport — **shared pool with K3S**) → health check struct → registry auth (**app-level
  only**) → IP via `k8s::network::allocate_ip_for_docker` + `ensure_docker_network` → `scheduler::
  pick_nodes` (load-aware round-robin) → per node `agent.run_container` → rows in `docker_containers` →
  `status=DEPLOYING`. Reconciled by `docker/status_sync.rs` (30s, polls agent).

**Ops:** scale/pause(→0)/resume/delete branch per orchestrator. Logs/terminal: K3S via kube API; Docker
proxied from agent (`/containers/:id/logs`, WS terminal).

**Divergence / gaps (Docker is the secondary backend):**
- ~~Scale-up does a full `deploy_app` redeploy~~ — FIXED: `deploy_app_inner(add)` adds only the delta
  on scale-up (existing containers stay running; no downtime), full redeploy path unchanged.
- ~~Docker missing `/etc/hosts` mount~~ — FIXED: mounted read-only when `mount_etc_hosts`.
- ~~Docker missing platform `image_registries` (`image_registry_id`)~~ — FIXED: `deploy_app` now falls
  back to the linked `image_registries` row and builds `RegistryAuth` (parity with K3S pull-secret).
- GPU — NOT a gap: `deploy_app` already sets `gpu_count` and the agent applies an nvidia `DeviceRequest`
  (`agent/src/docker_ops.rs`); scheduler filters GPU-capable nodes.
- Anti-affinity — NOT a gap: K3S applies it unconditionally and SOFT (`preferred_during_scheduling`,
  `pod_spec.rs:229`); the Docker scheduler already prefers distinct nodes (cycles back only when nodes
  run short) — equivalent behavior.
- ~~Builds are K8s-Job only~~ — FIXED: `builds.rs` is orchestrator-aware; Docker builds run kaniko as a
  container via the agent (completion via new agent inspect endpoint). See `docs/DESIGN-docker-builds.md`.
- **Cordon/drain** — no Docker equivalent. STILL OPEN.
- Cross-calls: Docker reuses `k8s::deployment::allocate_nodeport` and `k8s::network::*` (nodeport + IPAM
  shared across orchestrators).

---

## 3. App template

Definition (`api/templates.rs`): `spec` (appModel), `requirements[]` (kind + env_mapping + config_files),
`inputs`, first-class image identity (`image_registry_id/repository/tag/digest`, `render_image_ref`);
visibility PUBLIC(admin)/ORG(project)/PRIVATE.

Deploy (`api/templates_deploy.rs::deploy_from_template`): validate every declared requirement has a
binding (**skip rejected**) → **pre-check binding quota** (`managed_usage::check_binding_allowed`) →
`resolve_binding` per kind → inject env (env_mapping) + render config files (minijinja) → INSERT `apps`
row + `app_ports`/`app_env_vars`(secrets encrypted)/`app_file_mounts` + `app_template_bindings` →
return `{id}`.

**Orchestrator-agnostic:** template deploy only *creates* the app (does NOT deploy). The client then
calls the normal `/deploy`, which is where K3S/Docker branch. So templates inherit all app-deploy
asymmetries above.

Binding modes: `database` managed **and** provision; `cache`/`objstore`/`mq`/`smtp` **managed-only**
(provision returns an explicit `BadRequest`, e.g. "Redis provisioning is not supported — register a
redis_endpoint and use mode=managed"); unknown → "unsupported binding: kind=.. mode=..".

Cleanup: `cleanup_bindings_for_app` (called from `delete_app`) drops `provisioned` DBs.

**Gaps:**
- No transaction around the whole deploy-from-template — a failure after the `apps` INSERT leaves an
  orphan app row (no rollback).
- Only databases support `provision`; everything else must pre-exist as a managed endpoint.

---

## 4. Database (external services model)

DBs are **external** — provisioned as a logical DB on a `database_clusters` server, never as a pod.

Cluster admin (`api/databases.rs`): CRUD `database_clusters` (host/port/admin creds AES-encrypted;
`cluster_type` MYSQL_GALERA | POSTGRESQL).

Instance lifecycle: `provision_new_database` (template path) / `databases.rs::create` (direct) →
`k8s::database::provision_mysql` connects via `sqlx::MySqlPool` to the external host and runs
`CREATE DATABASE/USER/GRANT` → store `database_instances` (password encrypted) → **K3S also**
`create_db_secret` (k8s Secret with DB_HOST/PORT/NAME/USER/PASS/URL). Creds reach the app via **env
injection** (both orchestrators) and, on K3S only, the Secret. Drop on app delete via
`cleanup_bindings_for_app → drop_database`.

`db-credentials` endpoint (`apps.rs:~1393`) is a **different concept**: a "database-as-app" (app_type
POSTGRES/MYSQL/…) whose host is `…svc.cluster.local` — **K3S-only** (depends on cluster DNS).

**Divergence / gaps:**
- **PostgreSQL provisioning unsupported** (`k8s/database.rs:44` returns BadRequest); MySQL-only.
- Docker: no k8s Secret — creds via env only (silent, no warning).
- database-as-app `db-credentials` has no Docker equivalent (no in-cluster DNS).
- **Direct `databases.rs::create` bypasses the binding quota** (verified: no `check_binding_allowed`),
  while the template path enforces it — a quota hole.
- `create_db_secret` decrypts admin creds on every op; `delete_db_secret` is best-effort (orphan risk).

---

## Cross-cutting summary

| Capability | K3S | Docker |
|---|---|---|
| Deploy primitive | Deployment+Service+NetworkPolicy+ConfigMap | agent `run_container` + `docker_containers` |
| Scale up | kube patch replicas | incremental add (delta only, no redeploy) |
| Cordon/drain | yes | **no** |
| Builds (GIT) | K8s Job | **unsupported** |
| Platform image registries | yes (`image_registry_id`) | yes (now falls back to `image_registries`) |
| GPU | yes | yes (nvidia DeviceRequest via agent) |
| Anti-affinity | soft (preferred) | soft (scheduler prefers distinct nodes) |
| tz / /etc/hosts mounts | hostPath mounts | tz via env; /etc/hosts now mounted |
| DB creds delivery | env + k8s Secret | env only |
| database-as-app db-credentials | yes (cluster DNS) | **no** |
| PostgreSQL provisioning | **no (TODO)** | **no (TODO)** |
| NodePort + IPAM | k8s::* | **reuses k8s::* (shared)** |

**Takeaway:** K3S is the primary, full-feature backend; Docker is a functional-but-reduced secondary
(missing builds, cordon, anti-affinity/GPU, platform registries, some mounts; inefficient scale-up).

Two orchestrator-independent correctness gaps — now FIXED:
- **(a) Direct DB-create quota bypass — FIXED.** `databases.rs::create` now calls
  `managed_usage::check_binding_allowed({database_instance: 1})` before provisioning, matching the
  template path (usage is instance-based: `COUNT(*) FROM database_instances`).
- **(b) deploy-from-template not atomic — FIXED.** The app row + ports + env vars + file mounts +
  binding records now write inside one transaction; on any failure it rolls back AND drops any external
  databases provisioned earlier in the same deploy (best-effort) so they don't orphan / keep counting
  against quota.

Remaining items are Docker-backend feature parity (larger) — see the matrix above.
