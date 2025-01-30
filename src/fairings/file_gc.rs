use crate::services::s3_service::S3Service;
use rocket::fairing::{Fairing, Info, Kind};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum FileGcError {
    #[error("s3 service failure: {0:#?}")]
    S3(#[from] crate::services::s3_service::S3ServiceError),
}

pub struct FileGc {
    s3_service: S3Service,
    stop_signal: Mutex<Option<tokio::sync::mpsc::Sender<()>>>,
    task_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl FileGc {
    pub fn new(s3_service: S3Service) -> Self {
        Self {
            s3_service,
            stop_signal: Mutex::new(None),
            task_handle: Mutex::new(None),
        }
    }
}

impl Fairing for FileGc {
    fn info(&self) -> Info {
        Info {
            name: "file_gc",
            kind: Kind::Ignite | Kind::Shutdown,
        }
    }
}
