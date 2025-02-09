-- Add up migration script here

CREATE TABLE collection_tags (
    collection_id UUID NOT NULL REFERENCES collections(id),
    tag TEXT NOT NULL,
    PRIMARY KEY (collection_id, tag)
);
