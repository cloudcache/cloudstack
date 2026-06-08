# Operational Gap Tracker

Living checklist of operational gaps across the five focus domains: **users, infrastructure
resources, subscriptions, applications, finance**. Maintained by the `/loop` audit. Findings are
*verified against source* before being listed here (speculative subagent findings were dropped or
demoted). Each iteration picks the highest-value unblocked item, fixes it surgically, and checks it
off.

Baseline: backend `cargo check` passes (SQLX_OFFLINE=true).

Legend: `[ ]` open · `[x]` done · `[~]` needs product decision (do not auto-fix)

---

## Users

- [x] **U-P0a Refresh tokens survive credential changes (security).** Password reset
  (`auth.rs` `reset_password`), admin password reset (`users.rs`), and account deactivation
  (`users.rs`) deleted `user_sessions` but left `refresh_tokens` — a stolen/old refresh token could
  still mint new access tokens after a reset or deactivation. Fixed: each site now also deletes
  `refresh_tokens` for the user. *(iteration 1)*
- [x] **U-1a Self-service `change_password` implemented** (`profile.rs`). Was a 403 stub; the full
  frontend flow (`settings/profile/profile-password-change.tsx` → `actions.ts` → adapter) already
  existed. New handler verifies the current password against whichever backend owns it (local argon2
  hash, or LDAP bind for pure-LDAP users) then applies the new password to the same backend —
  mirroring the login auth path. Made `verify_password` `pub(crate)`. *(iter 3)*
- [x] **U-1b Self-service `update_profile` (display name) implemented.** Was a 403 stub. Backend
  (`profile.rs`) now validates `display_name` (non-empty, ≤128), updates the local row + mirrors to
  LLDAP (`update_user`, non-fatal) — same dual path as login/change_password. Username & email stay
  admin/LDAP-managed. Frontend: new `profile-info.tsx` card (editable display name; username/email
  shown read-only) + `updateProfile` action, wired into the profile page (`me` already returns the
  fields). Backend green, touched FE files typecheck clean. *(iter 8)*
- [ ] **U-2 Password reset not gated on email verification.** `forgot_password` issues a reset link to
  any active user incl. unverified ones; after reset they log in then immediately hit the
  EMAIL_NOT_VERIFIED block. Consider auto-verifying on successful reset, or gating reset.
- [x] **U-3 obsolete — TOTP removed entirely.** Per owner, 2FA is delegated to the IdP (LLDAP). Removed
  the half-baked app-level TOTP: backend handlers + routes + login verification + `totp_code`/
  `totp_enabled`; frontend totp-settings/create-dialog/two-fa-auth/totp.model + adapter methods +
  auth-options totpToken; migration `029` drops `totp_credentials`. (Unused `totp_rs` dep + i18n keys
  left as harmless cleanup.) *(iter 10)*
- [ ] **U-4 No admin "mark email verified" escape hatch.** If verification mail is lost, admin can only
  trigger resend. Consider `PATCH /admin/users/:id { email_verified }`.
- [ ] **U-5 Frontend verify-email UX inconsistency.** Login error message is zh; `/verify-email` page is
  en. Minor copy/i18n cleanup.

## Infrastructure resources

- [ ] **R-1 Orphaned node-provision endpoints (verified).** Backend exposes provision-logs/cancel/
  stream (`mod.rs:261-263`) but `adminNodes` adapter only has `create`/`reprovision` — no method
  consumes logs/cancel/stream, and the nodes tab only shows the static `provision_error`. Not broken,
  but a live provision-log viewer (SSE) is backend-ready and unsurfaced. Sizeable frontend feature,
  not a surgical fix — schedule deliberately.
- [ ] **R-2 Service endpoint password edit UX.** MQ/SMTP/Redis update skips re-encryption when password
  field is blank ("leave blank to keep"); confirm the UI communicates this so users don't think blank
  clears the secret.
- [ ] **R-3 IP pool `pool_type` defaulting.** `ipam.rs` silently defaults `pool_type` to "LB"; confirm
  the admin form surfaces the choice.

## Subscriptions

