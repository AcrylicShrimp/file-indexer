use crate::{
    interfaces::admins::{AdminTaskInitiator, AdminTaskStatus},
    services::{
        admin_task_service::{AdminTaskService, FILE_GC_TASK_NAME},
        file_service::FileService,
    },
};
use chrono::Utc;
use rocket::{
    async_trait,
    fairing::{Fairing, Info, Kind},
    Orbit, Rocket,
};
use std::time::Duration;
use tokio::sync::Mutex;

pub struct FileGc {
    admin_task_service: AdminTaskService,
    file_service: FileService,
    stop_signal: Mutex<Option<tokio::sync::mpsc::Sender<()>>>,
    task_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl FileGc {
    pub fn new(admin_task_service: AdminTaskService, file_service: FileService) -> Self {
        Self {
            admin_task_service,
            file_service,
            stop_signal: Mutex::new(None),
            task_handle: Mutex::new(None),
        }
    }

    async fn create_file_gc_task(&self) {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let task_handle = tokio::spawn(file_gc_task(
            rx,
            self.admin_task_service.clone(),
            self.file_service.clone(),
        ));

        *self.stop_signal.lock().await = Some(tx);
        *self.task_handle.lock().await = Some(task_handle);
    }
}

#[async_trait]
impl Fairing for FileGc {
    fn info(&self) -> Info {
        Info {
            name: "file_gc",
            kind: Kind::Ignite | Kind::Shutdown,
        }
    }

    async fn on_liftoff(&self, _rocket: &Rocket<Orbit>) {
        self.create_file_gc_task().await;
    }

    async fn on_shutdown(&self, _rocket: &Rocket<Orbit>) {
        if let Some(tx) = self.stop_signal.lock().await.take() {
            if let Err(err) = tx.send(()).await {
                log::warn!("failed to send stop signal to file gc task: {err:#?}");
                return;
            }
        }

        if let Some(task_handle) = self.task_handle.lock().await.take() {
            if let Err(err) = task_handle.await {
                log::warn!("failed to wait for file gc task to finish: {err:#?}");
            }
        }
    }
}

async fn file_gc_task(
    mut stop_signal: tokio::sync::mpsc::Receiver<()>,
    admin_task_service: AdminTaskService,
    file_service: FileService,
) {
    // 6 hours
    let duration_secs = 60 * 60 * 6;

    loop {
        let mut timer = tokio::time::interval(Duration::from_secs(duration_secs));

        tokio::select! {
            _ = stop_signal.recv() => {
                return;
            }
            _ = timer.tick() => {
                file_gc_task_on_tick(
                    &admin_task_service,
                    &file_service,
                ).await;
            }
        }
    }
}

async fn file_gc_task_on_tick(admin_task_service: &AdminTaskService, file_service: &FileService) {
    // 2 hours
    let duration_secs = 60 * 60 * 2;
    let before_uploaded_at = Utc::now() - Duration::from_secs(duration_secs);

    let result = file_service.delete_unready_files(before_uploaded_at).await;
    let metadata = match result {
        Ok(_) => serde_json::json!({
            "success": true,
        }),
        Err(err) => serde_json::json!({ "success": false, "error": err.to_string() }),
    };

    let result = admin_task_service
        .enqueue_task(
            AdminTaskInitiator::System,
            FILE_GC_TASK_NAME.to_owned(),
            metadata,
            Some(AdminTaskStatus::Completed),
            false,
        )
        .await;

    if let Err(err) = result {
        log::warn!("failed to enqueue file gc task: {err:#?}");
    }
}
