-- Migration 013: Bind apps to a specific cluster.
--
-- Previously, cluster was resolved at deploy time from the pool — a restarted
-- app could theoretically land on a different cluster. Now apps store their
-- assigned cluster_id and reuse it across deploys.
--
-- Idempotent on MySQL 5.7.

-- Add cluster_id column (nullable — filled on first deploy)
SET @col_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='cluster_id');
SET @sql = IF(@col_exists = 0,
  'ALTER TABLE apps ADD COLUMN `cluster_id` CHAR(36) NULL AFTER `pool_id`',
  'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;

-- Add FK (only if it doesn't exist yet)
SET @fk_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND CONSTRAINT_NAME='fk_apps_cluster');
SET @sql = IF(@fk_exists = 0,
  'ALTER TABLE apps ADD CONSTRAINT fk_apps_cluster FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE SET NULL',
  'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;

-- Backfill: for apps that already have a pool_id, assign the first active cluster in that pool
UPDATE apps a
  JOIN (
    SELECT pool_id, MIN(id) AS cluster_id
    FROM clusters
    WHERE is_active = 1
    GROUP BY pool_id
  ) c ON c.pool_id = a.pool_id
SET a.cluster_id = c.cluster_id
WHERE a.cluster_id IS NULL AND a.pool_id IS NOT NULL;
