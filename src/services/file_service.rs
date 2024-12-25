use crate::interfaces::dto;
use chrono::DateTime;
use futures::future::{try_join, try_join_all};
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum FileServiceError {
    #[error("database error: {0:#?}")]
    DbError(#[from] sqlx::Error),
}

pub struct FileService {
    db_pool: sqlx::PgPool,
}

impl FileService {
    pub fn new(db_pool: sqlx::PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn get_file(&self, file_id: Uuid) -> Result<Option<dto::File>, FileServiceError> {
        let file_task = sqlx::query_as!(
            row_types::File,
            "
SELECT id, name, size, mime_type, uploaded_at
FROM files
WHERE id = $1
            ",
            file_id
        )
        .fetch_optional(&self.db_pool);
        let file_tags_task = sqlx::query_as!(
            row_types::FileTag,
            "
SELECT tag
FROM file_tags
WHERE file_id = $1
        ",
            file_id
        )
        .fetch_all(&self.db_pool);
        let (file, file_tags) = try_join(file_task, file_tags_task).await?;

        Ok(file.map(|file| dto::File::from((file, file_tags))))
    }

    pub async fn list_files(
        &self,
        limit: usize,
        cursor: Option<FileCursor>,
    ) -> Result<Vec<dto::File>, FileServiceError> {
        let files = match cursor {
            Some(cursor) => {
                sqlx::query_as!(
                    row_types::File,
                    "
SELECT id, name, size, mime_type, uploaded_at
FROM files
WHERE uploaded_at <= $1 AND id > $2
ORDER BY uploaded_at DESC, id ASC
LIMIT $3
                ",
                    cursor.uploaded_at.naive_utc(),
                    cursor.id,
                    limit as i64
                )
                .fetch_all(&self.db_pool)
                .await
            }
            None => {
                sqlx::query_as!(
                    row_types::File,
                    "
SELECT id, name, size, mime_type, uploaded_at
FROM files
ORDER BY uploaded_at DESC, id ASC
LIMIT $1
                ",
                    limit as i64
                )
                .fetch_all(&self.db_pool)
                .await
            }
        }?;
        let file_tags_tasks = files.iter().map(|file| {
            sqlx::query_as!(
                row_types::FileTag,
                "
SELECT tag
FROM file_tags
WHERE file_id = $1
ORDER BY tag ASC
            ",
                file.id
            )
            .fetch_all(&self.db_pool)
        });
        let file_tags = try_join_all(file_tags_tasks).await?;

        Ok(files
            .into_iter()
            .zip(file_tags)
            .map(|(file, tags)| dto::File {
                id: file.id,
                name: file.name,
                size: file.size as usize,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.and_utc(),
                tags: tags.into_iter().map(|tag| tag.tag).collect(),
            })
            .collect())
    }

    pub async fn create_file(&self, file: dto::File) -> Result<dto::File, FileServiceError> {
        let file_id = sqlx::query_as!(
            row_types::FileId,
            "
INSERT INTO files (name, size, mime_type, uploaded_at)
VALUES ($1, $2, $3, $4)
RETURNING id
            ",
            &file.name,
            file.size as i64,
            &file.mime_type,
            file.uploaded_at.naive_utc()
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(dto::File {
            id: file_id.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at,
            tags: file.tags,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileCursor {
    pub id: Uuid,
    pub uploaded_at: DateTime<Utc>,
}

mod row_types {
    use crate::interfaces::dto;
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    pub struct FileId {
        pub id: Uuid,
    }

    pub struct File {
        pub id: Uuid,
        pub name: String,
        pub size: i64,
        pub mime_type: String,
        pub uploaded_at: NaiveDateTime,
    }

    impl From<(File, Vec<FileTag>)> for dto::File {
        fn from((file, tags): (File, Vec<FileTag>)) -> Self {
            Self {
                id: file.id,
                name: file.name,
                size: file.size as usize,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.and_utc(),
                tags: tags.into_iter().map(|tag| tag.tag).collect(),
            }
        }
    }

    pub struct FileTag {
        pub tag: String,
    }
}
