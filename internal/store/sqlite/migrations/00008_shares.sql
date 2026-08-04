-- +goose Up
-- 00008_shares.sql: per-page share links (/share/{token}). A share pins a
-- single document (project + path) and is served publicly as rendered HTML,
-- so a page can be shared without exposing the full docs URL.

CREATE TABLE shares (
    token       TEXT    PRIMARY KEY,
    project_id  TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    created_by  TEXT    NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE INDEX idx_shares_project ON shares(project_id, path);

-- +goose Down
DROP TABLE shares;
