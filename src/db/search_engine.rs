use meilisearch_sdk::{client::Client, indexes::Index};
use thiserror::Error;

pub const FILES_INDEX_UID: &str = "file-indexer-files";
pub const COLLECTIONS_INDEX_UID: &str = "file-indexer-collections";

pub const FILES_PRIMARY_KEY: Option<&str> = Some("id");
pub const COLLECTIONS_PRIMARY_KEY: Option<&str> = Some("id");

#[derive(Error, Debug)]
pub enum SearchEngineError {
    #[error("environment variable `MEILISEARCH_URL` is unable to be retrieved: {0:#?}")]
    RetrieveMeilisearchUrl(std::env::VarError),

    #[error("environment variable `MEILISEARCH_API_KEY` is unable to be retrieved: {0:#?}")]
    RetrieveMeilisearchApiKey(std::env::VarError),

    #[error("meilisearch error: {0:#?}")]
    MeilisearchError(#[from] meilisearch_sdk::errors::Error),

    #[error("failed to create index: {0:#?}")]
    FailedToCreateIndex(meilisearch_sdk::errors::MeilisearchError),
}

pub struct SearchEngine {
    client: Client,
}

impl SearchEngine {
    pub async fn init() -> Result<Self, SearchEngineError> {
        let url =
            std::env::var("MEILISEARCH_URL").map_err(SearchEngineError::RetrieveMeilisearchUrl)?;
        let api_key = match std::env::var("MEILISEARCH_API_KEY") {
            Ok(api_key) => Some(api_key),
            Err(std::env::VarError::NotPresent) => None,
            Err(err) => {
                return Err(SearchEngineError::RetrieveMeilisearchApiKey(err));
            }
        };

        let client = Client::new(url, api_key)?;
        setup_index(&client).await?;

        Ok(Self { client })
    }

    pub fn into_client(self) -> Client {
        self.client
    }
}

async fn setup_index(client: &Client) -> Result<(), SearchEngineError> {
    match client.get_index(FILES_INDEX_UID).await {
        Ok(_) => {}
        Err(meilisearch_sdk::errors::Error::Meilisearch(err))
            if err.error_code == meilisearch_sdk::errors::ErrorCode::IndexNotFound =>
        {
            create_file_index(client).await?;
        }
        Err(err) => {
            return Err(SearchEngineError::MeilisearchError(err));
        }
    }

    match client.get_index(COLLECTIONS_INDEX_UID).await {
        Ok(_) => {}
        Err(meilisearch_sdk::errors::Error::Meilisearch(err))
            if err.error_code == meilisearch_sdk::errors::ErrorCode::IndexNotFound =>
        {
            create_collection_index(client).await?;
        }
        Err(err) => {
            return Err(SearchEngineError::MeilisearchError(err));
        }
    }

    Ok(())
}

async fn create_file_index(client: &Client) -> Result<Index, SearchEngineError> {
    let task = client.create_index(FILES_INDEX_UID, None).await?;
    let task = task.wait_for_completion(client, None, None).await?;
    let index = task
        .try_make_index(client)
        .map_err(|task| SearchEngineError::FailedToCreateIndex(task.unwrap_failure()))?;

    index.set_searchable_attributes(&["name", "tags"]).await?;
    index
        .set_filterable_attributes(&["size", "mime_type", "tags", "uploaded_at"])
        .await?;

    Ok(index)
}

async fn create_collection_index(client: &Client) -> Result<Index, SearchEngineError> {
    let task = client.create_index(COLLECTIONS_INDEX_UID, None).await?;
    let task = task.wait_for_completion(client, None, None).await?;
    let index = task
        .try_make_index(client)
        .map_err(|task| SearchEngineError::FailedToCreateIndex(task.unwrap_failure()))?;

    index.set_searchable_attributes(&["name", "tags"]).await?;
    index
        .set_filterable_attributes(&["size", "tags", "created_at"])
        .await?;

    Ok(index)
}
