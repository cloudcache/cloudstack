-- Migration 012: Remove deprecated `network_policy` column from `apps`.
--
-- The old single-field `network_policy` is replaced by the more granular
-- `use_network_policy` + `ingress_network_policy` + `egress_network_policy`
-- added in migration 010.
--
-- Fully idempotent on MySQL 5.7 — all references to `network_policy` are
-- wrapped in dynamic SQL so the migration succeeds even if the column
-- was already dropped.

-- Step 1: Copy old values to new fields (only if column still exists)
SET @col_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='network_policy');
SET @sql = IF(@col_exists > 0,
  'UPDATE apps SET ingress_network_policy = network_policy, egress_network_policy = network_policy, use_network_policy = 1 WHERE network_policy != ''ALLOW_ALL'' AND ingress_network_policy = ''ALLOW_ALL'' AND egress_network_policy = ''ALLOW_ALL''',
  'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;

-- Step 2: Drop the old column (only if it still exists)
SET @col_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='network_policy');
SET @sql = IF(@col_exists > 0,
  'ALTER TABLE apps DROP COLUMN network_policy',
  'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;

-- Step 3: Seed billing currency config key
INSERT IGNORE INTO platform_config (`key`, `value`, description)
VALUES ('billing_currency', 'cny', 'Default billing currency (cny or usd)');
