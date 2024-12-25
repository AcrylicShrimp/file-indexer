-- Add up migration script here

CREATE TABLE file_tags (
    file_id UUID NOT NULL REFERENCES files(id),
    tag TEXT NOT NULL,
    PRIMARY KEY (file_id, tag)
);
