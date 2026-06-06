-- Migration 014: Add local password authentication.
--
-- LDAP is optional. Without it, admins and self-registered users can still
-- log in with a local password (argon2 hash stored in `users.password_hash`).
--
-- On first boot the platform has no users. We seed a default admin via
-- platform_config so the operator can log in without LDAP.

-- Add password_hash column (nullable — NULL means LDAP-only user)
SET @col_exists = (SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='users' AND COLUMN_NAME='password_hash');
SET @sql = IF(@col_exists = 0,
  'ALTER TABLE users ADD COLUMN `password_hash` VARCHAR(512) NULL AFTER `ldap_gid`',
  'SELECT 1');
PREPARE stmt FROM @sql; EXECUTE stmt; DEALLOCATE PREPARE stmt;

-- Seed default admin credentials config (operator changes on first login).
-- Password is NOT stored here — it's set by the app on first boot if no admin exists.
INSERT IGNORE INTO platform_config (`key`, `value`, description)
VALUES ('admin_bootstrap_done', '0', 'Set to 1 after initial admin account is created');
