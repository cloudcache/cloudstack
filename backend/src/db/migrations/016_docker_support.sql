-- Migration 016: Docker orchestrator support
-- Adds orchestrator column to clusters, agent_port to nodes,
-- and docker_containers table for tracking Docker-managed containers.

-- ── clusters.orchestrator ────────────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='clusters' AND COLUMN_NAME='orchestrator') = 0,
  'ALTER TABLE clusters ADD COLUMN `orchestrator` VARCHAR(16) NOT NULL DEFAULT ''K3S'' AFTER `node_main_iface`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── cluster_nodes.agent_port ─────────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='agent_port') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `agent_port` SMALLINT UNSIGNED NOT NULL DEFAULT 9800',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── docker_containers ────────────────────────────────────────────────────────
SET @_sql = IF(
  (SELECT COUNT(*) FROM INFORMATION_SCHEMA.TABLES
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='docker_containers') = 0,
  'CREATE TABLE docker_containers (
    id             VARCHAR(36)   NOT NULL,
    app_id         VARCHAR(36)   NOT NULL,
    node_id        VARCHAR(36)   NOT NULL,
    container_id   VARCHAR(64)   NOT NULL COMMENT ''Docker container hex ID'',
    container_name VARCHAR(128)  NOT NULL COMMENT ''qs-{app_name}-{index}'',
    image          VARCHAR(512)  NOT NULL,
    status         VARCHAR(32)   NOT NULL DEFAULT ''CREATING'' COMMENT ''CREATING, RUNNING, STOPPED, FAILED, REMOVING'',
    ip_address     VARCHAR(45)   NULL     COMMENT ''Fixed IP within Docker bridge network'',
    host_port_map  JSON          NULL     COMMENT ''{"8080/tcp": 30100}'',
    error_message  TEXT          NULL,
    created_at     DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at     DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    INDEX idx_dc_app (app_id),
    INDEX idx_dc_node (node_id),
    INDEX idx_dc_status (status),
    UNIQUE INDEX idx_dc_container (container_id)
  ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
