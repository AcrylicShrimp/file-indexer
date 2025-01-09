-- Add up migration script here

CREATE TABLE admin_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_id UUID NOT NULL REFERENCES admins(id),
    token TEXT NOT NULL UNIQUE,
    logined_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expired_at TIMESTAMP NOT NULL
);

CREATE INDEX admin_sessions_idx_expired_at ON admin_sessions(expired_at);
