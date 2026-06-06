-- 010_frontend_gaps.sql
-- Adds fields and tables required by the frontend that were missing from the backend.
-- Volume design: system-managed hostPath volumes (no Longhorn, no PVC objects).
-- The backend derives host_path from configured storage root + project/app/volume IDs.
-- Every worker node has an identical directory structure.

-- ── apps: app_type + network policy columns ──────────────────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='app_type') = 0,
  'ALTER TABLE apps ADD COLUMN `app_type` ENUM("APP","POSTGRES","MYSQL","MARIADB","MONGODB","REDIS") NOT NULL DEFAULT "APP" AFTER `pool_id`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='use_network_policy') = 0,
  'ALTER TABLE apps ADD COLUMN `use_network_policy` TINYINT(1) NOT NULL DEFAULT 0 AFTER `network_policy`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='ingress_network_policy') = 0,
  'ALTER TABLE apps ADD COLUMN `ingress_network_policy` ENUM("ALLOW_ALL","NAMESPACE_ONLY","DENY_ALL","INTERNET_ONLY") NOT NULL DEFAULT "ALLOW_ALL" AFTER `use_network_policy`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='egress_network_policy') = 0,
  'ALTER TABLE apps ADD COLUMN `egress_network_policy` ENUM("ALLOW_ALL","NAMESPACE_ONLY","DENY_ALL","INTERNET_ONLY") NOT NULL DEFAULT "ALLOW_ALL" AFTER `ingress_network_policy`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── System-managed hostPath volumes ──────────────────────────────────────────

CREATE TABLE IF NOT EXISTS app_managed_volumes (
  id                   CHAR(36)      NOT NULL PRIMARY KEY,
  app_id               CHAR(36)      NOT NULL,
  name                 VARCHAR(128)  NOT NULL,
  container_mount_path VARCHAR(512)  NOT NULL,
  host_path            VARCHAR(512)  NOT NULL,
  share_with_others    TINYINT(1)    NOT NULL DEFAULT 0,
  shared_volume_id     CHAR(36)      NULL,
  created_at           DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,

  KEY idx_amv_app (app_id),
  CONSTRAINT fk_amv_app    FOREIGN KEY (app_id)           REFERENCES apps(id)                ON DELETE CASCADE,
  CONSTRAINT fk_amv_shared FOREIGN KEY (shared_volume_id) REFERENCES app_managed_volumes(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── Per-volume backup schedules ───────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS app_volume_backups (
  id              CHAR(36)     NOT NULL PRIMARY KEY,
  volume_id       CHAR(36)     NOT NULL,
  s3_target_id    CHAR(36)     NOT NULL,
  cron_expr       VARCHAR(128) NOT NULL,
  retention_days  SMALLINT UNSIGNED NOT NULL DEFAULT 7,
  use_db_backup   TINYINT(1)   NOT NULL DEFAULT 0,
  is_active       TINYINT(1)   NOT NULL DEFAULT 1,
  last_run_at     DATETIME     NULL,
  last_run_status ENUM('SUCCESS','FAILED','RUNNING') NULL,
  created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,

  KEY idx_avb_volume (volume_id),
  CONSTRAINT fk_avb_volume FOREIGN KEY (volume_id)    REFERENCES app_managed_volumes(id) ON DELETE CASCADE,
  CONSTRAINT fk_avb_s3     FOREIGN KEY (s3_target_id) REFERENCES s3_targets(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── Per-app HTTP basic auth ───────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS app_basic_auth (
  id         CHAR(36)     NOT NULL PRIMARY KEY,
  app_id     CHAR(36)     NOT NULL UNIQUE,
  username   VARCHAR(128) NOT NULL,
  password   VARCHAR(512) NOT NULL,
  created_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  CONSTRAINT fk_aba_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── DB management tool deployments ───────────────────────────────────────────

CREATE TABLE IF NOT EXISTS app_db_tools (
  id          CHAR(36)     NOT NULL PRIMARY KEY,
  app_id      CHAR(36)     NOT NULL,
  tool        ENUM('dbgate','phpmyadmin','pgadmin') NOT NULL,
  status      ENUM('STARTING','RUNNING','STOPPED')  NOT NULL DEFAULT 'STARTING',
  access_url  VARCHAR(512) NULL,
  username    VARCHAR(128) NULL,
  password    VARCHAR(128) NULL,
  created_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,

  UNIQUE KEY uq_dbt_app_tool (app_id, tool),
  CONSTRAINT fk_dbt_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
