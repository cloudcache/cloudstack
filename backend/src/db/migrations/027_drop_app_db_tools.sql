-- 027_drop_app_db_tools.sql
-- Remove the legacy `app_db_tools` table.
--
-- The old model deployed a hosted DB-admin container (dbgate / phpmyadmin /
-- pgadmin) per app via k8s and tracked it here. Under the external-services
-- model the platform no longer creates such containers; users connect to their
-- (external) database with their own client using the connection info surfaced
-- by the Credentials tab. The backend handlers and routes for db-tools have been
-- removed, so this table is now unused.
--
-- Guarded so the migration is idempotent and safe to re-run.

SET @t = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'app_db_tools') = 1,
  'DROP TABLE app_db_tools',
  'SELECT 1');
PREPARE _s FROM @t; EXECUTE _s; DEALLOCATE PREPARE _s;
