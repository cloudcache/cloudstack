-- 023_managed_binding_quotas.sql
-- Phase 2c: per-kind quotas on managed-service bindings.
--
-- Existing 003 schema already covers:
--   subscription_plans.quota_db_instance_count
--   projects.quota_db_instances
-- This migration adds matching pairs for the 4 new kinds shipped in P2:
-- MQ, SMTP, Redis, S3-bindings.  `0` = unlimited (matches existing convention).

-- ── subscription_plans columns ──────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='subscription_plans'
     AND COLUMN_NAME='quota_mq_binding_count') = 0,
  'ALTER TABLE subscription_plans ADD COLUMN quota_mq_binding_count INT UNSIGNED NOT NULL DEFAULT 0
    COMMENT ''Max distinct MQ endpoints a project may bind (0=unlimited)''',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='subscription_plans'
     AND COLUMN_NAME='quota_smtp_binding_count') = 0,
  'ALTER TABLE subscription_plans ADD COLUMN quota_smtp_binding_count INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='subscription_plans'
     AND COLUMN_NAME='quota_redis_binding_count') = 0,
  'ALTER TABLE subscription_plans ADD COLUMN quota_redis_binding_count INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='subscription_plans'
     AND COLUMN_NAME='quota_s3_binding_count') = 0,
  'ALTER TABLE subscription_plans ADD COLUMN quota_s3_binding_count INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── projects columns ────────────────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects'
     AND COLUMN_NAME='quota_mq_bindings') = 0,
  'ALTER TABLE projects ADD COLUMN quota_mq_bindings INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects'
     AND COLUMN_NAME='quota_smtp_bindings') = 0,
  'ALTER TABLE projects ADD COLUMN quota_smtp_bindings INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects'
     AND COLUMN_NAME='quota_redis_bindings') = 0,
  'ALTER TABLE projects ADD COLUMN quota_redis_bindings INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects'
     AND COLUMN_NAME='quota_s3_bindings') = 0,
  'ALTER TABLE projects ADD COLUMN quota_s3_bindings INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
