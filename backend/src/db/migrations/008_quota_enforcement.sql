-- 008_quota_enforcement.sql
-- App quota enforcement: SUSPENDED status, violation log

-- Add SUSPENDED to app status ENUM.
-- PAUSED  = user-initiated (user can resume anytime)
-- SUSPENDED = system-initiated (quota exceeded; blocked until quota freed)
ALTER TABLE apps
  MODIFY COLUMN status
    ENUM('STOPPED','DEPLOYING','RUNNING','FAILED','PAUSED','SUSPENDED')
    NOT NULL DEFAULT 'STOPPED';

-- Quota violation log — one row per event (warn or enforce action taken).
CREATE TABLE IF NOT EXISTS quota_violations (
  id              CHAR(36)    NOT NULL PRIMARY KEY,
  project_id      CHAR(36)    NOT NULL,
  app_id          CHAR(36)    NULL     COMMENT 'NULL = project-level event',
  -- dimension: cpu_mcores | mem_mb | app_count
  dimension       VARCHAR(16) NOT NULL,
  used_value      BIGINT      NOT NULL,
  quota_value     BIGINT      NOT NULL,
  pct_used        FLOAT       NOT NULL,
  -- action: warn | suspend | block
  action          VARCHAR(16) NOT NULL,
  resolved_at     DATETIME    NULL     COMMENT 'Set when usage drops back under threshold',
  created_at      DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,

  KEY idx_qv_project (project_id, created_at),
  CONSTRAINT fk_qv_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Platform config for quota behaviour
INSERT IGNORE INTO platform_config (`key`, `value`, description) VALUES
  ('quota_warn_pct',    '80',  'Usage % that triggers a warning (0 = disabled)'),
  ('quota_hard_pct',    '100', 'Usage % that triggers auto-suspension (0 = disabled)'),
  ('quota_check_enabled', '1', 'Master switch: 1 = enforce quota, 0 = track only');
