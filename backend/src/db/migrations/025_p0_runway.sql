-- 025_p0_runway.sql
-- P0 release blockers: email verification + private registry pull-secret
-- + subscription renewal audit log. Idempotent, additive only — existing
-- rows default to a state that doesn't break old flows.

-- ── Email verification (P0.2) ───────────────────────────────────────────────
-- Existing users default to verified (1) so they aren't locked out at the
-- migration boundary; only new signups land in unverified state (0).
SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='users'
     AND COLUMN_NAME='email_verified') = 0,
  'ALTER TABLE users ADD COLUMN email_verified TINYINT(1) NOT NULL DEFAULT 1',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='users'
     AND COLUMN_NAME='email_verification_token') = 0,
  'ALTER TABLE users ADD COLUMN email_verification_token VARCHAR(64) NULL',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='users'
     AND COLUMN_NAME='email_verification_sent_at') = 0,
  'ALTER TABLE users ADD COLUMN email_verification_sent_at DATETIME NULL',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── Private registry → app pull-secret bridge (P0.3) ────────────────────────
-- Templates already carry `image_registry_id`; copy it to the app row on
-- deploy so the k8s deployer can ensure an imagePullSecret without re-querying
-- the template.
SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='apps'
     AND COLUMN_NAME='image_registry_id') = 0,
  'ALTER TABLE apps ADD COLUMN image_registry_id CHAR(36) NULL COMMENT ''FK to image_registries; resolved to imagePullSecret on deploy''',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── Subscription renewal audit log (P0.1 + P0.4) ────────────────────────────
-- Every monthly renewal attempt — success or failure — is recorded here.
-- One row per (subscription, attempted_at). Used for invoicing + debug.
SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='subscription_renewals') = 0,
  'CREATE TABLE subscription_renewals (
      id              CHAR(36)      NOT NULL PRIMARY KEY,
      subscription_id CHAR(36)      NOT NULL,
      user_id         CHAR(36)      NOT NULL,
      plan_id         CHAR(36)      NOT NULL,
      billing_cycle   VARCHAR(16)   NOT NULL,
      attempted_at    DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
      amount          DECIMAL(10,2) NOT NULL,
      status          VARCHAR(32)   NOT NULL COMMENT ''CHARGED | INSUFFICIENT_FUNDS | EXPIRED | RENEWED_FREE | ERROR'',
      balance_before  DECIMAL(10,2) NULL,
      balance_after   DECIMAL(10,2) NULL,
      previous_expires_at DATETIME  NULL,
      new_expires_at  DATETIME      NULL,
      error_message   TEXT          NULL,
      INDEX idx_sr_sub (subscription_id),
      INDEX idx_sr_user (user_id),
      INDEX idx_sr_attempted (attempted_at)
   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;