- [x] **S-1 Plan quota shape — NOT a bug.** Verified: `admin-plans-tab.tsx:47-49` has a `q()` helper
  that reads both nested (`plan.quota.*`) and flat (`quota_*`) formats. Display is correct. *(iter 2)*
- [ ] **S-2 `subscription_self_service` config not reflected in UI.** When disabled the subscribe
  endpoint 404s with no frontend guard hiding the button.
- [x] **S-3 Admin balance operations audited** (recharge/gift + adjustment). Hardened
  `api/billing.rs` for financial-record integrity: (完整性) DB-tx + explicit target-user existence
  check; (唯一性) client `idempotency_key` with a UNIQUE index (migration
  `026_wallet_tx_idempotency.sql`) so a retried/double-submitted op is a no-op replay, incl. a
  concurrent-race recovery path; (可追溯) `operator_id` + a now-mandatory reason on every entry;
  (可审计) `wallet_transactions` confirmed append-only (no UPDATE/DELETE anywhere) — it is the
  immutable ledger. Both handlers refactored onto a shared `apply_admin_balance_change` core.
  Frontend wired end-to-end: `admin-billing-tab.tsx` generates the key per dialog-open and requires a
  reason for recharge; adapter + `actions.ts` thread the key through. *(iter 4)*
- [x] **S-3b Comp/gift subscription revenue attribution.** Admin plan assignment hardcoded
  `price_paid = 0.00`, making comped subscriptions invisible to revenue/MRR. Per owner: comps must be
  attributed. Fixed `admin_assign_plan` (`subscriptions.rs`) to record the plan's list value for the
  chosen cycle (ANNUALLY → `price_annually`, fallback monthly×12; else `price_monthly`). `price_paid`
  is display/reporting-only — verified it never triggers a charge — so this is safe. *(iter 5)*

## Applications

