-- +goose Up
-- 00006_links.sql: wiki link index for backlinks (rebuilt during reindex)

CREATE TABLE page_links (
    project_id   TEXT NOT NULL,
    source_path  TEXT NOT NULL,
    target_path  TEXT NOT NULL,
    PRIMARY KEY (project_id, source_path, target_path)
);

CREATE INDEX idx_page_links_target ON page_links(project_id, target_path);

-- +goose Down
DROP INDEX idx_page_links_target;
DROP TABLE page_links;
