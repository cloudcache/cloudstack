-- 022_managed_service_endpoints.sql
-- Per-kind tables for third-party service endpoints that app templates can
-- declare as dependencies (alongside the existing database_clusters and
-- s3_targets). Admins register endpoints; tenants pick them at deploy time.
--
-- Schema mirrors s3_targets style: one table per kind, password is AES-GCM
-- encrypted via the app's crypto service (same as elsewhere).

-- ── MQ (RabbitMQ / similar) ─────────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='mq_endpoints') = 0,
  'CREATE TABLE mq_endpoints (
      id         CHAR(36)     NOT NULL PRIMARY KEY,
      name       VARCHAR(128) NOT NULL,
      host       VARCHAR(256) NOT NULL,
      port       SMALLINT UNSIGNED NOT NULL DEFAULT 5672,
      vhost      VARCHAR(128) NOT NULL DEFAULT ''/'',
      username   VARCHAR(128) NOT NULL,
      password   VARCHAR(1024) NOT NULL COMMENT ''AES-256-GCM encrypted'',
      tls_enabled TINYINT(1)  NOT NULL DEFAULT 0,
      description TEXT        NULL,
      is_active  TINYINT(1)   NOT NULL DEFAULT 1,
      created_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
      UNIQUE KEY uq_mq_name (name)
   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── SMTP relay ──────────────────────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='smtp_endpoints') = 0,
  'CREATE TABLE smtp_endpoints (
      id           CHAR(36)     NOT NULL PRIMARY KEY,
      name         VARCHAR(128) NOT NULL,
      host         VARCHAR(256) NOT NULL,
      port         SMALLINT UNSIGNED NOT NULL DEFAULT 587,
      username     VARCHAR(256) NULL,
      password     VARCHAR(1024) NULL COMMENT ''AES-256-GCM encrypted'',
      from_address VARCHAR(256) NULL,
      tls_enabled  TINYINT(1)   NOT NULL DEFAULT 1,
      description  TEXT         NULL,
      is_active    TINYINT(1)   NOT NULL DEFAULT 1,
      created_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
      UNIQUE KEY uq_smtp_name (name)
   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── Redis (or other K/V cache) ──────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='redis_endpoints') = 0,
  'CREATE TABLE redis_endpoints (
      id         CHAR(36)     NOT NULL PRIMARY KEY,
      name       VARCHAR(128) NOT NULL,
      host       VARCHAR(256) NOT NULL,
      port       SMALLINT UNSIGNED NOT NULL DEFAULT 6379,
      password   VARCHAR(1024) NULL COMMENT ''AES-256-GCM encrypted'',
      db_index   SMALLINT     NOT NULL DEFAULT 0,
      tls_enabled TINYINT(1)  NOT NULL DEFAULT 0,
      description TEXT        NULL,
      is_active  TINYINT(1)   NOT NULL DEFAULT 1,
      created_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
      UNIQUE KEY uq_redis_name (name)
   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
