use super::RepositoryError;
use chrono::{DateTime, Utc};
use futures::future::try_join;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct FileRepository {
    db_pool: PgPool,
}

impl FileRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn find_one_by_id(
        &self,
        file_id: Uuid,
    ) -> Result<Option<entities::FileEntity>, RepositoryError> {
        let file_task = sqlx::query_as!(
            row_types::RawFile,
            "
SELECT
    id,
    name,
    size,
    mime_type,
    uploaded_at
FROM files
WHERE id = $1 AND is_ready = TRUE",
            file_id
        )
        .fetch_optional(&self.db_pool);
        let tags_task = sqlx::query_as!(
            row_types::RawFileTag,
            "
SELECT tag
FROM file_tags
WHERE file_id = $1
ORDER BY tag",
            file_id
        )
        .fetch_all(&self.db_pool);

        let (file, tags) = try_join(file_task, tags_task).await?;

        Ok(file.map(|raw| (raw, tags).into()))
    }

    pub async fn find_one_for_upload(
        &self,
        file_id: Uuid,
    ) -> Result<Option<entities::FileEntityForUpload>, RepositoryError> {
        let file = sqlx::query_as!(
            row_types::RawFileForUpload,
            "
SELECT size, mime_type
FROM files
WHERE id = $1",
            file_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(file.map(|raw| raw.into()))
    }

    pub async fn list(
        &self,
        limit: usize,
        cursor: Option<entities::FileCursorEntity>,
    ) -> Result<Vec<entities::FileEntity>, RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let files = match cursor {
            Some(cursor) => {
                sqlx::query_as!(
                    row_types::RawFile,
                    "
SELECT
    id,
    name,
    size,
    mime_type,
    uploaded_at
FROM files
WHERE uploaded_at <= $1 AND $2 < id AND is_ready = TRUE
ORDER BY uploaded_at DESC, id ASC
LIMIT $3",
                    cursor.uploaded_at.naive_utc(),
                    cursor.id,
                    limit as i64
                )
                .fetch_all(&mut *tx)
                .await?
            }
            None => {
                sqlx::query_as!(
                    row_types::RawFile,
                    "
SELECT
    id,
    name,
    size,
    mime_type,
    uploaded_at
FROM files
WHERE is_ready = TRUE
ORDER BY uploaded_at DESC, id ASC
LIMIT $1",
                    limit as i64
                )
                .fetch_all(&mut *tx)
                .await?
            }
        };

        let tags = sqlx::query_as!(
            row_types::RawFileTagWithFileId,
            "
SELECT file_id, tag
FROM file_tags
WHERE file_id = ANY($1::uuid[])",
            &files.iter().map(|file| file.id).collect::<Vec<_>>()
        )
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;

        let mut files_map =
            HashMap::<_, _>::from_iter(files.iter().map(|file| (file.id, Vec::with_capacity(10))));

        for tag in tags {
            files_map
                .entry(tag.file_id)
                .or_default()
                .push(row_types::RawFileTag { tag: tag.tag });
        }

        Ok(files
            .into_iter()
            .map(|file| {
                let mut tags = files_map.remove(&file.id).unwrap_or_default();
                tags.sort_unstable_by(|a, b| a.tag.cmp(&b.tag));

                (file, tags).into()
            })
            .collect())
    }

    pub async fn create_one(
        &self,
        file: entities::FileEntityForCreation,
    ) -> Result<entities::FileEntity, RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let after_creation = sqlx::query_as!(
            row_types::RawFileAfterCreation,
            "
INSERT INTO files (name, size, mime_type)
VALUES ($1, $2, $3)
RETURNING id, uploaded_at",
            &file.name,
            file.size as i64,
            &file.mime_type,
        )
        .fetch_one(&mut *tx)
        .await?;

        if !file.tags.is_empty() {
            sqlx::query!(
                "
INSERT INTO file_tags (file_id, tag)
SELECT $1, UNNEST($2::text[])
                ",
                after_creation.id,
                &file.tags[..]
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok((file, after_creation).into())
    }

    pub async fn update_one(
        &self,
        file: entities::FileEntityForUpdate,
        tags_for_creation: Vec<String>,
        tags_for_deletion: Vec<String>,
    ) -> Result<Option<entities::FileEntity>, RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let file_id = file.id;
        let file = sqlx::query_as!(
            row_types::RawFileAfterUpdate,
            "
UPDATE files
SET
    name = COALESCE($1, name),
    size = COALESCE($2, size),
    mime_type = COALESCE($3, mime_type)
WHERE id = $4
RETURNING name, size, mime_type, uploaded_at",
            file.name,
            file.size.map(|size| size as i64),
            file.mime_type,
            file_id,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let file = match file {
            Some(file) => file,
            None => {
                return Ok(None);
            }
        };

        let mut tags = sqlx::query_as!(
            row_types::RawFileTag,
            "
SELECT tag
FROM file_tags
WHERE file_id = $1
ORDER BY tag",
            file_id
        )
        .fetch_all(&mut *tx)
        .await?;

        if !tags_for_deletion.is_empty() {
            sqlx::query!(
                "
DELETE FROM file_tags
WHERE file_id = $1 AND tag = ANY($2::text[])
                    ",
                file_id,
                &tags_for_deletion
            )
            .execute(&mut *tx)
            .await?;

            tags.retain(|tag| !tags_for_deletion.contains(&tag.tag));
        }

        if !tags_for_creation.is_empty() {
            sqlx::query!(
                "
INSERT INTO file_tags (file_id, tag)
SELECT $1, UNNEST($2::text[])
                ",
                file_id,
                &tags_for_creation
            )
            .execute(&mut *tx)
            .await?;

            tags.extend(
                tags_for_creation
                    .into_iter()
                    .map(|tag| row_types::RawFileTag { tag }),
            );
        }

        tx.commit().await?;
        tags.sort_unstable_by(|a, b| a.tag.cmp(&b.tag));

        Ok(Some(entities::FileEntity {
            id: file_id,
            name: file.name,
            size: file.size as usize,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at.and_utc(),
            tags: tags.into_iter().map(|raw| raw.tag).collect(),
        }))
    }

    pub async fn update_one_as_ready(
        &self,
        file_id: Uuid,
    ) -> Result<Option<entities::FileEntity>, RepositoryError> {
        let file = sqlx::query_as!(
            row_types::RawFileAfterUpdate,
            "
UPDATE files
SET is_ready = TRUE
WHERE id = $1
RETURNING
    name,
    size,
    mime_type,
    uploaded_at",
            file_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        let file = match file {
            Some(file) => file,
            None => {
                return Ok(None);
            }
        };

        let tags = sqlx::query_as!(
            row_types::RawFileTag,
            "
SELECT tag
FROM file_tags
WHERE file_id = $1
ORDER BY tag",
            file_id
        )
        .fetch_all(&self.db_pool)
        .await?;

        Ok(Some(entities::FileEntity {
            id: file_id,
            name: file.name,
            size: file.size as usize,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at.and_utc(),
            tags: tags.into_iter().map(|raw| raw.tag).collect(),
        }))
    }

    pub async fn delete_one(&self, file_id: Uuid) -> Result<(), RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        sqlx::query!(
            "
DELETE FROM file_tags
WHERE file_id = $1",
            file_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "
DELETE FROM files
WHERE id = $1",
            file_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn delete_unready_many(
        &self,
        before_uploaded_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let file_ids = sqlx::query_as!(
            row_types::RawFileId,
            "
SELECT id
FROM files
WHERE uploaded_at < $1 AND is_ready = FALSE",
            before_uploaded_at.naive_utc()
        )
        .fetch_all(&mut *tx)
        .await?;

        sqlx::query!(
            "
DELETE FROM file_tags
WHERE file_id = ANY($1::uuid[])",
            &file_ids
                .iter()
                .map(|file_id| file_id.id)
                .collect::<Vec<_>>()
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "
DELETE FROM files
WHERE id = ANY($1::uuid[])",
            &file_ids
                .iter()
                .map(|file_id| file_id.id)
                .collect::<Vec<_>>()
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
}

mod row_types {
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    pub struct RawFile {
        pub id: Uuid,
        pub name: String,
        pub size: i64,
        pub mime_type: String,
        pub uploaded_at: NaiveDateTime,
    }

    pub struct RawFileId {
        pub id: Uuid,
    }

    pub struct RawFileTag {
        pub tag: String,
    }

    pub struct RawFileTagWithFileId {
        pub file_id: Uuid,
        pub tag: String,
    }

    pub struct RawFileForUpload {
        pub size: i64,
        pub mime_type: String,
    }

    pub struct RawFileAfterCreation {
        pub id: Uuid,
        pub uploaded_at: NaiveDateTime,
    }

    pub struct RawFileAfterUpdate {
        pub name: String,
        pub size: i64,
        pub mime_type: String,
        pub uploaded_at: NaiveDateTime,
    }
}

pub mod entities {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FileEntity {
        pub id: Uuid,
        pub name: String,
        pub size: usize,
        pub mime_type: String,
        pub uploaded_at: DateTime<Utc>,
        pub tags: Vec<String>,
    }

    impl From<(super::row_types::RawFile, Vec<super::row_types::RawFileTag>)> for FileEntity {
        fn from(
            (raw, tags): (super::row_types::RawFile, Vec<super::row_types::RawFileTag>),
        ) -> Self {
            Self {
                id: raw.id,
                name: raw.name,
                size: raw.size as usize,
                mime_type: raw.mime_type,
                uploaded_at: raw.uploaded_at.and_utc(),
                tags: tags.into_iter().map(|raw| raw.tag).collect(),
            }
        }
    }

    impl
        From<(
            FileEntityForCreation,
            super::row_types::RawFileAfterCreation,
        )> for FileEntity
    {
        fn from(
            (file, raw): (
                FileEntityForCreation,
                super::row_types::RawFileAfterCreation,
            ),
        ) -> Self {
            Self {
                id: raw.id,
                name: file.name,
                size: file.size,
                mime_type: file.mime_type,
                uploaded_at: raw.uploaded_at.and_utc(),
                tags: file.tags,
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FileEntityForUpload {
        pub size: usize,
        pub mime_type: String,
    }

    impl From<super::row_types::RawFileForUpload> for FileEntityForUpload {
        fn from(raw: super::row_types::RawFileForUpload) -> Self {
            Self {
                size: raw.size as usize,
                mime_type: raw.mime_type,
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FileCursorEntity {
        pub id: Uuid,
        pub uploaded_at: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FileEntityForCreation {
        pub name: String,
        pub size: usize,
        pub mime_type: String,
        pub tags: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FileEntityForUpdate {
        pub id: Uuid,
        pub name: Option<String>,
        pub size: Option<usize>,
        pub mime_type: Option<String>,
    }
}
