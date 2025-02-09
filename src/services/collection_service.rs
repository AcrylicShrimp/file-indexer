use crate::{
    db::repositories::collection::{self, CollectionRepository},
    interfaces::{collections, files},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum CollectionServiceError {
    #[error("repository error: {0:#?}")]
    RepositoryError(#[from] crate::db::repositories::RepositoryError),
}

#[derive(Clone)]
pub struct CollectionService {
    collection_repository: CollectionRepository,
}

impl CollectionService {
    pub fn new(collection_repository: CollectionRepository) -> Self {
        Self {
            collection_repository,
        }
    }

    pub async fn get_collection(
        &self,
        collection_id: Uuid,
    ) -> Result<Option<collections::Collection>, CollectionServiceError> {
        let collection = self
            .collection_repository
            .find_one_by_id(collection_id)
            .await?;

        Ok(collection.map(|collection| collections::Collection {
            id: collection.id,
            name: collection.name,
            created_at: collection.created_at,
            tags: collection.tags,
        }))
    }

    pub async fn list_collections(
        &self,
        limit: usize,
        cursor: Option<collections::CollectionCursor>,
    ) -> Result<Vec<collections::Collection>, CollectionServiceError> {
        let cursor = cursor.map(|cursor| collection::entities::CollectionCursorEntity {
            id: cursor.id,
            name: cursor.name,
        });
        let collections = self.collection_repository.list(limit, cursor).await?;

        Ok(collections
            .into_iter()
            .map(|collection| collections::Collection {
                id: collection.id,
                name: collection.name,
                created_at: collection.created_at,
                tags: collection.tags,
            })
            .collect())
    }

    pub async fn list_collection_files(
        &self,
        collection_id: Uuid,
        limit: usize,
        cursor: Option<collections::CollectionFileCursor>,
    ) -> Result<Vec<files::File>, CollectionServiceError> {
        let cursor = cursor.map(|cursor| collection::entities::CollectionFileCursorEntity {
            id: cursor.id,
            name: cursor.name,
        });
        let files = self
            .collection_repository
            .list_files(collection_id, limit, cursor)
            .await?;

        Ok(files
            .into_iter()
            .map(|file| files::File {
                id: file.id,
                name: file.name,
                size: file.size,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at,
                tags: file.tags,
            })
            .collect())
    }

    pub async fn create_collection(
        &self,
        collection: collections::CreatingCollection,
    ) -> Result<collections::Collection, CollectionServiceError> {
        let collection = self
            .collection_repository
            .create_one(collection::entities::CollectionEntityForCreation {
                name: collection.name,
                tags: collection.tags,
            })
            .await?;

        Ok(collections::Collection {
            id: collection.id,
            name: collection.name,
            created_at: collection.created_at,
            tags: collection.tags,
        })
    }

    pub async fn update_collection(
        &self,
        collection_id: Uuid,
        collection: collections::UpdatingCollection,
    ) -> Result<Option<collections::Collection>, CollectionServiceError> {
        let collection = self
            .collection_repository
            .update_one(
                collection::entities::CollectionEntityForUpdate {
                    id: collection_id,
                    name: collection.name,
                },
                collection.tags_for_creation.unwrap_or_default(),
                collection.tags_for_deletion.unwrap_or_default(),
            )
            .await?;

        Ok(collection.map(|collection| collections::Collection {
            id: collection.id,
            name: collection.name,
            created_at: collection.created_at,
            tags: collection.tags,
        }))
    }

    pub async fn delete_collection(
        &self,
        collection_id: Uuid,
    ) -> Result<(), CollectionServiceError> {
        self.collection_repository.delete_one(collection_id).await?;

        Ok(())
    }
}
