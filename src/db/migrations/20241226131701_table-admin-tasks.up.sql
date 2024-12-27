-- Add up migration script here

CREATE TYPE admin_task_initiator AS ENUM ('user', 'system');
CREATE TYPE admin_task_status AS ENUM ('pending', 'in_progress', 'canceled', 'completed', 'failed');

CREATE TABLE admin_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    initiator admin_task_initiator NOT NULL,
    name TEXT NOT NULL,
    metadata JSONB NOT NULL,
    status admin_task_status NOT NULL DEFAULT 'pending',
    enqueued_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX admin_tasks_idx_name_status_enqueued_at ON admin_tasks (name ASC, status ASC, enqueued_at ASC);
CREATE INDEX admin_tasks_idx_updated_at_id ON admin_tasks (updated_at DESC, id ASC);

CREATE OR REPLACE FUNCTION update_admin_task_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_admin_task_updated_at
BEFORE UPDATE ON admin_tasks
FOR EACH ROW
EXECUTE FUNCTION update_admin_task_updated_at();