- [x] **A-1 DB-tool K8s deploy stub — OBSOLETE, not a gap.** Architecture clarified by owner:
  databases / Redis / other shared services now live OUTSIDE k8s and are injected into pods as env
  vars (the managed service-endpoint model). The platform no longer creates DB/Redis pods or
  containers via k8s. So `deploy_db_tool`/`delete_db_tool` (`apps.rs:2415,2480`) and their
  "deploy/tear down via K8s when ready" TODOs are vestigial from the old model — **do NOT implement**.
  Cleanup candidate (flag, don't delete unprompted): the `app_db_tools` table + endpoints, and any
  `app_type` DB-pod creation path in `deployment.rs`, if confirmed unused. *(iter 5 — reclassified)*
- [x] **A-2 Unsupported binding modes — already handled (verified).** `resolve_binding`
  (`templates_deploy.rs`) returns an explicit `BadRequest` for every unsupported provision mode
  (Redis/S3/MQ/SMTP) plus a catch-all `unsupported binding: kind=.. mode=..`. Not a gap. *(iter 6)*
- [x] **A-3 Deploy-time binding quota pre-check — already implemented (verified).**
  `templates_deploy.rs:175-216` computes the distinct-ref delta and calls
  `managed_usage::check_binding_allowed` before creating the app. Not a gap. *(iter 6)*
- [i] **DB provisioning consistent with external model (verified).** `("database","provision")` →
  `provision_new_database` connects to an EXTERNAL `database_clusters` host via
  `sqlx::MySqlPool::connect` and runs `CREATE DATABASE/USER` (logical DB on an external server), then
  injects connection info as env. No k8s pod/container created. Only naming is legacy
  (`crate::k8s::database`, `k8s_secret_name` column) — cosmetic cleanup candidate.
- [ ] **A-4 Project-scoped template management has no UI.** Backend allows project owners to create
  ORG-scoped templates; frontend only does global/admin templates.

## Finance

- [x] **F-1 Overdue-fee user notification.** The daily overdue job charged negative-balance users
  silently. Now `apply_overdue_charges` (`billing.rs`) emails each charged user (non-fatal, after the
  charge commits) via a new `mailer.send_overdue_notice` — shows fee, current balance, currency, and a
  top-up nudge. Platform name + currency fetched once before the loop. *(iter 6)*
- [x] **F-2 Stripe webhook double-credit race (money correctness).** The `status != "PENDING"` check
  read outside the transaction, so two concurrent at-least-once deliveries could both credit the
  wallet. Fixed: `handle_checkout_completed` now does an atomic conditional-claim
  `UPDATE ... WHERE id=? AND status='PENDING'` under a row lock before crediting; the loser matches 0
  rows and bails. *(iter 2)*
- [x] **F-3 Duplicate invoices per period.** The unique-violation mapping only covered `invoice_no`
  (randomly generated), so the documented "invoice for this period already exists" guard never fired.
  Fixed: `admin_generate_invoice` now pre-checks `(user_id, period_start, period_end)` and returns
  Conflict. *(iter 2)*
- [~] **F-4 Top-up per-user rate limit — DROPPED as low-value.** A top-up adds the user's *own* money
  via their *own* Stripe payment; the 3% fee is self-inflicted, not a platform attack vector. Global IP
  rate limiting already guards the endpoint. Not worth the churn unless pending-row DB bloat becomes a
  real concern. *(iter 6)*

## Legacy DB-pod dead-code audit (iter 7)

**Conclusion: the k8s→external-services migration was done cleanly — almost no true dead code.**

- `k8s/deployment.rs` — **no** DB/Redis pod-creation branching at all. The deployer never
  special-cases database types. Clean.
- `k8s/database.rs` — all LIVE and consistent with the external model: `provision_database`/
  `drop_database` connect to EXTERNAL `database_clusters` via SQL (`MySqlPool::connect`, `CREATE/DROP
  DATABASE`); `create_db_secret`/`delete_db_secret` (called by `api/databases.rs`) put external-DB
  creds into a k8s Secret for the app pod. KEEP all.
- Cosmetic legacy naming only: module path `crate::k8s::database`, column `k8s_secret_name`. Harmless.
- **`app_type` DB values** (POSTGRES/MYSQL/MARIADB/MONGODB/REDIS) + `GET .../db-credentials`
  (`apps.rs:1423`): a "database-as-app" user feature; `deployment.rs` does not special-case it, so it's
  ordinary user-driven container deployment, not platform pod-creation. Product decision to deprecate,
  not dead code.
- [x] **`db-tools` reworked into an external-client connection helper** (owner chose "rework to fit
  external model"). The old model deployed a hosted dbgate/phpmyadmin/pgadmin **container** per app via
  k8s — incompatible with external services. Discovered the `DbCredentials` card already surfaces
  creds + a connection URL, so the rework adds *value* without duplication:
  - Backend: removed `list/deploy/get/delete_db_tool` handlers + DeployDbToolRequest (`apps.rs`) and
    their routes (`mod.rs`); migration `027_drop_app_db_tools.sql` drops the now-unused table. Build
    verified green (foreground).
  - Frontend: `db-tools.tsx` rewritten into a "Connect from an external client" card that renders a
    copyable per-dialect CLI command (psql / mysql / mongosh / redis-cli) built from the real
    `db-credentials` response; deleted `db-gate-db-tool.tsx` + `phpmyadmin-db-tool.tsx`; trimmed
    `credentials/actions.ts` to just `getDatabaseCredentials`; removed the adapter `dbTools` block;
    fixed the now-inaccurate `app.dbTools.title/description` copy (en+zh). Touched files typecheck
    clean. *(iter 7)*

### Build-integrity fix (iter 7)
- [x] **api/billing.rs `apply_admin_balance_change` did not compile** (E0382 use-after-move of
  `db_tx`: rollback in the race path moved it before the fall-through `commit`). Introduced in iter 4
  (S-3); masked because concurrent background `cargo check`s raced on the target dir and mis-reported
  success. Restructured to explicit `match insert { Ok => commit, Err => race-recover else propagate }`.
  Verified green with a single foreground build.

## Docker backend parity (iter 9)

Closing K3S↔Docker asymmetries found in the lifecycle trace (all backend-only — the qs-agent already
honors the relevant `RunContainerRequest` fields).

- [x] **`/etc/hosts` mount.** `docker/deployment.rs` now mounts `/etc/hosts` read-only when
  `mount_etc_hosts` is set (added the column to the deploy SELECT/`AppRow`). K3S already did this.
- [x] **Platform image registries.** `deploy_app` now falls back to the `image_registries` row linked
  via `image_registry_id` (app-level creds take precedence) and builds `RegistryAuth` with the
  registry endpoint — parity with the K3S imagePullSecret synthesis. Previously private pulls from a
  platform registry failed on Docker.
- [x] **GPU — verified already implemented (not a gap).** `deploy_app` sets `gpu_count` and the agent
  applies an nvidia `DeviceRequest` (`agent/src/docker_ops.rs`); scheduler filters GPU nodes.
- [x] **Anti-affinity — verified already at parity (not a gap).** K3S applies SOFT anti-affinity
  unconditionally (`pod_spec.rs:229`); the Docker scheduler already prefers distinct nodes.
- [x] **Scale-up incremental (no redeploy).** `deploy_app` body extracted into `deploy_app_inner(add:
  Option<(start_idx, count)>)`: `None` = full redeploy (delete + create `replicas` from 0, unchanged);
  `Some((start,count))` = create only the delta, numbered after existing containers, **without
  deleting/recreating running ones**. `scale_deployment` scale-up now calls it with
  `(current, replicas-current)` — existing replicas stay up (no downtime), parity with K3S in-place
  replica patch. Scheduler picks the least-loaded nodes for the delta (existing containers counted).
- [x] **Builds (GIT source) on Docker.** Implemented per `docs/DESIGN-docker-builds.md` (all phases incl.
  Decision-1=B agent endpoint): `builds.rs` is orchestrator-aware (`run_build_dispatch`); Docker builds
  run **kaniko as a container via the agent** (`run_build_container`), completion polled via a new agent
  `GET /containers/:id/inspect` (exit code), logs proxied from the build container, cancel = stop+remove.
  Registry push auth via optional `config.json` file-mount (+ `registry_insecure` flag); private-repo
  git auth via `GIT_USERNAME`/`GIT_PASSWORD`. Migration `028` adds `build_jobs.node_id`/`container_id`.
  Also fixed the latent image-ref bug in **both** backends: builds now push to the deploy-pulled
  `{registry}/{app}:latest` (+ `:build-{id}`). Agent + backend compile clean.
  Note: agent change (inspect endpoint) requires redistributing the qs-agent binary + re-provisioning
  Docker nodes before Docker builds work end-to-end.
- [ ] **Cordon/drain has no Docker equivalent.** OPEN.

## Registry push-auth closed loop (iter 10)

- [x] **Registry credentials are now settable + used by both builders.** Previously kaniko pushed
  anonymously (only `registry_host` existed) — private registries would fail. Now:
  - `platform.rs`: `registry_host/username/password/insecure` whitelisted in `set_config`;
    `registry_password` treated as sensitive (AES-encrypted on set, masked `***` on list).
  - Build (`builds.rs`): `build_registry_config_json` builds a docker `config.json` from the creds
    (decrypt best-effort, omitted when unset). Docker build mounts it at
    `/kaniko/.docker/config.json` (file-mount); **K8s build now creates a Secret + mounts it** at
    `/kaniko/.docker` (was missing). `--insecure`/`--skip-tls-verify` added when `registry_insecure`.
  - Frontend: new **Image Registry** card in the Platform Config tab (host/username/insecure +
    write-only password; registry keys hidden from the generic key list). Closed loop:
    set (encrypted) → used by both backends.
  - Caveat: still runtime-untested; needs a real registry + migration run to confirm push auth.

### Dropped after verification (NOT bugs)
- Subscription renewal "loop not transactional": each `renew_one` is independently atomic and the due
  query is idempotent; a crash just defers remaining renewals to the next cycle. Correct as-is.
- Renewal "doesn't check `plan.is_active`": grandfathering existing subscribers on a retired plan is
  standard SaaS behavior, not a bug.
