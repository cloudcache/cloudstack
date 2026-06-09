# Architecture & Status Review

A whole-project review: architecture, design, functionality, code quality, process flows, and blocking
points. Companion to `LIFECYCLE-TRACE.md` (deep flow traces) and `OPERATIONS-GAPS.md` (fix log).
Confidence is marked: **[V]** verified in code this review · **[?]** agent-reported, needs confirmation.

---

## 1. System overview

A multi-tenant PaaS ("QuickStack") that deploys user apps onto either **K3s** or **Docker** clusters.

| Component | Tech | Role |
|---|---|---|
| Backend | Rust (axum, sqlx/MySQL, kube-rs, bollard) — ~20k LOC | Single source of truth; all orchestration, billing, auth |
| Frontend | Next.js 15 App Router + NextAuth | Pure UI → calls backend via one adapter |
| Identity | LLDAP (MySQL-backed) | User directory; backend syncs + binds |
| Node agent | Rust (`/agent`, bollard) | Runs on Docker nodes; backend drives it over HTTP |
| Ingress | Pingora proxy-manager (external LB node) | Domain routing + TLS + bandwidth stats |
| DB | MySQL 8 | App data + embedded sqlx migrations (001–029) |
| Metrics store | VictoriaMetrics (optional) | TSDB — **scraper not implemented** (see §7) |

Dual-orchestrator dispatch: `cluster_orchestrator(cluster_id)` → `K3S`|`DOCKER`; `src/k8s/*` and
`src/docker/*` are parallel backends. Background loops in `main.rs`: quota (60s), billing (300s),
token cleanup (1h), app status-sync (30s, both backends), node health (120s), LDAP sync (configurable),
+ **startup recovery for stale PROVISIONING nodes**. [V]

## 2. Backend code quality — strong

- Clean module-per-domain layout (`api/*`), typed errors (`AppError`/`AppResult`), atomic SQL
  transactions for money paths, guarded/idempotent migrations, offline sqlx cache (regenerated &
  validated against the live schema this session). [V]
- Security baseline present: CORS allow-list, IP rate-limiting on auth+webhook, path sandboxing
  (`storage_guard`), platform-config key whitelist, encrypted secrets (AES-GCM), webhook UUID tokens. [V]
- Note: many queries use the non-macro `sqlx::query(...)` form (offline-safe) — fine, but loses
  compile-time column checking for those; `cargo sqlx prepare` covers the macro ones. [V]

## 3. Frontend — works, but heavy type debt

- App Router + server-actions + a **single backend adapter** (`backend-api.adapter.ts`, ~1.7k lines).
  NextAuth credentials provider → backend `/auth/login`; token refresh rotation. [V]
- **Type-safety debt is the dominant frontend issue:** ~151 `as any` casts across ~74 files, and
  `ignoreBuildErrors` + `ignoreDuringBuilds` enabled — a Prisma→Rust migration left camelCase↔snake_case
  schema mismatches papered over. The build cannot catch type regressions. [V/?]
  → Biggest frontend lever: generate TS types from the Rust API (or a case-transform layer) and retire
  the ignore flags incrementally.

## 4. Auth / authz — solid model

JWT access + rotating refresh tokens (sessions/refresh_tokens tables, revoked on credential change —
fixed this session). LLDAP is the identity source (local-password fallback); `is_global_admin` is
promote-only from the `lldap_admin` group. Project RBAC via `check_project_access` with role levels
(OBSERVER<OPERATOR<ADMIN) + implicit owner=ADMIN. TOTP removed (delegated to IdP). [V]

Weak spots to confirm [?]: per-user (not just per-IP) rate limiting absent on authd endpoints; LDAP
email match not case-normalized; ownership-transfer may not require the new owner to be a member;
webhook tokens non-rotatable. Worth a focused authz pass — none confirmed as exploitable yet.

## 5. Networking & ingress

Flat **L2 bridge CNI** model (Flannel/Multus removed), one IP pool per cluster; host-local IPAM for
K3s pods (recorded post-deploy), pre-allocated IPs for Docker (one per app). NodePort allocation shared
across both backends. Pingora proxy-manager (external) maps domain→node:nodeport, does TLS + per-domain
bandwidth counters feeding network billing. [V for model; proxy details V/?]

Gaps [?]: single Pingora instance (no failover), no pool-CIDR overlap validation, scale-up doesn't
allocate per-replica IPs (one app IP shared — pre-existing), NetworkPolicy construction not deeply
reviewed. Docker has no NetworkPolicy equivalent.

