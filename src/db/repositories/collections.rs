use super::RepositoryError;
use futures::future::try_join;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct CollectionRepository {
    db_pool: PgPool,
}

impl CollectionRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn find_one_by_id(
        &self,
        collection_id: Uuid,
    ) -> Result<Option<entities::CollectionEntity>, RepositoryError> {
        let collection_task = sqlx::query_as!(
            row_types::RawCollection,
            "
SELECT id, name, created_at
FROM collections
WHERE id = $1",
            collection_id
        )
        .fetch_optional(&self.db_pool);
        let tags_task = sqlx::query_as!(
            row_types::RawCollectionTag,
            "
SELECT tag
FROM collection_tags
WHERE collection_id = $1
ORDER BY tag",
            collection_id
        )
        .fetch_all(&self.db_pool);

        let (collection, tags) = try_join(collection_task, tags_task).await?;

        Ok(collection.map(|raw| (raw, tags).into()))
    }

    pub async fn list(
        &self,
        limit: usize,
        cursor: Option<entities::CollectionCursorEntity>,
    ) -> Result<Vec<entities::CollectionEntity>, RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let collections = match cursor {
            Some(cursor) => {
                sqlx::query_as!(
                    row_types::RawCollection,
                    "
SELECT id, name, created_at
FROM collections
WHERE $1 <= name AND $2 < id
ORDER BY name ASC, id ASC
LIMIT $3",
                    &cursor.name,
                    cursor.id,
                    limit as i64,
                )
                .fetch_all(&mut *tx)
                .await?
            }
            None => {
                sqlx::query_as!(
                    row_types::RawCollection,
                    "
SELECT id, name, created_at
FROM collections
ORDER BY name ASC, id ASC
LIMIT $1",
                    limit as i64,
                )
                .fetch_all(&mut *tx)
                .await?
            }
        };

        let tags = sqlx::query_as!(
            row_types::RawCollectionTagWithCollectionId,
            "
SELECT collection_id, tag
FROM collection_tags
WHERE collection_id = ANY($1::uuid[])
ORDER BY tag",
            &collections
                .iter()
                .map(|collection| collection.id)
                .collect::<Vec<_>>()
        )
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;

        let mut collections_map = HashMap::<_, _>::from_iter(
            collections
                .iter()
                .map(|collection| (collection.id, Vec::with_capacity(10))),
        );

        for tag in tags {
            collections_map
                .entry(tag.collection_id)
                .or_default()
                .push(row_types::RawCollectionTag { tag: tag.tag });
        }

        Ok(collections
            .into_iter()
            .map(|raw| {
                let mut tags = collections_map.remove(&raw.id).unwrap_or_default();
                tags.sort_unstable_by(|a, b| a.tag.cmp(&b.tag));

                (raw, tags).into()
            })
            .collect())
    }

    pub async fn create_one(
        &self,
        collection: entities::CollectionEntityForCreation,
    ) -> Result<entities::CollectionEntity, RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let after_creation = sqlx::query_as!(
            row_types::RawCollectionAfterCreation,
            "
INSERT INTO collections (name)
VALUES ($1)
RETURNING id, created_at",
            collection.name
        )
        .fetch_one(&mut *tx)
        .await?;

        if !collection.tags.is_empty() {
            sqlx::query!(
                "
INSERT INTO collection_tags (collection_id, tag)
SELECT $1, UNNEST($2::text[])
                ",
                after_creation.id,
                &collection.tags[..]
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok((collection, after_creation).into())
    }

    pub async fn update_one(
        &self,
        collection: entities::CollectionEntityForUpdate,
        tags_for_creation: Vec<String>,
        tags_for_deletion: Vec<String>,
    ) -> Result<Option<entities::CollectionEntity>, RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        let collection_id = collection.id;
        let collection = sqlx::query_as!(
            row_types::RawCollectionAfterUpdate,
            "
UPDATE collections
SET name = COALESCE($1, name)
WHERE id = $2
RETURNING name, created_at",
            collection.name,
            collection_id,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let collection = match collection {
            Some(collection) => collection,
            None => {
                return Ok(None);
            }
        };

        let mut tags = sqlx::query_as!(
            row_types::RawCollectionTag,
            "
SELECT tag
FROM collection_tags
WHERE collection_id = $1
ORDER BY tag",
            collection_id
        )
        .fetch_all(&mut *tx)
        .await?;

        if !tags_for_deletion.is_empty() {
            sqlx::query!(
                "
DELETE FROM collection_tags
WHERE collection_id = $1 AND tag = ANY($2::text[])
                    ",
                collection_id,
                &tags_for_deletion
            )
            .execute(&mut *tx)
            .await?;

            tags.retain(|tag| !tags_for_deletion.contains(&tag.tag));
        }

        if !tags_for_creation.is_empty() {
            sqlx::query!(
                "
INSERT INTO collection_tags (collection_id, tag)
SELECT $1, UNNEST($2::text[])
                ",
                collection_id,
                &tags_for_creation
            )
            .execute(&mut *tx)
            .await?;

            tags.extend(
                tags_for_creation
                    .into_iter()
                    .map(|tag| row_types::RawCollectionTag { tag }),
            );
        }

        tx.commit().await?;
        tags.sort_unstable_by(|a, b| a.tag.cmp(&b.tag));

        Ok(Some(entities::CollectionEntity {
            id: collection_id,
            name: collection.name,
            created_at: collection.created_at.and_utc(),
            tags: tags.into_iter().map(|raw| raw.tag).collect(),
        }))
    }

    pub async fn delete_one(&self, collection_id: Uuid) -> Result<(), RepositoryError> {
        let mut tx = self.db_pool.begin().await?;

        sqlx::query!(
            "
DELETE FROM collection_tags
WHERE collection_id = $1",
            collection_id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "
DELETE FROM collections
WHERE id = $1",
            collection_id
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

    pub struct RawCollection {
        pub id: Uuid,
        pub name: String,
        pub created_at: NaiveDateTime,
    }

    pub struct RawCollectionTag {
        pub tag: String,
    }

    pub struct RawCollectionTagWithCollectionId {
        pub collection_id: Uuid,
        pub tag: String,
    }

    pub struct RawCollectionAfterCreation {
        pub id: Uuid,
        pub created_at: NaiveDateTime,
    }

    pub struct RawCollectionAfterUpdate {
        pub name: String,
        pub created_at: NaiveDateTime,
    }
}

pub mod entities {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct CollectionEntity {
        pub id: Uuid,
        pub name: String,
        pub created_at: DateTime<Utc>,
        pub tags: Vec<String>,
    }

    impl
        From<(
            super::row_types::RawCollection,
            Vec<super::row_types::RawCollectionTag>,
        )> for CollectionEntity
    {
        fn from(
            (raw, tags): (
                super::row_types::RawCollection,
                Vec<super::row_types::RawCollectionTag>,
            ),
        ) -> Self {
            Self {
                id: raw.id,
                name: raw.name,
                created_at: raw.created_at.and_utc(),
                tags: tags.into_iter().map(|raw| raw.tag).collect(),
            }
        }
    }

    impl
        From<(
            CollectionEntityForCreation,
            super::row_types::RawCollectionAfterCreation,
        )> for CollectionEntity
    {
        fn from(
            (collection, raw): (
                CollectionEntityForCreation,
                super::row_types::RawCollectionAfterCreation,
            ),
        ) -> Self {
            Self {
                id: raw.id,
                name: collection.name,
                created_at: raw.created_at.and_utc(),
                tags: collection.tags,
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct CollectionCursorEntity {
        pub id: Uuid,
        pub name: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct CollectionEntityForCreation {
        pub name: String,
        pub tags: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct CollectionEntityForUpdate {
        pub id: Uuid,
        pub name: Option<String>,
    }
}
