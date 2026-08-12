-- +goose Up
ALTER TABLE projects ADD COLUMN deleted_at TEXT;
CREATE INDEX idx_projects_deleted_at ON projects(deleted_at);

CREATE TABLE project_index_state (
    project_id  TEXT PRIMARY KEY,
    revision    TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL CHECK(status IN ('dirty', 'clean', 'error')),
    updated_at  TEXT NOT NULL,
    last_error  TEXT NOT NULL DEFAULT ''
);

-- +goose Down
DROP TABLE project_index_state;
DROP INDEX idx_projects_deleted_at;
ALTER TABLE projects DROP COLUMN deleted_at;
