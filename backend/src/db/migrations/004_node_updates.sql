-- Migration 004: node storage path, pod CIDR, and PROVISIONING status

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='storage_path') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `storage_path` VARCHAR(512) NULL AFTER `storage_available`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='pod_cidr') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `pod_cidr` VARCHAR(32) NULL AFTER `storage_path`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- MODIFY COLUMN is safe to re-run (expanding ENUM is always idempotent)
ALTER TABLE cluster_nodes
  MODIFY COLUMN node_status ENUM('PROVISIONING','READY','NOT_READY','UNKNOWN') NOT NULL DEFAULT 'UNKNOWN';
