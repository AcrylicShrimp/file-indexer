use crate::{
    db::search_engine::{FILES_INDEX_UID, FILES_PRIMARY_KEY},
    interfaces::dto::{File, FileSearchQuery, FileSearchQueryFilter},
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

pub struct IndexService {
    client: Client,
}

impl IndexService {
    pub fn new(client: Client) -> Self {
        Self { client }
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

    pub async fn search_files(&self, q: &FileSearchQuery) -> Result<Vec<File>, IndexServiceError> {
        let index = self.client.index(FILES_INDEX_UID);

        let mut query = index.search();
        query.with_query(&q.q);
        query.with_limit(q.limit);
        query.with_attributes_to_highlight(Selectors::Some(&[]));

        let filter = if q.filters.is_empty() {
            vec![]
        } else {
            Vec::from_iter(q.filters.iter().filter_map(|filters| build_filter(filters)))
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
}

fn build_filter(filters: &[FileSearchQueryFilter]) -> Option<String> {
    if filters.is_empty() {
        return None;
    }

    Some(Vec::from_iter(filters.iter().map(build_filter_element)).join(" OR "))
}

fn build_filter_element(filter: &FileSearchQueryFilter) -> String {
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
