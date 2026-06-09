-- 030_node_ssh_port.sql
-- Per-node SSH port (default 22 applied in code when NULL). Lets nodes on a
-- non-standard SSH port be provisioned, and is reused on reprovision.
-- Guarded so the migration is idempotent and safe to re-run.

SET @c = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='cluster_nodes'
     AND COLUMN_NAME='ssh_port') = 0,
  'ALTER TABLE cluster_nodes ADD COLUMN ssh_port SMALLINT UNSIGNED NULL AFTER ip_address',
  'SELECT 1');
PREPARE _s FROM @c; EXECUTE _s; DEALLOCATE PREPARE _s;
