use crate::interfaces::dto;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum AdminTaskServiceError {
    #[error("database error: {0:#?}")]
    DbError(#[from] sqlx::Error),
}

pub struct AdminTaskService {
    db_pool: PgPool,
}

impl AdminTaskService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn get_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<dto::AdminTask>, AdminTaskServiceError> {
        let task = sqlx::query_as!(
            row_types::AdminTask,
            "
SELECT
    id,
    initiator AS \"initiator:_\",
    name,
    metadata,
    status AS \"status:_\",
    enqueued_at,
    updated_at
FROM admin_tasks
WHERE id = $1",
            task_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(task.map(|task| task.into()))
    }

    pub async fn list_tasks(
        &self,
        limit: usize,
        cursor: Option<AdminTaskCursor>,
    ) -> Result<Vec<dto::AdminTaskPreview>, AdminTaskServiceError> {
        let admin_tasks = match cursor {
            Some(cursor) => {
                sqlx::query_as!(
                    row_types::AdminTaskPreview,
                    "
SELECT id, initiator AS \"initiator:_\", name, status AS \"status:_\", enqueued_at, updated_at
FROM admin_tasks
WHERE id > $1 AND updated_at <= $2
ORDER BY updated_at DESC, id ASC
LIMIT $3",
                    cursor.id,
                    cursor.updated_at.naive_utc(),
                    limit as i64
                )
                .fetch_all(&self.db_pool)
                .await
            }
            None => {
                sqlx::query_as!(
                    row_types::AdminTaskPreview,
                    "
SELECT id, initiator AS \"initiator:_\", name, status AS \"status:_\", enqueued_at, updated_at
FROM admin_tasks
ORDER BY updated_at DESC, id ASC
LIMIT $1",
                    limit as i64
                )
                .fetch_all(&self.db_pool)
                .await
            }
        }?;

        if admin_tasks.is_empty() {
            return Ok(vec![]);
        }

        Ok(admin_tasks.into_iter().map(|task| task.into()).collect())
    }

    pub async fn enqueue_task(
        &self,
        initiator: dto::AdminTaskInitiator,
        name: &str,
        metadata: &Value,
        status: Option<dto::AdminTaskStatus>,
    ) -> Result<(), AdminTaskServiceError> {
        let admin_task_id = match status {
            Some(status) => {
                sqlx::query_as!(
                    row_types::CreatingAdminTask,
                    "
INSERT INTO admin_tasks (initiator, name, metadata, status)
VALUES ($1, $2, $3, $4)
RETURNING id, enqueued_at, updated_at
",
                    initiator as _,
                    name,
                    metadata,
                    status as _,
                )
                .fetch_one(&self.db_pool)
                .await?
            }
            None => {
                sqlx::query_as!(
                    row_types::CreatingAdminTask,
                    "
INSERT INTO admin_tasks (initiator, name, metadata)
VALUES ($1, $2, $3)
RETURNING id, enqueued_at, updated_at
",
                    initiator as _,
                    name,
                    metadata,
                )
                .fetch_one(&self.db_pool)
                .await?
            }
        };

        Ok(())
    }

    pub async fn update_task_status(
        &self,
        task_id: Uuid,
        status: dto::AdminTaskStatus,
    ) -> Result<(), AdminTaskServiceError> {
        sqlx::query!(
            "UPDATE admin_tasks SET status = $1 WHERE id = $2",
            status as _,
            task_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}

pub struct AdminTaskCursor {
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
}

mod row_types {
    use crate::interfaces::dto;
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    pub struct AdminTaskPreview {
        pub id: Uuid,
        pub initiator: dto::AdminTaskInitiator,
        pub name: String,
        pub status: dto::AdminTaskStatus,
        pub enqueued_at: NaiveDateTime,
        pub updated_at: NaiveDateTime,
    }

    impl From<AdminTaskPreview> for dto::AdminTaskPreview {
        fn from(task: AdminTaskPreview) -> Self {
            Self {
                id: task.id,
                initiator: task.initiator,
                name: task.name,
                status: task.status,
                enqueued_at: task.enqueued_at.and_utc(),
                updated_at: task.updated_at.and_utc(),
            }
        }
    }

    pub struct AdminTask {
        pub id: Uuid,
        pub initiator: dto::AdminTaskInitiator,
        pub name: String,
        pub metadata: serde_json::Value,
        pub status: dto::AdminTaskStatus,
        pub enqueued_at: NaiveDateTime,
        pub updated_at: NaiveDateTime,
    }

    impl From<AdminTask> for dto::AdminTask {
        fn from(task: AdminTask) -> Self {
            Self {
                id: task.id,
                initiator: task.initiator,
                name: task.name,
                metadata: task.metadata,
                status: task.status,
                enqueued_at: task.enqueued_at.and_utc(),
                updated_at: task.updated_at.and_utc(),
            }
        }
    }

    pub struct CreatingAdminTask {
        pub id: Uuid,
        pub enqueued_at: NaiveDateTime,
        pub updated_at: NaiveDateTime,
    }
}