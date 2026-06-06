-- 019_flatten_network.sql
-- Flatten network: single pool per cluster (replaces vpc_pool_id + pub_pool_id)
-- Also adds ip_pool_id to clusters for bridge CNI configuration.

-- ── clusters.ip_pool_id (the single flat L2 pool) ────────────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='clusters' AND COLUMN_NAME='ip_pool_id') = 0,
  'ALTER TABLE clusters ADD COLUMN `ip_pool_id` CHAR(36) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS
   WHERE CONSTRAINT_SCHEMA=DATABASE() AND CONSTRAINT_NAME='fk_cluster_ip_pool') = 0,
  'ALTER TABLE clusters ADD CONSTRAINT fk_cluster_ip_pool FOREIGN KEY (ip_pool_id) REFERENCES ip_pools(id) ON DELETE SET NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- Migrate existing data: vpc_pool_id takes priority
UPDATE clusters SET ip_pool_id = COALESCE(vpc_pool_id, pub_pool_id)
WHERE ip_pool_id IS NULL AND (vpc_pool_id IS NOT NULL OR pub_pool_id IS NOT NULL);
