-- 028_docker_builds.sql
-- Support GIT-source builds on Docker-orchestrator clusters.
--
-- Docker builds run kaniko as a container on a Docker node (via qs-agent), so a
-- build_jobs row needs to remember which node + container ran it (K8s builds
-- keep using k8s_job_name). Also seed the registry_insecure flag used when
-- kaniko pushes to an http registry.
--
-- All statements are guarded so the migration is idempotent and safe to re-run.

-- ── build_jobs.node_id ─────────────────────────────────────────────────────────
SET @c = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='build_jobs'
     AND COLUMN_NAME='node_id') = 0,
  'ALTER TABLE build_jobs ADD COLUMN node_id CHAR(36) NULL AFTER k8s_job_name',
  'SELECT 1');
PREPARE _s FROM @c; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── build_jobs.container_id ────────────────────────────────────────────────────
SET @c = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='build_jobs'
     AND COLUMN_NAME='container_id') = 0,
  'ALTER TABLE build_jobs ADD COLUMN container_id VARCHAR(128) NULL AFTER node_id',
  'SELECT 1');
PREPARE _s FROM @c; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── seed registry_insecure flag (kaniko --insecure when pushing to http) ───────
INSERT IGNORE INTO platform_config (`key`, `value`, description)
VALUES ('registry_insecure', '0', '镜像仓库使用 http(不校验 TLS)时设为 1');
