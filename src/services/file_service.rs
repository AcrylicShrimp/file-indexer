use crate::interfaces::dto;
use chrono::DateTime;
use futures::future::try_join;
use serde::{Deserialize, Serialize};
use sqlx::{types::chrono::Utc, PgPool};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum FileServiceError {
    #[error("database error: {0:#?}")]
    DbError(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct FileService {
    db_pool: PgPool,
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
            row_types::FileTagOnly,
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
        let mut tx = self.db_pool.begin().await?;

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
                .fetch_all(&mut *tx)
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
                .fetch_all(&mut *tx)
                .await
            }
        }?;

        if files.is_empty() {
            return Ok(vec![]);
        }

        let file_tags = sqlx::query_as!(
            row_types::FileTag,
            "
SELECT file_id, tag
FROM file_tags
WHERE file_id = ANY($1::uuid[])
        ",
            &files.iter().map(|file| file.id).collect::<Vec<_>>()
        )
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;

        let mut files_map =
            HashMap::<_, _>::from_iter(files.iter().map(|file| (file.id, Vec::with_capacity(10))));

        for tag in file_tags {
            files_map.entry(tag.file_id).or_default().push(tag.tag);
        }

        Ok(files
            .into_iter()
            .map(|file| {
                let mut tags = files_map.remove(&file.id).unwrap_or_default();
                tags.sort_unstable();

                dto::File {
                    id: file.id,
                    name: file.name,
                    size: file.size as usize,
                    mime_type: file.mime_type,
                    uploaded_at: file.uploaded_at.and_utc(),
                    tags,
                }
            })
            .collect())
    }

    pub async fn create_file(
        &self,
        mut file: dto::CreatingFile,
    ) -> Result<dto::File, FileServiceError> {
        let mut tx = self.db_pool.begin().await?;

        let created_file = sqlx::query_as!(
            row_types::CreatedFile,
            "
INSERT INTO files (name, size, mime_type)
VALUES ($1, $2, $3)
RETURNING id, uploaded_at
            ",
            &file.name,
            file.size as i64,
            &file.mime_type,
        )
        .fetch_one(&mut *tx)
        .await?;

        if let Some(tags) = &mut file.tags {
            tags.sort_unstable();
            tags.dedup();

            if !tags.is_empty() {
                sqlx::query!(
                    "
INSERT INTO file_tags (file_id, tag)
SELECT $1, UNNEST($2::text[])
                    ",
                    created_file.id,
                    &tags[..]
                )
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        Ok(dto::File {
            id: created_file.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            uploaded_at: created_file.uploaded_at.and_utc(),
            tags: file.tags.unwrap_or_default(),
        })
    }

    pub async fn update_file(
        &self,
        file_id: Uuid,
        file: dto::UpdatingFile,
    ) -> Result<Option<dto::File>, FileServiceError> {
        let mut tx = self.db_pool.begin().await?;

        let original_file = match self.get_file(file_id).await? {
            Some(original_file) => original_file,
            None => {
                return Ok(None);
            }
        };

        let name = file.name.as_ref().unwrap_or(&original_file.name);
        let size = file.size.unwrap_or(original_file.size);
        let mime_type = file.mime_type.as_ref().unwrap_or(&original_file.mime_type);
        sqlx::query!(
            "
UPDATE files
SET name = $1, size = $2, mime_type = $3
WHERE id = $4
            ",
            name,
            size as i64,
            mime_type,
            file_id
        )
        .execute(&mut *tx)
        .await?;

        if let Some(tags) = &file.tags {
            let from_set = BTreeSet::from_iter(original_file.tags.iter());
            let to_set = BTreeSet::from_iter(tags.iter());

            let to_add = Vec::from_iter(to_set.difference(&from_set).map(|tag| tag.to_string()));
            let to_remove = Vec::from_iter(from_set.difference(&to_set).map(|tag| tag.to_string()));

            if !to_remove.is_empty() {
                sqlx::query!(
                    "
DELETE FROM file_tags
WHERE file_id = $1 AND tag = ANY($2::text[])
                    ",
                    file_id,
                    &to_remove
                )
                .execute(&mut *tx)
                .await?;
            }

            if !to_add.is_empty() {
                sqlx::query!(
                    "
INSERT INTO file_tags (file_id, tag)
SELECT $1, UNNEST($2::text[])
                    ",
                    file_id,
                    &to_add
                )
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        Ok(Some(dto::File {
            id: file_id,
            name: file.name.unwrap_or(original_file.name),
            size: file.size.unwrap_or(original_file.size),
            mime_type: file.mime_type.unwrap_or(original_file.mime_type),
            uploaded_at: original_file.uploaded_at,
            tags: file.tags.unwrap_or_default(),
        }))
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

    pub struct CreatedFile {
        pub id: Uuid,
        pub uploaded_at: NaiveDateTime,
    }

    pub struct File {
        pub id: Uuid,
        pub name: String,
        pub size: i64,
        pub mime_type: String,
        pub uploaded_at: NaiveDateTime,
    }

    pub struct FileTag {
        pub file_id: Uuid,
        pub tag: String,
    }

    pub struct FileTagOnly {
        pub tag: String,
    }

    impl From<(File, Vec<FileTagOnly>)> for dto::File {
        fn from((file, tags): (File, Vec<FileTagOnly>)) -> Self {
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
}
