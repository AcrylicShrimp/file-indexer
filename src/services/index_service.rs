use crate::{
    db::search_engine::{FILES_INDEX_UID, FILES_PRIMARY_KEY},
    interfaces::dto::File,
};
use meilisearch_sdk::client::Client;
use thiserror::Error;

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
        self.client
            .index(FILES_INDEX_UID)
            .add_or_update(&[file], FILES_PRIMARY_KEY)
            .await?;
        Ok(())
    }
}
