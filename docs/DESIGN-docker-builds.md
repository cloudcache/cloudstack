# Design & Plan: GIT-source builds on Docker clusters

**Status:** IMPLEMENTED (all phases, Decision 1 = B). Builds compile clean (agent + backend). Requires
qs-agent redistribution + Docker-node re-provision for the new inspect endpoint to be live.

## Goal
Let apps with `source_type = 'GIT'` be built on **Docker-orchestrator** clusters. Today builds only
run as **K8s Jobs** (`builds.rs` is K8s-only), so Docker clusters can't build from source — a
deploy from GIT on Docker would try to pull an image that was never produced.

## Current K8s flow (reference)
`api/builds.rs`:
- `trigger_build` → validates GIT source, rejects concurrent builds, inserts `build_jobs` (PENDING),
  spawns `run_build_job`.
- `run_build_job` → creates a **K8s Job** running **kaniko** (`gcr.io/kaniko-project/executor`) with
  `--context=git://{git_url}#refs/heads/{branch}` and `--destination={image_tag}`, `--cache=true`.
  Sets status RUNNING. Spawns `watch_build_completion` (polls Job conditions every 30s, ≤60min).
- `build_logs` → SSE from the build pod via kube `log_stream`.
- `cancel_build` → status CANCELLED + delete K8s Job.

## Facts discovered (ground truth)
- **Agent already runs containers** via bollard (`agent/docker_ops.rs::run_container`): honors image
  pull (`registry_auth`), env, volumes, **file_mounts** (written to disk + bind-mounted), restart
  policy, `/containers/:id/logs` (SSE), stop, remove. **No** inspect / exit-code / wait endpoint;
  `GET /containers` returns `state` ("running"/"exited") + `status` ("Exited (0) …") strings.
- `build_jobs` schema has a K8s-specific `k8s_job_name VARCHAR(256) NULL`; no node/container columns.
- **Image-ref mismatch (latent, affects both orchestrators):** deploy pulls
  `{registry_host}/{app.name}:latest` (`k8s/deployment.rs`, `docker/deployment.rs:170`), but
  `run_build_job` pushes to `{container_image|localhost/qs/app_id}:build-{id8}`. A GIT build's output
  tag does not match what deploy pulls. The Docker design will push to the **deploy-expected ref**, and
  we should fix K8s to match (flagged separately).
- **No registry auth is mounted for kaniko** today (only `registry_host` config key exists; no
  insecure/push-cred keys). Works only if the registry is unauthenticated/in-cluster. The Docker design
  must handle push creds + insecure-registry explicitly.

## Proposed design

**Builder = kaniko run as a Docker container via the agent.** Same tool/args as K8s, so build output
and behavior stay consistent. Kaniko is daemonless (no docker socket, no privileged), so it runs as an
ordinary container; it clones the git context itself and pushes the result.

`api/builds.rs` becomes **orchestrator-aware** (branch on `cluster_orchestrator`, like `apps.rs`):

1. **trigger_build** (shared): unchanged validation + `build_jobs` insert; then dispatch:
   - K3S → `run_build_job` (existing).
   - DOCKER → `run_build_container` (new).
2. **run_build_container** (new):
   - Pick a READY node in the cluster (reuse `docker::scheduler::pick_nodes(count=1)`).
   - Compute destination = `{registry_host}/{app.name}:latest` (+ optionally a `:build-{id8}` tag).
   - Build a `RunContainerRequest`: image `gcr.io/kaniko-project/executor:latest`, `restart_policy:"no"`,
     args `--context=git://{git_url}#refs/heads/{branch}`, `--destination=…`, `--cache=true`
     (+ `--insecure`/`--insecure-pull` when `registry_insecure` config set), and a **file_mount** at
     `/kaniko/.docker/config.json` containing registry auth (built from platform registry creds) when
     present. Container name `qs-build-{id8}`.
   - `agent.run_container(...)`; persist node_id + container_id; status RUNNING + image_tag.
   - Spawn `watch_build_completion_docker`.
3. **watch_build_completion_docker** (new): poll the agent every ~15–30s for the build container's
   terminal state → SUCCEEDED (exit 0) / FAILED (non-zero or gone) / timeout. See completion-detection
   options below.
4. **build_logs** (branch): DOCKER → proxy `GET {node}/containers/{cid}/logs` (reuse the app-log proxy
   pattern in `docker/deployment.rs::log_stream`) as SSE.
5. **cancel_build** (branch): DOCKER → agent stop + remove the build container.

