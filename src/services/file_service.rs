use crate::{
    db::repositories::file::{self, FileRepository},
    interfaces::files,
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum FileServiceError {
    #[error("repository error: {0:#?}")]
    RepositoryError(#[from] crate::db::repositories::RepositoryError),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileCursor {
    pub id: Uuid,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct FileService {
    file_repository: FileRepository,
}

impl FileService {
    pub fn new(file_repository: FileRepository) -> Self {
        Self { file_repository }
    }

    pub async fn get_file(&self, file_id: Uuid) -> Result<Option<files::File>, FileServiceError> {
        let file = self.file_repository.find_one_by_id(file_id).await?;

        Ok(file.map(|file| files::File {
            id: file.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at,
            tags: file.tags,
        }))
    }

    pub async fn get_file_for_upload(
        &self,
        file_id: Uuid,
    ) -> Result<Option<(usize, String)>, FileServiceError> {
        let result = self.file_repository.find_one_for_upload(file_id).await?;

        Ok(result.map(|result| (result.size, result.mime_type)))
    }

    pub async fn list_files(
        &self,
        limit: usize,
        cursor: Option<FileCursor>,
    ) -> Result<Vec<files::File>, FileServiceError> {
        let cursor = cursor.map(|cursor| file::entities::FileCursorEntity {
            id: cursor.id,
            uploaded_at: cursor.uploaded_at,
        });
        let files = self.file_repository.list(limit, cursor).await?;

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

    pub async fn create_file(
        &self,
        file: files::CreatingFile,
    ) -> Result<files::File, FileServiceError> {
        let file = self
            .file_repository
            .create_one(file::entities::FileEntityForCreation {
                name: file.name,
                size: file.size,
                mime_type: file.mime_type,
                tags: file.tags.unwrap_or_default(),
            })
            .await?;

        Ok(files::File {
            id: file.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at,
            tags: file.tags,
        })
    }

    pub async fn update_file(
        &self,
        file_id: Uuid,
        file: files::UpdatingFile,
    ) -> Result<Option<files::File>, FileServiceError> {
        let file = self
            .file_repository
            .update_one(
                file::entities::FileEntityForUpdate {
                    id: file_id,
                    name: file.name,
                    size: file.size,
                    mime_type: file.mime_type,
                },
                file.tags_for_creation.unwrap_or_default(),
                file.tags_for_deletion.unwrap_or_default(),
            )
            .await?;

        Ok(file.map(|file| files::File {
            id: file.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at,
            tags: file.tags,
        }))
    }

    pub async fn mark_file_as_ready(
        &self,
        file_id: Uuid,
    ) -> Result<Option<files::File>, FileServiceError> {
        let file = self.file_repository.update_one_as_ready(file_id).await?;

        Ok(file.map(|file| files::File {
            id: file.id,
            name: file.name,
            size: file.size,
            mime_type: file.mime_type,
            uploaded_at: file.uploaded_at,
            tags: file.tags,
        }))
    }

    pub async fn delete_file(&self, file_id: Uuid) -> Result<(), FileServiceError> {
        self.file_repository.delete_one(file_id).await?;

        Ok(())
    }

    pub async fn delete_unready_files(
        &self,
        before_uploaded_at: DateTime<Utc>,
    ) -> Result<(), FileServiceError> {
        self.file_repository
            .delete_unready_many(before_uploaded_at)
            .await?;

        Ok(())
    }
}
