use crate::{
    interfaces::{
        admins::{AdminTask, AdminTaskStatus},
        collections::CollectionCursor,
        files::FileCursor,
    },
    services::{
        admin_task_service::{
            AdminTaskService, RE_INDEX_COLLECTIONS_TASK_NAME, RE_INDEX_FILES_TASK_NAME,
        },
        collection_service::CollectionService,
        file_service::FileService,
        index_service::IndexService,
    },
};
use chrono::{DateTime, Utc};
use rocket::{
    async_trait,
    fairing::{Fairing, Info, Kind},
    Orbit, Rocket,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ReIndexerError {
    #[error("admin task service failure: {0:#?}")]
    AdminTask(#[from] crate::services::admin_task_service::AdminTaskServiceError),
    #[error("collection service failure: {0:#?}")]
    Collection(#[from] crate::services::collection_service::CollectionServiceError),
    #[error("file service failure: {0:#?}")]
    File(#[from] crate::services::file_service::FileServiceError),
    #[error("index service failure: {0:#?}")]
    Index(#[from] crate::services::index_service::IndexServiceError),
    #[error("failed to serialize or deserialize admin task metadata: {0:#?}")]
    MetadataSerde(#[from] serde_json::Error),
}

pub struct ReIndexer {
    admin_task_service: AdminTaskService,
    collection_service: CollectionService,
    file_service: FileService,
    index_service: IndexService,
    stop_signal: Mutex<Option<tokio::sync::mpsc::Sender<()>>>,
    task_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl ReIndexer {
    pub fn new(
        admin_task_service: AdminTaskService,
        collection_service: CollectionService,
        file_service: FileService,
        index_service: IndexService,
    ) -> Self {
        Self {
            admin_task_service,
            collection_service,
            file_service,
            index_service,
            stop_signal: Mutex::new(None),
            task_handle: Mutex::new(None),
        }
    }

    async fn create_re_index_task(&self) {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let task_handle = tokio::spawn(re_index_task(
            rx,
            self.admin_task_service.clone(),
            self.collection_service.clone(),
            self.file_service.clone(),
            self.index_service.clone(),
        ));

        *self.stop_signal.lock().await = Some(tx);
        *self.task_handle.lock().await = Some(task_handle);
    }
}

#[async_trait]
impl Fairing for ReIndexer {
    fn info(&self) -> Info {
        Info {
            name: "re-indexer",
            kind: Kind::Liftoff | Kind::Shutdown,
        }
    }

    async fn on_liftoff(&self, _rocket: &Rocket<Orbit>) {
        self.create_re_index_task().await;
    }

    async fn on_shutdown(&self, _rocket: &Rocket<Orbit>) {
        if let Some(tx) = self.stop_signal.lock().await.take() {
            if let Err(err) = tx.send(()).await {
                log::warn!("failed to send stop signal to re-index task: {err:#?}");
                return;
            }
        }

        if let Some(task_handle) = self.task_handle.lock().await.take() {
            if let Err(err) = task_handle.await {
                log::warn!("failed to wait for re-index task to finish: {err:#?}");
            }
        }
    }
}

async fn re_index_task(
    mut stop_signal: tokio::sync::mpsc::Receiver<()>,
    admin_task_service: AdminTaskService,
    collection_service: CollectionService,
    file_service: FileService,
    index_service: IndexService,
) {
    let mut duration_secs = 10;

    loop {
        let mut timer = tokio::time::interval(Duration::from_secs(duration_secs));

        tokio::select! {
            _ = stop_signal.recv() => {
                return;
            }
            _ = timer.tick() => {
                let files_result = re_index_task_on_tick_files(
                    &admin_task_service,
                    &file_service,
                    &index_service,
                ).await;

                let collections_result = re_index_task_on_tick_collections(
                    &admin_task_service,
                    &collection_service,
                    &index_service,
                ).await;

                let file_duration_secs = match files_result {
                    Ok(ReIndexTaskResult::NoTask) => {
                        10
                    }
                    Ok(ReIndexTaskResult::TaskNotCompleted) => {
                        1
                    }
                    Ok(ReIndexTaskResult::TaskCompleted) => {
                        10
                    }
                    Err(err) => {
                        log::error!("re-index task on tick for files error: {err:#?}");
                        10
                    }
                };

                let collections_duration_secs = match collections_result {
                    Ok(ReIndexTaskResult::NoTask) => {
                        10
                    }
                    Ok(ReIndexTaskResult::TaskNotCompleted) => {
                        1
                    }
                    Ok(ReIndexTaskResult::TaskCompleted) => {
                        10
                    }
                    Err(err) => {
                        log::error!("re-index task on tick for collections error: {err:#?}");
                        10
                    }
                };

                duration_secs = std::cmp::min(file_duration_secs, collections_duration_secs);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ReIndexTaskResult {
    NoTask,
    TaskNotCompleted,
    TaskCompleted,
}

async fn re_index_task_on_tick_files(
    admin_task_service: &AdminTaskService,
    file_service: &FileService,
    index_service: &IndexService,
) -> Result<ReIndexTaskResult, ReIndexerError> {
    let task = admin_task_service
        .get_last_active_task(RE_INDEX_FILES_TASK_NAME)
        .await?;
    let task = match task {
        Some(admin_task) => admin_task,
        None => {
            return Ok(ReIndexTaskResult::NoTask);
        }
    };
    let task_id = task.id;

    admin_task_service
        .update_task_status(task_id, AdminTaskStatus::InProgress)
        .await?;

    let result =
        re_index_task_on_tick_for_task_files(task, admin_task_service, file_service, index_service)
            .await;
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            admin_task_service
                .update_task_status(task_id, AdminTaskStatus::Failed)
                .await?;
            return Err(err);
        }
    };

    if result == ReIndexTaskResult::TaskCompleted {
        admin_task_service
            .update_task_status(task_id, AdminTaskStatus::Completed)
            .await?;
    }

    Ok(result)
}

async fn re_index_task_on_tick_collections(
    admin_task_service: &AdminTaskService,
    collection_service: &CollectionService,
    index_service: &IndexService,
) -> Result<ReIndexTaskResult, ReIndexerError> {
    let task = admin_task_service
        .get_last_active_task(RE_INDEX_COLLECTIONS_TASK_NAME)
        .await?;
    let task = match task {
        Some(admin_task) => admin_task,
        None => {
            return Ok(ReIndexTaskResult::NoTask);
        }
    };
    let task_id = task.id;

    admin_task_service
        .update_task_status(task_id, AdminTaskStatus::InProgress)
        .await?;

    let result = re_index_task_on_tick_for_task_collections(
        task,
        admin_task_service,
        collection_service,
        index_service,
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            admin_task_service
                .update_task_status(task_id, AdminTaskStatus::Failed)
                .await?;
            return Err(err);
        }
    };

    if result == ReIndexTaskResult::TaskCompleted {
        admin_task_service
            .update_task_status(task_id, AdminTaskStatus::Completed)
            .await?;
    }

    Ok(result)
}

async fn re_index_task_on_tick_for_task_files(
    admin_task: AdminTask,
    admin_task_service: &AdminTaskService,
    file_service: &FileService,
    index_service: &IndexService,
) -> Result<ReIndexTaskResult, ReIndexerError> {
    #[derive(Serialize, Deserialize)]
    struct ReIndexTaskMetadata {
        last_file_id: Option<Uuid>,
        last_file_uploaded_at: Option<DateTime<Utc>>,
    }

    let metadata: ReIndexTaskMetadata = serde_json::from_value(admin_task.metadata)?;
    let cursor = match (metadata.last_file_id, metadata.last_file_uploaded_at) {
        (Some(last_file_id), Some(last_file_uploaded_at)) => Some(FileCursor {
            id: last_file_id,
            uploaded_at: last_file_uploaded_at,
        }),
        _ => None,
    };

    let files = file_service.list_files(1000, cursor).await?;
    let last_file = match files.last() {
        Some(file) => file,
        None => {
            // no more files to index; task is completed
            return Ok(ReIndexTaskResult::TaskCompleted);
        }
    };

    index_service.index_files(&files).await?;

    let metadata = ReIndexTaskMetadata {
        last_file_id: Some(last_file.id),
        last_file_uploaded_at: Some(last_file.uploaded_at),
    };
    let metadata = serde_json::to_value(metadata)?;

    admin_task_service
        .update_task_metadata(admin_task.id, metadata)
        .await?;

    Ok(ReIndexTaskResult::TaskNotCompleted)
}

async fn re_index_task_on_tick_for_task_collections(
    admin_task: AdminTask,
    admin_task_service: &AdminTaskService,
    collection_service: &CollectionService,
    index_service: &IndexService,
) -> Result<ReIndexTaskResult, ReIndexerError> {
    #[derive(Serialize, Deserialize)]
    struct ReIndexTaskMetadata {
        last_collection_id: Option<Uuid>,
        last_collection_name: Option<String>,
    }

    let metadata: ReIndexTaskMetadata = serde_json::from_value(admin_task.metadata)?;
    let cursor = match (metadata.last_collection_id, metadata.last_collection_name) {
        (Some(last_collection_id), Some(last_collection_name)) => Some(CollectionCursor {
            id: last_collection_id,
            name: last_collection_name,
        }),
        _ => None,
    };

    let collections = collection_service.list_collections(1000, cursor).await?;
    let last_collection = match collections.last() {
        Some(collection) => collection,
        None => {
            // no more files to index; task is completed
            return Ok(ReIndexTaskResult::TaskCompleted);
        }
    };

    index_service.index_collections(&collections).await?;

    let metadata = ReIndexTaskMetadata {
        last_collection_id: Some(last_collection.id),
        last_collection_name: Some(last_collection.name.clone()),
    };
    let metadata = serde_json::to_value(metadata)?;

    admin_task_service
        .update_task_metadata(admin_task.id, metadata)
        .await?;

    Ok(ReIndexTaskResult::TaskNotCompleted)
}