## 6. Storage

hostPath model: managed volumes (system-assigned path), extra volumes (admin hostPath), inline files via
ConfigMap; `storage_guard` sandboxes paths. S3 targets + backup *schedules* exist. [V]

Gaps: **backup execution + S3 file list/download/restore are stubs** (schedules stored, not run/served)
[V] — users may believe data is backed up when it isn't. No per-project storage quota; volume cleanup
on app-delete unclear [?]. PostgreSQL DB provisioning unsupported (MySQL-only) [V].

## 7. Metrics / monitoring — **architecturally present, functionally a stub**

VictoriaMetrics client is real, but `metrics/collector.rs::ScrapeTask` is a **placeholder** (logs, never
scrapes). Node capacity is admin-entered; agent `/status` returns counts, not usage. So the monitoring
aggregate endpoints (`/monitoring/apps`, node metrics) **return zeros**. [V] This is the single biggest
"looks-done-but-isn't" gap.

## 8. Functional domains (maturity)

| Domain | State |
|---|---|
| Users / auth / profile | Solid (this session: self-service password+name, token revocation; TOTP removed) |
| Projects + RBAC | Solid |
| Apps (deploy/scale/pause/logs/terminal) | Solid on K3s; Docker at parity except builds-in-progress nuances |
| App templates + service bindings (mq/redis/smtp/db/s3) | Solid; provision only for DB; atomic deploy (fixed) |
| Subscriptions + billing/wallet/invoices/Stripe | Solid + hardened (idempotent top-up, atomic, comp attribution, overdue notice) |
| Databases (external clusters) | Works for MySQL; **PostgreSQL provisioning unimplemented** |
| Builds (GIT→image via kaniko) | K3s + Docker (this session) + registry-auth closed loop; **needs real-infra test + agent redistribution** |
| Nodes/clusters/pools/IPAM/registries | Functional; node provisioning has crash-recovery |
| Monitoring | **Stub (no scraper)** |
| Backups | **Schedules only; execution/restore stub** |

## 9. Process flows
See `LIFECYCLE-TRACE.md` for verified step-by-step traces of node-init, app, template, and DB lifecycles
across both orchestrators, incl. the K3s↔Docker divergence matrix.

## 10. Blocking points — prioritized

**P0 — “looks done but isn’t” / data-risk**
1. **Metrics never scraped** — collector is a stub; all monitoring shows zeros. [V]
2. **Backups don’t run** — only schedules stored; no execution; S3 restore stubbed. Data-loss illusion. [V]
3. **Build-job status not crash-recoverable** — completion watchers are in-memory tokio tasks; if the
   backend restarts mid-build, `build_jobs` stays `RUNNING` forever (nodes have startup recovery; builds
   do **not**). [V] → add a startup reconciler for `RUNNING`/`PENDING` builds (and `management_jobs`).

**P1 — correctness/robustness**
4. Frontend type debt (151 `as any`, ignore flags) — regressions invisible at build. [V]
5. Pingora single-instance, no failover/health-check → LB is a SPOF. [?]
6. PostgreSQL provisioning unsupported despite schema/UI implying it. [V]
7. Per-user rate limiting + a few authz edges (transfer, LDAP email case) to confirm. [?]
8. Real-infra validation still pending for Docker/K3s build, Stripe webhook, end-to-end deploy. [V]

**P2 — UX/cleanup**
9. No per-project storage quota; volume cleanup on delete unclear. [?]
10. IP-pool CIDR overlap not validated; scale-up IP semantics. [?]
11. Residual cleanup: unused `totp_rs` dep + i18n totp keys; `.sqlx` gitignored (fresh clones can’t build
    offline). [V]

### Corrected stale findings (do NOT action)
- "DB-tools tab deploys nothing" — **removed entirely this session** (reworked to a CLI connection
  helper); table dropped (migration 027). Ignore any agent note about db-tools.
- Anti-affinity / GPU "missing on Docker", scale-up "full redeploy", `/etc/hosts`/registry parity — all
  **fixed** this session (see OPERATIONS-GAPS.md).

## 11. Overall assessment
Backend architecture is coherent and surprisingly mature (dual-orchestrator, billing, RBAC, migrations,
crash-recovery for nodes). The headline risks are **operational truthfulness**: monitoring and backups
look implemented but are stubs, and build-status can wedge on restart — these mislead operators. After
those, the frontend type-safety debt and real-infra validation are the next priorities.
