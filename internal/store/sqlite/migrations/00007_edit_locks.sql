-- +goose Up
-- 00007_edit_locks.sql: exclusive per-page edit locks. One user can edit a
-- page at a time; rows are lazily expired by comparing expires_at with the
-- current time, so a crashed/offline editor's lock frees itself after the
-- lease window. Times are Unix milliseconds (UTC).

CREATE TABLE edit_locks (
    project_id   TEXT    NOT NULL,
    path         TEXT    NOT NULL,
    user_id      TEXT    NOT NULL,
    username     TEXT    NOT NULL,
    acquired_at  INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,
    PRIMARY KEY (project_id, path)
);

-- +goose Down
DROP TABLE edit_locks;
