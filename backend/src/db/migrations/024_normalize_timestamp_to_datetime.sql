-- 024_normalize_timestamp_to_datetime.sql
-- Schema-wide normalization: every datetime column in this codebase decodes
-- as Rust `chrono::NaiveDateTime`, which sqlx maps to MySQL `DATETIME`.
-- Six columns slipped through earlier migrations as `TIMESTAMP`; that type
-- mismatch keeps surfacing as 500 errors:
--   "Rust type NaiveDateTime (as SQL type DATETIME) is not compatible with SQL type TIMESTAMP"
--
-- Fix the schema once so we never hunt for this again. Each ALTER is
-- guarded so the migration is idempotent and safe to re-run.
--
-- ── refresh_tokens ──────────────────────────────────────────────────────────
SET @t = IF(
  (SELECT DATA_TYPE FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='refresh_tokens'
     AND COLUMN_NAME='expires_at') = 'timestamp',
  'ALTER TABLE refresh_tokens MODIFY COLUMN expires_at DATETIME NOT NULL',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

SET @t = IF(
  (SELECT DATA_TYPE FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='refresh_tokens'
     AND COLUMN_NAME='created_at') = 'timestamp',
  'ALTER TABLE refresh_tokens MODIFY COLUMN created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── docker_containers ──────────────────────────────────────────────────────
SET @t = IF(
  (SELECT DATA_TYPE FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='docker_containers'
     AND COLUMN_NAME='created_at') = 'timestamp',
  'ALTER TABLE docker_containers MODIFY COLUMN created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

SET @t = IF(
  (SELECT DATA_TYPE FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='docker_containers'
     AND COLUMN_NAME='updated_at') = 'timestamp',
  'ALTER TABLE docker_containers MODIFY COLUMN updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── node_provision_logs ────────────────────────────────────────────────────
SET @t = IF(
  (SELECT DATA_TYPE FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='node_provision_logs'
     AND COLUMN_NAME='started_at') = 'timestamp',
  'ALTER TABLE node_provision_logs MODIFY COLUMN started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

SET @t = IF(
  (SELECT DATA_TYPE FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='node_provision_logs'
     AND COLUMN_NAME='finished_at') = 'timestamp',
  'ALTER TABLE node_provision_logs MODIFY COLUMN finished_at DATETIME NULL',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;
