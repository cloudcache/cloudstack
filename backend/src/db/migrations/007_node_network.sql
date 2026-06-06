-- 007_node_network.sql
-- Fixed-IP container networking (Multus/bridge), node metrics cache, GPU info

-- ip_pools.pool_type: widen from ENUM to VARCHAR so we can add VPC / PUBLIC types
-- MODIFY COLUMN is safe to re-run
ALTER TABLE ip_pools
  MODIFY COLUMN pool_type VARCHAR(32) NOT NULL DEFAULT 'LB';

-- ── clusters: VPC/public pool references + node NIC name ─────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='clusters' AND COLUMN_NAME='vpc_pool_id') = 0,
  'ALTER TABLE clusters ADD COLUMN `vpc_pool_id` CHAR(36) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='clusters' AND COLUMN_NAME='pub_pool_id') = 0,
  'ALTER TABLE clusters ADD COLUMN `pub_pool_id` CHAR(36) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='clusters' AND COLUMN_NAME='node_main_iface') = 0,
  'ALTER TABLE clusters ADD COLUMN `node_main_iface` VARCHAR(64) NOT NULL DEFAULT "eth0"',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS
   WHERE CONSTRAINT_SCHEMA=DATABASE() AND CONSTRAINT_NAME='fk_cluster_vpc_pool') = 0,
  'ALTER TABLE clusters ADD CONSTRAINT fk_cluster_vpc_pool FOREIGN KEY (vpc_pool_id) REFERENCES ip_pools(id) ON DELETE SET NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS
   WHERE CONSTRAINT_SCHEMA=DATABASE() AND CONSTRAINT_NAME='fk_cluster_pub_pool') = 0,
  'ALTER TABLE clusters ADD CONSTRAINT fk_cluster_pub_pool FOREIGN KEY (pub_pool_id) REFERENCES ip_pools(id) ON DELETE SET NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── Per-app fixed IP assignments ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS app_ip_allocations (
  id           CHAR(36)    NOT NULL PRIMARY KEY,
  app_id       CHAR(36)    NOT NULL,
  pool_id      CHAR(36)    NOT NULL,
  ip_address   VARCHAR(45) NOT NULL,
  alloc_ref_id CHAR(36)    NOT NULL,
  created_at   DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uk_app_pool (app_id, pool_id),
  CONSTRAINT fk_aia_pool  FOREIGN KEY (pool_id)      REFERENCES ip_pools(id),
  CONSTRAINT fk_aia_alloc FOREIGN KEY (alloc_ref_id) REFERENCES ip_allocations(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── cluster_nodes: metrics cache + NIC detection ──────────────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='main_iface') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `main_iface` VARCHAR(64) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='cpu_used_pct') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `cpu_used_pct` FLOAT NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='mem_used_mb') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `mem_used_mb` BIGINT NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='disk_used_gb') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `disk_used_gb` BIGINT NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='disk_total_gb') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `disk_total_gb` BIGINT NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='load1') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `load1` FLOAT NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='metrics_updated_at') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `metrics_updated_at` DATETIME NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
