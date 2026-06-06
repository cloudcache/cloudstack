-- 015: Add provision_error column and schedulable column to cluster_nodes
SET @col_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'cluster_nodes' AND COLUMN_NAME = 'provision_error');
SET @sql = IF(@col_exists = 0,
    'ALTER TABLE cluster_nodes ADD COLUMN provision_error TEXT NULL AFTER node_status',
    'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;

SET @col_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'cluster_nodes' AND COLUMN_NAME = 'schedulable');
SET @sql = IF(@col_exists = 0,
    'ALTER TABLE cluster_nodes ADD COLUMN schedulable TINYINT(1) NOT NULL DEFAULT 1 AFTER provision_error',
    'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;
