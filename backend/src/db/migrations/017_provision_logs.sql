-- ── Provision step logs: real-time tracking of node provisioning ──────────────

CREATE TABLE IF NOT EXISTS node_provision_logs (
    id            BIGINT       NOT NULL AUTO_INCREMENT PRIMARY KEY,
    node_id       CHAR(36)     NOT NULL,
    attempt       INT UNSIGNED NOT NULL DEFAULT 1    COMMENT 'Provision attempt number (incremented on reprovision)',
    step_index    SMALLINT     NOT NULL,
    step_name     VARCHAR(64)  NOT NULL,
    status        VARCHAR(16)  NOT NULL DEFAULT 'RUNNING' COMMENT 'RUNNING, OK, FAILED, SKIPPED, CANCELLED',
    output        MEDIUMTEXT   NULL     COMMENT 'Accumulated stdout+stderr',
    started_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at   DATETIME     NULL,
    INDEX idx_npl_node_attempt (node_id, attempt)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── cluster_nodes: provision progress + cancel flag ──────────────────────────

SET @col_exists = (SELECT COUNT(*)
    FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='provision_step');
SET @stmt = IF(@col_exists = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `provision_step` VARCHAR(64) NULL',
  'SELECT 1');
PREPARE s FROM @stmt; EXECUTE s; DEALLOCATE PREPARE s;

SET @col_exists = (SELECT COUNT(*)
    FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='provision_cancel');
SET @stmt = IF(@col_exists = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `provision_cancel` TINYINT(1) NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE s FROM @stmt; EXECUTE s; DEALLOCATE PREPARE s;

SET @col_exists = (SELECT COUNT(*)
    FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='provision_attempt');
SET @stmt = IF(@col_exists = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `provision_attempt` INT UNSIGNED NOT NULL DEFAULT 0',
  'SELECT 1');
PREPARE s FROM @stmt; EXECUTE s; DEALLOCATE PREPARE s;

SET @col_exists = (SELECT COUNT(*)
    FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes' AND COLUMN_NAME='provision_error');
SET @stmt = IF(@col_exists = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN `provision_error` TEXT NULL',
  'SELECT 1');
PREPARE s FROM @stmt; EXECUTE s; DEALLOCATE PREPARE s;
