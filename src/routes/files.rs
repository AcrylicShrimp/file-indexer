use crate::{
    interfaces::dto::{
        AdminTaskInitiator, AdminTaskStatus, CreatedFile, CreatingFile, File, FileDownloadUrl,
        FileUploadUrl, SimpleOk, UpdatingFile, UploadedParts,
    },
    services::{
        admin_task_service::{AdminTaskService, UPLOAD_FILE_TASK_NAME},
        file_service::{FileCursor, FileService},
        index_service::IndexService,
        s3_service::S3Service,
    },
};
use futures::future::try_join_all;
use rocket::{delete, get, http::Status, patch, post, routes, serde::json::Json, Route, State};
use std::{time::Duration, vec};
use uuid::Uuid;

/// 1 hour
const DOWNLOAD_URL_DURATION: Duration = Duration::from_secs(60 * 60);
/// 1 hour
const UPLOAD_URL_DURATION: Duration = Duration::from_secs(60 * 60);

pub fn routes() -> Vec<Route> {
    routes![
        files_list,
        files_get,
        files_create_download_url,
        files_create,
        files_create_upload_url,
        files_complete_upload,
        files_abort_upload,
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
    let now = chrono::Utc::now();
    let url = s3_service
        .generate_presigned_url_for_download(file_id, DOWNLOAD_URL_DURATION)
        .await;
    let url = match url {
        Ok(Some(url)) => url,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to generate presigned url for download: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };
    let expires_at = now + DOWNLOAD_URL_DURATION;

    Ok(Json(FileDownloadUrl { url, expires_at }))
}

#[post("/", data = "<body>")]
async fn files_create(
    file_service: &State<FileService>,
    body: Json<CreatingFile>,
) -> Result<Json<CreatedFile>, Status> {
    let file = match file_service.create_file(body.into_inner()).await {
        Ok(file) => file,
        Err(err) => {
            log::error!("failed to create file: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(CreatedFile {
        id: file.id,
        name: file.name,
        size: file.size,
        mime_type: file.mime_type,
        uploaded_at: file.uploaded_at,
        tags: file.tags,
    }))
}

#[post("/<file_id>/upload-urls")]
async fn files_create_upload_url(
    file_service: &State<FileService>,
    s3_service: &State<S3Service>,
    file_id: Uuid,
) -> Result<Json<FileUploadUrl>, Status> {
    let (size, mime_type) = match file_service.get_file_for_upload(file_id).await {
        Ok(Some((size, mime_type))) => (size, mime_type),
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to get file for upload: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    /// 5 TiB
    const MAX_FILE_SIZE: usize = 1024 * 1024 * 1024 * 1024 * 5;

    if MAX_FILE_SIZE < size {
        return Err(Status::UnprocessableEntity);
    }

    let id = s3_service.create_multipart_upload(file_id, mime_type).await;
    let id = match id {
        Ok(id) => id,
        Err(err) => {
            log::error!("failed to create multipart upload: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    /// 64 MiB
    const PART_SIZE: usize = 1024 * 1024 * 64;

    let count: u32 = if size <= PART_SIZE {
        1
    } else {
        (size / PART_SIZE) as u32
    };

    if 10000 <= count {
        return Err(Status::UnprocessableEntity);
    }

    let now = chrono::Utc::now();
    let mut presigned_url_tasks = Vec::with_capacity(count as usize);

    for part_number in 1..=count {
        presigned_url_tasks.push(s3_service.generate_presigned_url_for_upload(
            file_id,
            &id,
            part_number,
            UPLOAD_URL_DURATION,
        ));
    }

    let urls = try_join_all(presigned_url_tasks).await;
    let urls = match urls {
        Ok(urls) => urls,
        Err(err) => {
            log::error!("failed to generate presigned urls for upload: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(FileUploadUrl {
        id,
        urls,
        expires_at: now + UPLOAD_URL_DURATION,
    }))
}

#[post("/<file_id>/upload-urls/<upload_id>/completes", data = "<body>")]
async fn files_complete_upload(
    admin_task_service: &State<AdminTaskService>,
    file_service: &State<FileService>,
    index_service: &State<IndexService>,
    s3_service: &State<S3Service>,
    file_id: Uuid,
    upload_id: String,
    body: Json<UploadedParts>,
) -> Result<Option<Json<File>>, Status> {
    let body = body.into_inner();
    let file = match file_service.mark_file_as_ready(file_id).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to mark file as ready: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    let parts = body
        .parts
        .iter()
        .map(|part| (part.part_number, part.e_tag.clone()))
        .collect::<Vec<_>>();

    match s3_service
        .complete_multipart_upload(file_id, upload_id, &parts)
        .await
    {
        Ok(Some(())) => {}
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to complete upload: {err:#?}");
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
            UPLOAD_FILE_TASK_NAME.to_owned(),
            serde_json::json!({ "file_id": file.id, "content": body }),
            Some(status),
            false,
        )
        .await;

    if let Err(err) = result {
        log::warn!("failed to enqueue admin task: {err:#?}");
    }

    Ok(Some(Json(file)))
}

#[delete("/<file_id>/upload-urls/<upload_id>")]
async fn files_abort_upload(
    s3_service: &State<S3Service>,
    file_id: Uuid,
    upload_id: String,
) -> Result<Json<SimpleOk>, Status> {
    let result = s3_service.abort_multipart_upload(file_id, upload_id).await;
    let result = match result {
        Ok(Some(())) => SimpleOk { ok: true },
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to abort multipart upload: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(result))
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
            UPLOAD_FILE_TASK_NAME.to_owned(),
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
