# Frontend → Rust Backend Migration Tasks

**Architecture principle**: Rust backend is the single source of truth. The old
frontend's `src/server/services/`, Kubernetes adapters, Longhorn adapter, Traefik
service, Prisma client are fully discarded. Next.js becomes a pure UI calling
the Rust backend via `backend-api.adapter.ts`.

**Volume design**: No Longhorn / no K8s PVC objects. Persistent volumes use
**hostPath mounts**. The backend derives the host path from a configured storage
root + project/app/volume IDs. Every worker node has an identical directory
structure, so paths are consistent. Users specify only `container_mount_path`
and an optional display name; the system manages the actual host path.

---

## Phase A — Foundation (no visible UI change)

- [x] A1. Fix `010_frontend_gaps.sql` MySQL 8 syntax + redesign volumes schema (hostPath model, remove Longhorn/PVC artifacts)
- [x] A2. Apply migration 010 to local DB
- [x] A3. Add missing `app_type`, `use_network_policy`, `ingress_network_policy`, `egress_network_policy` columns to Rust `apps` GET + PUT
- [x] A4. Rewrite `action-wrapper.utils.ts` — remove `userGroupService` dependency; `getUserSession()` reads JWT only; authorization delegates to backend 403
- [x] A5. Add small missing Rust endpoints:
  - [x] `GET /api/v1/projects/:pid/apps/:aid/pods` — pod list
  - [x] `POST /api/v1/projects/:pid/apps/:aid/webhook/regenerate`
  - [x] `GET /api/v1/projects/:pid/apps/:aid/deployments` — deployment history
  - [x] `GET /api/v1/projects/:pid/apps/:aid/db-credentials` — DB creds by app
  - [x] `POST /api/v1/admin/nodes/:id/cordon` + `/uncordon`
  - [x] `POST /api/v1/admin/s3-targets/test`
  - [x] `GET /auth/registration-status`

---

## Phase B — Core user-facing pages

- [x] B1. Migrate `/projects` actions.ts → `backend.projects.*`
- [x] B2. Migrate `/project/:id` actions.ts → `backend.apps.create/delete`
- [x] B3. Migrate `/project/app/:id` layout → `backend.apps.get()`
- [x] B4. Migrate Overview tab actions → builds, pods, logs URL, deploy/pause/resume/scale, webhook, metrics
- [x] B5. Migrate Environment tab actions → `backend.apps.env.*`
- [x] B6. Migrate Domains tab actions → `backend.apps.domains.*`
- [x] B7. Migrate Ports tab actions → `backend.apps.ports.*` (done with domains)
- [x] B8. Replace Next.js proxy routes:
  - [x] `api/pod-logs/route.ts` → proxy SSE to Rust app logs
  - [x] `api/build-logs/route.ts` → proxy SSE to Rust build logs
  - [x] `api/deployment-status/route.ts` → poll Rust app status via `/monitoring/app-status`
  - [x] Delete `api/print-schedules-jobs/route.ts`
  - [x] Delete `api/v1/webhook/deploy/route.ts` (Rust handles `/webhooks/:id` directly)

---

## Phase C — App advanced tabs (new Rust APIs required)

- [x] C1. Add basic auth endpoints to Rust:
  - `GET/PUT/DELETE /api/v1/projects/:pid/apps/:aid/basic-auth`
- [x] C2. Migrate Advanced tab — basic auth, network policy, health check
- [x] C3. Add managed host-volume endpoints to Rust:
  - `GET/POST /api/v1/projects/:pid/apps/:aid/managed-volumes`
  - `PUT/DELETE /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid`
  - `GET /api/v1/projects/:pid/managed-volumes/shareable?excludeAppId=`
  - `GET /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/usage`
- [x] C4. Add host-volume backup schedule endpoints to Rust:
  - `GET/POST /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups`
  - `PUT/DELETE /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups/:bid`
  - `POST /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups/:bid/run`
- [x] C5. Migrate Volumes tab — managed volumes + backup schedules (actions migrated; UI components use any cast pending full type migration)
- [x] C6. Add DB tools endpoints to Rust:
  - `GET/POST /api/v1/projects/:pid/apps/:aid/db-tools`
  - `GET/DELETE /api/v1/projects/:pid/apps/:aid/db-tools/:tool`
- [x] C7. Migrate Credentials tab — DB creds + DB tools
- [x] C8. Migrate General tab → `backend.apps.update()` (done in earlier B phase)

---

## Phase D — Settings + Profile

- [x] D1. Migrate Profile settings → `backend.profile.*`, `backend.auth.totp*`
- [x] D2. Add S3 test endpoint to Rust + migrate S3 Targets settings
- [x] D3. Migrate Users settings (global admin flag; project member roles replace user groups)
- [x] D4. Rebuild Server settings into new tabs:
  - Proxy Managers (replaces Traefik)
  - Nodes (with cordon/uncordon)
  - Registries
  - Resource Pools
  - Clusters + Storage
  - Platform Config
  - IP Pools
  - DB Clusters
  - Remove: Traefik, Longhorn UI, K3s/Longhorn upgrade, system backup, maintenance

