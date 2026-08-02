-- +goose Up
-- 00002_projects.sql: projects (metadata for one-bare-repo-per-project)

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    repo_dir    TEXT NOT NULL,
    archived_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_projects_archived_at ON projects(archived_at);

-- +goose Down
DROP INDEX idx_projects_archived_at;
DROP TABLE projects;
