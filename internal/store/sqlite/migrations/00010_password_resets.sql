-- +goose Up
-- 00010_password_resets.sql: one-time password reset tokens.

CREATE TABLE password_resets (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    used_at    TEXT
);

CREATE INDEX idx_password_resets_user_id ON password_resets(user_id);
CREATE INDEX idx_password_resets_expires_at ON password_resets(expires_at);

-- +goose Down
DROP INDEX idx_password_resets_expires_at;
DROP INDEX idx_password_resets_user_id;
DROP TABLE password_resets;
