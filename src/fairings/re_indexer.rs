use crate::{
    interfaces::dto::{AdminTask, AdminTaskStatus},
    services::{
        admin_task_service::{AdminTaskService, RE_INDEX_TASK_NAME},
        file_service::{FileCursor, FileService},
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
    #[error("file service failure: {0:#?}")]
    File(#[from] crate::services::file_service::FileServiceError),
    #[error("index service failure: {0:#?}")]
    Index(#[from] crate::services::index_service::IndexServiceError),
    #[error("failed to serialize or deserialize admin task metadata: {0:#?}")]
    MetadataSerde(#[from] serde_json::Error),
}

pub struct ReIndexer {
    admin_task_service: AdminTaskService,
    file_service: FileService,
    index_service: IndexService,
    stop_signal: Mutex<Option<tokio::sync::mpsc::Sender<()>>>,
    task_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl ReIndexer {
    pub fn new(
        admin_task_service: AdminTaskService,
        file_service: FileService,
        index_service: IndexService,
    ) -> Self {
        Self {
            admin_task_service,
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
            kind: Kind::Liftoff,
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
    file_service: FileService,
    index_service: IndexService,
) {
    let mut timer = tokio::time::interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            _ = stop_signal.recv() => {
                return;
            }
            _ = timer.tick() => {
                if let Err(err) = re_index_task_on_tick(
                    &admin_task_service,
                    &file_service,
                    &index_service,
                ).await {
                    log::error!("re-index task on tick error: {err:#?}");
                }
            }
        }
    }
}

async fn re_index_task_on_tick(
    admin_task_service: &AdminTaskService,
    file_service: &FileService,
    index_service: &IndexService,
) -> Result<(), ReIndexerError> {
    let admin_task = admin_task_service
        .get_last_active_task(RE_INDEX_TASK_NAME)
        .await?;
    let admin_task = match admin_task {
        Some(admin_task) => admin_task,
        None => {
            return Ok(());
        }
    };
    let admin_task_id = admin_task.id;

    admin_task_service
        .update_task_status(admin_task_id, AdminTaskStatus::InProgress)
        .await?;

    let result =
        re_index_task_on_tick_for_task(admin_task, admin_task_service, file_service, index_service)
            .await;

    if let Err(err) = result {
        admin_task_service
            .update_task_status(admin_task_id, AdminTaskStatus::Failed)
            .await?;
        return Err(err);
    }

    admin_task_service
        .update_task_status(admin_task_id, AdminTaskStatus::Completed)
        .await?;

    Ok(())
}

async fn re_index_task_on_tick_for_task(
    admin_task: AdminTask,
    admin_task_service: &AdminTaskService,
    file_service: &FileService,
    index_service: &IndexService,
) -> Result<(), ReIndexerError> {
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
            return Ok(());
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

    Ok(())
}
