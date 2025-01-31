-- Add up migration script here

CREATE TABLE files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    size BIGINT NOT NULL,
    mime_type TEXT NOT NULL,
    uploaded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_ready BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX files_idx_is_ready ON files (is_ready);
CREATE INDEX files_idx_id_is_ready ON files (id, is_ready);
CREATE INDEX files_idx_uploaded_at_id_is_ready ON files (uploaded_at DESC, id ASC, is_ready);
