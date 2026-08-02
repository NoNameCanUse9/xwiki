-- +goose Up
-- 00003_agent.sql: agent tokens, idempotency keys, audit log

CREATE TABLE agent_tokens (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    token_hash    TEXT NOT NULL UNIQUE,
    scope         TEXT NOT NULL CHECK (scope IN ('read','write')),
    project_ids   TEXT NOT NULL DEFAULT '[]',   -- JSON array
    path_prefixes TEXT NOT NULL DEFAULT '[]',   -- JSON array
    created_at    TEXT NOT NULL,
    last_used_at  TEXT,
    revoked_at    TEXT
);

CREATE TABLE idempotency_keys (
    key          TEXT NOT NULL,
    project_id   TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    result_json  TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    PRIMARY KEY (key, project_id)
);

CREATE TABLE audit_logs (
    id          TEXT PRIMARY KEY,
    actor_type  TEXT NOT NULL CHECK (actor_type IN ('user','token')),
    actor_id    TEXT NOT NULL,
    project_id  TEXT,
    action      TEXT NOT NULL,
    path        TEXT,
    detail      TEXT,
    request_id  TEXT,
    created_at  TEXT NOT NULL
);

CREATE INDEX idx_agent_tokens_revoked ON agent_tokens(revoked_at);
CREATE INDEX idx_audit_logs_created ON audit_logs(created_at);
CREATE INDEX idx_audit_logs_project ON audit_logs(project_id);

-- +goose Down
DROP INDEX idx_audit_logs_project;
DROP INDEX idx_audit_logs_created;
DROP INDEX idx_agent_tokens_revoked;
DROP TABLE audit_logs;
DROP TABLE idempotency_keys;
DROP TABLE agent_tokens;
