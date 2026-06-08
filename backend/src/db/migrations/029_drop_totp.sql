-- 029_drop_totp.sql
-- TOTP/2FA is removed from the application — MFA is delegated to the identity
-- provider (LLDAP). Drop the now-unused credentials table.
-- Guarded so the migration is idempotent and safe to re-run.

SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'totp_credentials') = 1,
  'DROP TABLE totp_credentials',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;
