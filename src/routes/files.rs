use crate::{
    interfaces::dto::{
        AdminTaskInitiator, AdminTaskStatus, CreatedFile, CreatingFile, File, FileDownloadUrl,
        FileUploadUrl, UpdatingFile,
    },
    services::{
        admin_task_service::{AdminTaskService, CREATE_FILE_TASK_NAME, UPDATE_FILE_TASK_NAME},
        file_service::{FileCursor, FileService},
        index_service::IndexService,
        s3_service::S3Service,
    },
};
use rocket::{get, http::Status, patch, post, routes, serde::json::Json, Route, State};
use std::vec;
use uuid::Uuid;

pub fn routes() -> Vec<Route> {
    routes![
        files_list,
        files_get,
        files_create_download_url,
        files_create,
        files_create_upload_url,
        files_update,
    ]
}

#[get("/?<query..>")]
async fn files_list(
    file_service: &State<FileService>,
    query: forms::ListQuery,
) -> Result<Json<Vec<File>>, Status> {
    let cursor = match (query.last_file_id, query.last_file_uploaded_at) {
        (Some(last_file_id), Some(last_file_uploaded_at)) => Some(FileCursor {
            id: last_file_id,
            uploaded_at: last_file_uploaded_at.date_time,
        }),
        _ => None,
    };

    let files = match file_service.list_files(query.limit, cursor).await {
        Ok(files) => files,
        Err(err) => {
            log::error!("failed to list files: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(files))
}

#[get("/<file_id>")]
async fn files_get(file_service: &State<FileService>, file_id: Uuid) -> Result<Json<File>, Status> {
    let file = match file_service.get_file(file_id).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to get file: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(file))
}

#[post("/<file_id>/download-urls")]
async fn files_create_download_url(
    s3_service: &State<S3Service>,
    file_id: Uuid,
) -> Result<Json<FileDownloadUrl>, Status> {
    let result = s3_service
        .generate_presigned_url_for_download(file_id)
        .await;
    let (url, expires_at) = match result {
        Ok(Some((url, expires_at))) => (url, expires_at),
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to generate presigned url for download: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(FileDownloadUrl { url, expires_at }))
}

#[post("/", data = "<body>")]
async fn files_create(
    admin_task_service: &State<AdminTaskService>,
    file_service: &State<FileService>,
    index_service: &State<IndexService>,
    s3_service: &State<S3Service>,
    body: Json<CreatingFile>,
) -> Result<Json<CreatedFile>, Status> {
    let body = body.into_inner();
    let file = match file_service.create_file(body.clone()).await {
        Ok(file) => file,
        Err(err) => {
            log::error!("failed to create file: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    let (upload_url, upload_url_expires_at) = match s3_service
        .generate_presigned_url_for_upload(file.id, &file.mime_type)
        .await
    {
        Ok(url) => url,
        Err(err) => {
            log::error!("failed to generate presigned url for upload: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    let status = match index_service.index_file(&file).await {
        Ok(()) => AdminTaskStatus::Completed,
        Err(err) => {
            log::warn!("failed to index file `{}`: {err:#?}", file.id);
            AdminTaskStatus::Failed
        }
    };

    let result = admin_task_service
        .enqueue_task(
            AdminTaskInitiator::User,
            CREATE_FILE_TASK_NAME.to_owned(),
            serde_json::json!({ "file_id": file.id, "content": body }),
            Some(status),
            false,
        )
        .await;

    if let Err(err) = result {
        log::warn!("failed to enqueue admin task: {err:#?}");
    }

    Ok(Json(CreatedFile {
        id: file.id,
        name: file.name,
        size: file.size,
        mime_type: file.mime_type,
        uploaded_at: file.uploaded_at,
        tags: file.tags,
        upload_url,
        upload_url_expires_at,
    }))
}

#[post("/<file_id>/upload-urls")]
async fn files_create_upload_url(
    file_service: &State<FileService>,
    s3_service: &State<S3Service>,
    file_id: Uuid,
) -> Result<Json<FileUploadUrl>, Status> {
    let mime_type = match file_service.get_file_mime_type(file_id).await {
        Ok(Some(mime_type)) => mime_type,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to get file mime type: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    let result = s3_service
        .generate_presigned_url_for_upload(file_id, &mime_type)
        .await;
    let (url, expires_at) = match result {
        Ok((url, expires_at)) => (url, expires_at),
        Err(err) => {
            log::error!("failed to generate presigned url for upload: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(FileUploadUrl { url, expires_at }))
}

#[patch("/<file_id>", data = "<body>")]
async fn files_update(
    admin_task_service: &State<AdminTaskService>,
    file_service: &State<FileService>,
    index_service: &State<IndexService>,
    file_id: Uuid,
    body: Json<UpdatingFile>,
) -> Result<Json<File>, Status> {
    let body = body.into_inner();
    let file = match file_service.update_file(file_id, body.clone()).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to update file: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    let status = match index_service.index_file(&file).await {
        Ok(()) => AdminTaskStatus::Completed,
        Err(err) => {
            log::warn!("failed to index file `{}`: {err:#?}", file.id);
            AdminTaskStatus::Failed
        }
    };

    let result = admin_task_service
        .enqueue_task(
            AdminTaskInitiator::User,
            UPDATE_FILE_TASK_NAME.to_owned(),
            serde_json::json!({ "file_id": file_id, "delta": body }),
            Some(status),
            false,
        )
        .await;

    if let Err(err) = result {
        log::warn!("failed to enqueue admin task: {err:#?}");
    }

    Ok(Json(file))
}

mod forms {
    use crate::forms::date_time_utc::DateTimeUtcFormField;
    use rocket::{
        form::{Error, Result},
        FromForm,
    };
    use uuid::Uuid;

    #[derive(FromForm, Debug)]
    pub struct ListQuery {
        #[field(name = uncased("limit"), default = 25, validate = range(1..=100))]
        pub limit: usize,
        #[field(name = uncased("last-file-id"), validate = is_last_file_id_valid(&self.last_file_uploaded_at))]
        pub last_file_id: Option<Uuid>,
        #[field(name = uncased("last-file-uploaded-at"), validate = is_last_file_uploaded_at_valid(&self.last_file_id))]
        pub last_file_uploaded_at: Option<DateTimeUtcFormField>,
    }

    fn is_last_file_id_valid<'v>(
        this: &Option<Uuid>,
        last_file_uploaded_at: &Option<DateTimeUtcFormField>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_file_uploaded_at.is_none() {
            Err(Error::validation(
                "`last-file-uploaded-at` must be provided if `last-file-id` is provided",
            ))?;
        }

        Ok(())
    }

    fn is_last_file_uploaded_at_valid<'v>(
        this: &Option<DateTimeUtcFormField>,
        last_file_id: &Option<Uuid>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_file_id.is_none() {
            Err(Error::validation(
                "`last-file-id` must be provided if `last-file-uploaded-at` is provided",
            ))?;
        }

        Ok(())
    }
}
