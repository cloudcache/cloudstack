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
- [ ] **U-1b `update_profile` (display name / identity) still a 403 stub** (`profile.rs`). Separate
  from passwords — identity fields are currently admin/LDAP-managed by design. Revisit only if
  self-service identity edits are wanted.
- [ ] **U-2 Password reset not gated on email verification.** `forgot_password` issues a reset link to
  any active user incl. unverified ones; after reset they log in then immediately hit the
  EMAIL_NOT_VERIFIED block. Consider auto-verifying on successful reset, or gating reset.
- [ ] **U-3 TOTP enable requires no password re-auth** (`auth.rs` `totp_setup`/`totp_verify`). An open
  session can enable 2FA and lock out the real owner. Lower priority; adds friction — confirm intent.
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
- **`db-tools` feature** (`deploy_db_tool` stub + `app_db_tools` table + routes + FULL frontend UI:
  Credentials tab dbgate/phpmyadmin/db-tools components, adapter, i18n): backend is a non-functional
  stub (writes a row, status stuck STARTING). Deploying a phpmyadmin/dbgate **container** via k8s
  contradicts the external-services model → conceptually obsolete. BUT it has full UI wiring →
  **removing it is a feature-removal decision, NOT silent dead-code cleanup.** ⏳ AWAITING OWNER CALL:
  remove / rework-for-external-model / leave-as-known-disabled.

### Build-integrity fix (iter 7)
- [x] **api/billing.rs `apply_admin_balance_change` did not compile** (E0382 use-after-move of
  `db_tx`: rollback in the race path moved it before the fall-through `commit`). Introduced in iter 4
  (S-3); masked because concurrent background `cargo check`s raced on the target dir and mis-reported
  success. Restructured to explicit `match insert { Ok => commit, Err => race-recover else propagate }`.
  Verified green with a single foreground build.

### Dropped after verification (NOT bugs)
- Subscription renewal "loop not transactional": each `renew_one` is independently atomic and the due
  query is idempotent; a crash just defers remaining renewals to the next cycle. Correct as-is.
- Renewal "doesn't check `plan.is_active`": grandfathering existing subscribers on a retired plan is
  standard SaaS behavior, not a bug.