---

## Phase E — Aggregated views

- [x] E1. Add Rust monitoring aggregate endpoints:
  - `GET /api/v1/monitoring/apps` — all accessible apps metrics
  - `GET /api/v1/monitoring/managed-volumes` — all accessible volume disk usage
  - `GET /api/v1/admin/nodes/metrics/aggregate` — cluster-wide node resource totals
- [x] E2. Migrate Monitoring page → new aggregate endpoints
- [x] E3. Add Rust backup file endpoints:
  - `GET /api/v1/backups?s3_target_id=` — returns backup schedules from DB
  - `DELETE /api/v1/backups/:s3_target_id/file?key=` — stub (S3 client deferred)
  - `GET /api/v1/backups/:s3_target_id/download?key=` — stub (S3 client deferred)
- [x] E4. Migrate Backups page → new backup schedule endpoints (S3 file browsing deferred post-MVP)

---

## Phase F — Cleanup

- [x] F1. Delete all `src/server/services/` files
- [x] F2. Delete obsolete `src/server/adapter/` files (keep only `backend-api.adapter.ts`)
- [x] F3. Remove all Prisma packages from `package.json`, delete `prisma/` directory
  - `@prisma/client` types replaced by `src/shared/model/prisma-compat.ts` shim across all 25 UI files

---

## Bug-fixes applied this session

- [x] `QS_ENCRYPTION_KEY` panic on startup — now falls back to `config.crypto.key`
- [x] Migration 003 `Can't DROP ... quota_cpu_mcores` — added `IF EXISTS` to all DROP COLUMN statements; delete stale `_sqlx_migrations` row with `success=0` before restart
- [x] Billing frontend page — added `/billing` with WalletCard, TransactionsTable, InvoicesTable, PaginationControls (URL-param based, `tx_page` / `inv_page`)
- [x] `usage_snapshots` never populated — added `take_hourly_usage_snapshots()` to `billing.rs`; called from `run_billing_tasks` every 5 min (INSERT IGNORE deduplicates per hour); cost driven by `price_cpu_mcore_hour`, `price_mem_mb_hour`, `price_db_hour` platform_config keys

---

## Deferred (post-MVP)

- Volume file browser (`fileBrowserService.deployFileBrowserForVolume`)
- Volume data download/upload as zip
- Volume restore from zip
- K3s / Longhorn in-UI version check and upgrade
- System backup (full cluster backup)
- App templates preset system
- K8s pod CPU/RAM metrics scrape — `ScrapeTask::spawn()` in `collector.rs` is a stub (logs but does not actually scrape); monitoring charts always empty until implemented
- App pause button missing from frontend (backend `POST .../pause` exists)
- Build cancel is a stub (endpoint exists, no K8s job cancellation)
- DB tool deploy/delete only writes DB rows, no K8s pods started

---

## Key architectural decisions

| Old system | New system |
|---|---|
| Traefik ingress | Pingora proxy managers (`/admin/proxy-managers`) |
| Longhorn PVC volumes | System-managed hostPath volumes (consistent path across all workers) |
| User groups + per-project permission matrix | `is_global_admin` flag + project member roles (`owner/admin/developer/viewer`) |
| Next.js Prisma/K8s backend | Rust backend only |
| Next.js log streaming routes | Direct WS/SSE from browser → Rust |
| Next.js webhook endpoint `/api/v1/webhook/deploy` | Rust handles `/webhooks/:webhook_id` directly |
| `app_extra_volumes` (user-specified arbitrary hostPath) | Kept as-is for admin use |
| PVC volumes (Longhorn) | `app_managed_volumes` (system-assigned hostPath, user specifies mount path only) |

---

## Database tables added by migration 010 (revised)

| Table | Purpose |
|---|---|
| `app_managed_volumes` | System-managed hostPath volumes per app |
| `app_volume_backups` | Per-volume backup schedules (to S3) |
| `app_basic_auth` | HTTP basic auth credentials per app |
| `app_db_tools` | Ephemeral DB management tool deployments (dbgate / phpmyadmin / pgadmin) |

Columns added to `apps`:
- `app_type ENUM('APP','POSTGRES','MYSQL','MARIADB','MONGODB','REDIS')`
- `use_network_policy TINYINT(1)`
- `ingress_network_policy ENUM('ALLOW_ALL','NAMESPACE_ONLY','DENY_ALL','INTERNET_ONLY')`
- `egress_network_policy ENUM('ALLOW_ALL','NAMESPACE_ONLY','DENY_ALL','INTERNET_ONLY')`
