-- Migration 006: resource pools → clusters → nodes hierarchy
--               apps and database_clusters scoped to a pool

-- ── Resource pools (geographic / logical zones) ───────────────────────────────
CREATE TABLE IF NOT EXISTS resource_pools (
  id           CHAR(36)     NOT NULL,
  name         VARCHAR(64)  NOT NULL,            -- slug: nyc, sjc, eu-west
  display_name VARCHAR(255) NOT NULL,
  region       VARCHAR(128) NULL,                -- optional geographic label
  description  VARCHAR(512) NULL,
  is_active    TINYINT(1)   NOT NULL DEFAULT 1,
  created_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_pool_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── K3s clusters (one or more per pool) ──────────────────────────────────────
CREATE TABLE IF NOT EXISTS clusters (
  id           CHAR(36)     NOT NULL,
  pool_id      CHAR(36)     NOT NULL,
  name         VARCHAR(128) NOT NULL,
  display_name VARCHAR(255) NULL,
  description  VARCHAR(512) NULL,
  k3s_token    VARCHAR(512) NULL                 COMMENT 'AES-256-GCM encrypted; set by admin or auto-generated',
  kubeconfig   MEDIUMTEXT   NULL                 COMMENT 'AES-256-GCM encrypted; written after master provisions',
  is_active    TINYINT(1)   NOT NULL DEFAULT 1,
  created_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_cluster_name (name),
  CONSTRAINT fk_cluster_pool FOREIGN KEY (pool_id) REFERENCES resource_pools(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── cluster_nodes.cluster_id ──────────────────────────────────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='cluster_id') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `cluster_id` CHAR(36) NULL AFTER `id`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.STATISTICS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND INDEX_NAME='idx_node_cluster') = 0,
  'ALTER TABLE cluster_nodes ADD KEY idx_node_cluster (cluster_id)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS
   WHERE CONSTRAINT_SCHEMA=DATABASE() AND CONSTRAINT_NAME='fk_node_cluster') = 0,
  'ALTER TABLE cluster_nodes ADD CONSTRAINT fk_node_cluster FOREIGN KEY (cluster_id) REFERENCES clusters(id)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── apps.pool_id ──────────────────────────────────────────────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND COLUMN_NAME='pool_id') = 0,
  'ALTER TABLE apps ADD COLUMN `pool_id` CHAR(36) NULL AFTER `project_id`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.STATISTICS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps' AND INDEX_NAME='idx_app_pool') = 0,
  'ALTER TABLE apps ADD KEY idx_app_pool (pool_id)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS
   WHERE CONSTRAINT_SCHEMA=DATABASE() AND CONSTRAINT_NAME='fk_app_pool') = 0,
  'ALTER TABLE apps ADD CONSTRAINT fk_app_pool FOREIGN KEY (pool_id) REFERENCES resource_pools(id)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── database_clusters.pool_id ─────────────────────────────────────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='database_clusters' AND COLUMN_NAME='pool_id') = 0,
  'ALTER TABLE database_clusters ADD COLUMN `pool_id` CHAR(36) NULL AFTER `id`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.STATISTICS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='database_clusters' AND INDEX_NAME='idx_dbcluster_pool') = 0,
  'ALTER TABLE database_clusters ADD KEY idx_dbcluster_pool (pool_id)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.REFERENTIAL_CONSTRAINTS
   WHERE CONSTRAINT_SCHEMA=DATABASE() AND CONSTRAINT_NAME='fk_dbcluster_pool') = 0,
  'ALTER TABLE database_clusters ADD CONSTRAINT fk_dbcluster_pool FOREIGN KEY (pool_id) REFERENCES resource_pools(id)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
