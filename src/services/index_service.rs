use crate::{
    db::search_engine::{
        COLLECTIONS_INDEX_UID, COLLECTIONS_PRIMARY_KEY, FILES_INDEX_UID, FILES_PRIMARY_KEY,
    },
    interfaces::{
        collections::{Collection, CollectionSearchQuery},
        files::{File, FileSearchQuery},
    },
};
use chrono::{DateTime, Utc};
use meilisearch_sdk::{
    client::Client,
    search::{SearchResults, Selectors},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum IndexServiceError {
    #[error("meilisearch error: {0:#?}")]
    MeilisearchError(#[from] meilisearch_sdk::errors::Error),
}

#[derive(Clone)]
pub struct IndexService {
    client: Client,
}

impl IndexService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn empty_index(&self) -> Result<(), IndexServiceError> {
        self.client
            .index(FILES_INDEX_UID)
            .delete_all_documents()
            .await?;
        self.client
            .index(COLLECTIONS_INDEX_UID)
            .delete_all_documents()
            .await?;

        Ok(())
    }

    pub async fn index_file(&self, file: &File) -> Result<(), IndexServiceError> {
        #[derive(Serialize)]
        struct IndexingFile<'a> {
            id: Uuid,
            name: &'a str,
            size: usize,
            mime_type: &'a str,
            tags: &'a [String],
            uploaded_at: i64,
        }

        self.client
            .index(FILES_INDEX_UID)
            .add_or_update(
                &[IndexingFile {
                    id: file.id,
                    name: &file.name,
                    size: file.size,
                    mime_type: &file.mime_type,
                    tags: &file.tags,
                    uploaded_at: file.uploaded_at.timestamp(),
                }],
                FILES_PRIMARY_KEY,
            )
            .await?;

        Ok(())
    }

    pub async fn index_collection(&self, collection: &Collection) -> Result<(), IndexServiceError> {
        #[derive(Serialize)]
        struct IndexingCollection<'a> {
            id: Uuid,
            name: &'a str,
            tags: &'a [String],
            created_at: i64,
        }

        self.client
            .index(COLLECTIONS_INDEX_UID)
            .add_or_update(
                &[IndexingCollection {
                    id: collection.id,
                    name: &collection.name,
                    tags: &collection.tags,
                    created_at: collection.created_at.timestamp(),
                }],
                COLLECTIONS_PRIMARY_KEY,
            )
            .await?;

        Ok(())
    }

    pub async fn index_files(&self, files: &[File]) -> Result<(), IndexServiceError> {
        #[derive(Serialize)]
        struct IndexingFile<'a> {
            id: Uuid,
            name: &'a str,
            size: usize,
            mime_type: &'a str,
            tags: &'a [String],
            uploaded_at: i64,
        }

        let indexing_files = files
            .iter()
            .map(|file| IndexingFile {
                id: file.id,
                name: &file.name,
                size: file.size,
                mime_type: &file.mime_type,
                tags: &file.tags,
                uploaded_at: file.uploaded_at.timestamp(),
            })
            .collect::<Vec<_>>();

        self.client
            .index(FILES_INDEX_UID)
            .add_or_update(&indexing_files, FILES_PRIMARY_KEY)
            .await?;

        Ok(())
    }

    pub async fn index_collections(
        &self,
        collections: &[Collection],
    ) -> Result<(), IndexServiceError> {
        #[derive(Serialize)]
        struct IndexingCollection<'a> {
            id: Uuid,
            name: &'a str,
        }

        let indexing_collections = collections
            .iter()
            .map(|collection| IndexingCollection {
                id: collection.id,
                name: &collection.name,
            })
            .collect::<Vec<_>>();

        self.client
            .index(COLLECTIONS_INDEX_UID)
            .add_or_update(&indexing_collections, COLLECTIONS_PRIMARY_KEY)
            .await?;

        Ok(())
    }

    pub async fn delete_file(&self, file_id: Uuid) -> Result<(), IndexServiceError> {
        self.client
            .index(FILES_INDEX_UID)
            .delete_document(file_id)
            .await?;

        Ok(())
    }

    pub async fn delete_collection(&self, collection_id: Uuid) -> Result<(), IndexServiceError> {
        self.client
            .index(COLLECTIONS_INDEX_UID)
            .delete_document(collection_id)
            .await?;

        Ok(())
    }

    pub async fn search_files(&self, q: &FileSearchQuery) -> Result<Vec<File>, IndexServiceError> {
        let index = self.client.index(FILES_INDEX_UID);

        let mut query = index.search();
        query.with_query(&q.q);
        query.with_limit(q.limit);
        query.with_attributes_to_highlight(Selectors::Some(&[]));

        let filter = if q.filters.is_empty() {
            vec![]
        } else {
            Vec::from_iter(
                q.filters
                    .iter()
                    .filter_map(|filters| filters::build_file_filter(filters)),
            )
        };
        let filter = Vec::from_iter(filter.iter().map(|filter| filter.as_str()));

        #[derive(Deserialize)]
        struct SearchedFile {
            id: Uuid,
            name: String,
            size: usize,
            mime_type: String,
            tags: Vec<String>,
            uploaded_at: i64,
        }

        let result: SearchResults<SearchedFile> =
            query.with_array_filter(filter).build().execute().await?;

        Ok(result
            .hits
            .into_iter()
            .map(|hit| File {
                id: hit.result.id,
                name: hit.result.name,
                size: hit.result.size,
                mime_type: hit.result.mime_type,
                tags: hit.result.tags,
                uploaded_at: DateTime::<Utc>::from_timestamp(hit.result.uploaded_at, 0)
                    .unwrap_or_default(),
            })
            .collect())
    }

    pub async fn search_collections(
        &self,
        q: &CollectionSearchQuery,
    ) -> Result<Vec<Collection>, IndexServiceError> {
        let index = self.client.index(COLLECTIONS_INDEX_UID);

        let mut query = index.search();
        query.with_query(&q.q);
        query.with_limit(q.limit);

        #[derive(Deserialize)]
        struct SearchedCollection {
            id: Uuid,
            name: String,
            created_at: i64,
            tags: Vec<String>,
        }

        let result: SearchResults<SearchedCollection> = query.build().execute().await?;

        Ok(result
            .hits
            .into_iter()
            .map(|hit| Collection {
                id: hit.result.id,
                name: hit.result.name,
                created_at: DateTime::<Utc>::from_timestamp(hit.result.created_at, 0)
                    .unwrap_or_default(),
                tags: hit.result.tags,
            })
            .collect())
    }
}

mod filters {
    use crate::interfaces::files::FileSearchQueryFilter;

    pub fn build_file_filter(filters: &[FileSearchQueryFilter]) -> Option<String> {
        if filters.is_empty() {
            return None;
        }

        Some(Vec::from_iter(filters.iter().map(build_file_filter_element)).join(" OR "))
    }

    fn build_file_filter_element(filter: &FileSearchQueryFilter) -> String {
        match filter {
            FileSearchQueryFilter::Size { operator, value } => {
                format!("size {} {}", operator.to_str(), value)
            }
            FileSearchQueryFilter::MimeType { value } => {
                format!("mime_type = '{}'", escape_str(value))
            }
            FileSearchQueryFilter::Tag { value } => {
                format!("tags = '{}'", escape_str(value))
            }
            FileSearchQueryFilter::TagIsEmpty => "tags IS EMPTY".to_owned(),
            FileSearchQueryFilter::TagIsNotEmpty => "tags IS NOT EMPTY".to_owned(),
            FileSearchQueryFilter::UploadedAt { operator, value } => {
                format!("uploaded_at {} {}", operator.to_str(), value.timestamp())
            }
        }
    }

    fn escape_str(s: &str) -> String {
        s.replace('\'', "\\'")
    }
}
