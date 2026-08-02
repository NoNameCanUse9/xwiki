-- +goose Up
-- 00005_users_disable.sql: account disable support

ALTER TABLE users ADD COLUMN disabled_at TEXT;

-- +goose Down
ALTER TABLE users DROP COLUMN disabled_at;