### Decision 1 — completion detection (pick one)
- **(A) No agent change (recommended for phase 1):** poll `GET /containers` (already returns `state` +
  `status`), find `qs-build-{id8}`, treat `state=="exited"` as done; parse exit code from `status`
  ("Exited (N) …") → 0 = SUCCEEDED else FAILED. Ships entirely backend-side; no agent redistribution.
  Slightly fragile (string parse) and exit container is only visible because agent lists `all=true`.
- **(B) Add agent `GET /containers/:id/inspect`** → `{state, exit_code, oom_killed}` (bollard
  `inspect_container`). Precise, clean. **Cost:** rebuild + redistribute the `qs-agent` binary and
  re-provision (or re-pull) existing Docker nodes — a real operational step.

Recommendation: **(A) first** to deliver value without touching node binaries; add **(B)** as a fast
follow-up for precise exit codes (and reuse it to improve app status sync too).

### Decision 2 — registry auth & insecure
- Add platform_config keys `registry_insecure` (bool) and reuse existing registry creds. Build a
  `config.json` docker-auth blob from the push-registry username/password (encrypted in DB) and inject
  as the kaniko `/kaniko/.docker/config.json` file_mount. If the registry is unauthenticated, omit.
- This also lets us **fix the K8s path** to mount the same secret (separate small change).

### Decision 3 — image ref reconciliation
- Push to `{registry_host}/{app.name}:latest` so the existing deploy path finds it; also tag
  `:build-{id8}` for traceability. Store the `:latest` ref (or both) in `build_jobs.image_tag`.
- Note + fix the K8s `run_build_job` to use the same destination (currently inconsistent).

### Decision 4 — schema
- Migration `028_build_node`: add `node_id CHAR(36) NULL` and `container_id VARCHAR(128) NULL` to
  `build_jobs` (Docker builds). Keep `k8s_job_name` for K8s. No FK churn needed (nullable).
- Rename is avoided to keep the change additive.

### Decision 5 — node selection & concurrency
- One build container on one node (`pick_nodes(count=1)`). Concurrency is already gated per-app by the
  existing "build already in progress" check. Build resource limits: small CPU/mem caps (config-driven,
  default e.g. 2000 mcore / 4096 MB) to avoid starving app workloads.

## Phased implementation plan (each phase compiles + is verifiable)

- **Phase 0 — prerequisites/flags (backend-only)**
  - Migration `028_build_node` (node_id, container_id).
  - platform_config: `registry_insecure` (default false). Confirm push-registry creds source.
  - Verify: `cargo check`; migration is guarded/idempotent.
- **Phase 1 — Docker build happy path (backend-only, completion via option A)**
  - `builds.rs`: orchestrator branch in `trigger_build`; `run_build_container`;
    `watch_build_completion_docker` (poll `/containers`); registry `config.json` file_mount; destination
    = deploy-expected ref.
  - Verify: `cargo check`; manual run on a Docker cluster — trigger build, container appears, image
    pushed, status → SUCCEEDED, deploy pulls it.
- **Phase 2 — logs + cancel for Docker (backend-only)**
  - `build_logs` + `cancel_build` orchestrator branches (reuse agent log proxy / stop+remove).
  - Verify: SSE logs stream; cancel stops the container and marks CANCELLED.
- **Phase 3 — K8s consistency fixes (backend-only)**
  - Fix `run_build_job` destination to `{registry_host}/{app.name}:latest`; mount registry auth secret.
  - Verify: K8s build output matches deploy pull ref.
- **Phase 4 (optional) — agent inspect endpoint (agent + backend)**
  - Add `GET /containers/:id/inspect`; switch `watch_build_completion_docker` to exact exit codes.
  - Verify: rebuild agent, re-provision a node, precise SUCCEEDED/FAILED.

## Risks / open questions
- **Registry reachability from Docker nodes** (push) and **insecure (http) registries** — needs the
  `registry_insecure` flag + creds; confirm the deployment's registry topology.
- **Agent redistribution** is the gating cost for any agent-side change (phase 4) — phases 1–3 avoid it.
- **kaniko image availability** on the node's registry/network (it pulls `kaniko-project/executor`).
- **Git auth** for private repos: app has `git_token` (encrypted). kaniko git context supports creds
  via `GIT_USERNAME`/`GIT_PASSWORD` env or URL — must inject for private repos (parity check: does the
  K8s path handle private-repo auth today? It does **not** appear to — flag as shared gap).
- **Build cache**: K8s kaniko uses `--cache=true` (registry-backed). Same for Docker; depends on
  registry write access.

## Recommendation
Proceed **Phases 0–2** first (backend-only, no agent redistribution): delivers working Docker GIT
builds + logs + cancel. Then Phase 3 (K8s consistency) and optionally Phase 4 (precise exit codes).
