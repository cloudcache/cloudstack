-- 026_wallet_tx_idempotency.sql
-- Audit hardening for admin wallet operations (gifts / recharge / adjustments).
--
-- wallet_transactions is the append-only financial ledger (no UPDATE/DELETE in
-- the codebase). To guarantee UNIQUENESS of an admin balance operation — so a
-- retried or double-submitted gift cannot create two ledger entries / double the
-- balance — we add a client-supplied idempotency key with a UNIQUE index.
--
-- The column is NULLable: legacy rows and non-admin flows (Stripe top-up,
-- metered deductions) carry no key. MySQL unique indexes allow multiple NULLs,
-- so only explicitly-keyed admin operations are de-duplicated.
--
-- Both ALTERs are guarded so the migration is idempotent and safe to re-run.

-- ── add column ───────────────────────────────────────────────────────────────
SET @c = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='wallet_transactions'
     AND COLUMN_NAME='idempotency_key') = 0,
  'ALTER TABLE wallet_transactions ADD COLUMN idempotency_key VARCHAR(64) NULL COMMENT ''客户端幂等键: 防止管理员重复提交造成重复入账'' AFTER ref_id',
  'SELECT 1');
PREPARE _s FROM @c; EXECUTE _s; DEALLOCATE PREPARE _s;

-- ── add unique index ─────────────────────────────────────────────────────────
SET @i = IF(
  (SELECT COUNT(*) FROM information_schema.STATISTICS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='wallet_transactions'
     AND INDEX_NAME='uq_wallet_tx_idem') = 0,
  'ALTER TABLE wallet_transactions ADD UNIQUE KEY uq_wallet_tx_idem (idempotency_key)',
  'SELECT 1');
PREPARE _s FROM @i; EXECUTE _s; DEALLOCATE PREPARE _s;
