-- +goose Up
-- 00009_search_trigram.sql: Rebuild FTS5 with trigram tokenizer for CJK support
-- and add path + title columns for weighted search.

-- 1. Add title column to the state table.
ALTER TABLE doc_index_state ADD COLUMN title TEXT NOT NULL DEFAULT '';

-- 2. Drop old triggers and FTS table.
DROP TRIGGER IF EXISTS doc_search_ai;
DROP TRIGGER IF EXISTS doc_search_ad;
DROP TRIGGER IF EXISTS doc_search_au;
DROP TABLE IF EXISTS doc_search;

-- 3. Recreate FTS5 with trigram tokenizer and extra columns.
CREATE VIRTUAL TABLE doc_search USING fts5(
    path,
    title,
    content,
    content='doc_index_state',
    content_rowid='id',
    tokenize='trigram'
);

-- 4. Recreate triggers to keep FTS in sync.
-- +goose StatementBegin
CREATE TRIGGER doc_search_ai AFTER INSERT ON doc_index_state BEGIN
    INSERT INTO doc_search(rowid, path, title, content)
    VALUES (new.id, new.path, new.title, new.content);
END;
-- +goose StatementEnd
-- +goose StatementBegin
CREATE TRIGGER doc_search_ad AFTER DELETE ON doc_index_state BEGIN
    INSERT INTO doc_search(doc_search, rowid, path, title, content)
    VALUES('delete', old.id, old.path, old.title, old.content);
END;
-- +goose StatementEnd
-- +goose StatementBegin
CREATE TRIGGER doc_search_au AFTER UPDATE ON doc_index_state BEGIN
    INSERT INTO doc_search(doc_search, rowid, path, title, content)
    VALUES('delete', old.id, old.path, old.title, old.content);
    INSERT INTO doc_search(rowid, path, title, content)
    VALUES (new.id, new.path, new.title, new.content);
END;
-- +goose StatementEnd

-- 5. Backfill: re-index all existing rows so FTS picks them up.
INSERT INTO doc_search(rowid, path, title, content)
SELECT id, path, title, content FROM doc_index_state;

-- +goose Down
DROP TRIGGER IF EXISTS doc_search_ai;
DROP TRIGGER IF EXISTS doc_search_ad;
DROP TRIGGER IF EXISTS doc_search_au;
DROP TABLE IF EXISTS doc_search;

ALTER TABLE doc_index_state DROP COLUMN title;

CREATE VIRTUAL TABLE doc_search USING fts5(
    content,
    content='doc_index_state',
    content_rowid='id',
    tokenize='unicode61'
);

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

INSERT INTO doc_search(rowid, content) SELECT id, content FROM doc_index_state;
