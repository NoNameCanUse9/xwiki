-- +goose Up
-- 00004_search.sql: FTS5 full-text search over indexed document snapshots.

-- Content/state table doubles as the FTS external content source.
CREATE TABLE doc_index_state (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  TEXT NOT NULL,
    path        TEXT NOT NULL,
    blob_sha    TEXT NOT NULL,
    content     TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL,
    UNIQUE (project_id, path)
);

CREATE VIRTUAL TABLE doc_search USING fts5(
    content,
    content='doc_index_state',
    content_rowid='id',
    tokenize='unicode61'
);

-- Triggers keep the FTS index in sync with the state table.
-- +goose StatementBegin
CREATE TRIGGER doc_search_ai AFTER INSERT ON doc_index_state BEGIN
    INSERT INTO doc_search(rowid, content) VALUES (new.id, new.content);
END;
-- +goose StatementEnd
-- +goose StatementBegin
CREATE TRIGGER doc_search_ad AFTER DELETE ON doc_index_state BEGIN
    INSERT INTO doc_search(doc_search, rowid, content) VALUES('delete', old.id, old.content);
END;
-- +goose StatementEnd
-- +goose StatementBegin
CREATE TRIGGER doc_search_au AFTER UPDATE ON doc_index_state BEGIN
    INSERT INTO doc_search(doc_search, rowid, content) VALUES('delete', old.id, old.content);
    INSERT INTO doc_search(rowid, content) VALUES (new.id, new.content);
END;
-- +goose StatementEnd

CREATE INDEX idx_doc_index_project ON doc_index_state(project_id);

-- +goose Down
DROP INDEX idx_doc_index_project;
DROP TRIGGER doc_search_au;
DROP TRIGGER doc_search_ad;
DROP TRIGGER doc_search_ai;
DROP TABLE doc_search;
DROP TABLE doc_index_state;
