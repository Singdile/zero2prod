-- 将整个迁移过程放入一个事务中，以确保原子化地成功或者失败
-- `sqlx` 并不会自动进行原子化处理
BEGIN;
  --为历史记录回填 `status`
    UPDATE  subscriptions
      SET status = 'confirmed'
      WHERE status IS NULL;
    --让`status`不为空
    ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
COMMIT;
