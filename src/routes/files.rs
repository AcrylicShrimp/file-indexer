use crate::{
    interfaces::dto::{
        AdminTaskInitiator, AdminTaskStatus, CreatingFile, File, FileSearchQuery, UpdatingFile,
    },
    services::{
        admin_task_service::{AdminTaskService, CREATE_FILE_TASK_NAME, UPDATE_FILE_TASK_NAME},
        file_service::{FileCursor, FileService},
        index_service::IndexService,
    },
};
use rocket::{get, http::Status, patch, post, routes, serde::json::Json, Route, State};
use std::vec;
use uuid::Uuid;

pub fn routes() -> Vec<Route> {
    routes![list, get, create, update, search]
}

#[get("/?<query..>")]
async fn list(
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
async fn get(file_service: &State<FileService>, file_id: Uuid) -> Result<Json<File>, Status> {
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

#[post("/", data = "<body>")]
async fn create(
    admin_task_service: &State<AdminTaskService>,
    file_service: &State<FileService>,
    index_service: &State<IndexService>,
    body: Json<CreatingFile>,
) -> Result<Json<File>, Status> {
    let body = body.into_inner();
    let file = match file_service.create_file(body.clone()).await {
        Ok(file) => file,
        Err(err) => {
            log::error!("failed to create file: {err:#?}");
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

    Ok(Json(file))
}

#[patch("/<file_id>", data = "<body>")]
async fn update(
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

#[post("/searches", data = "<query>")]
async fn search(
    index_service: &State<IndexService>,
    query: Json<FileSearchQuery>,
) -> Result<Json<Vec<File>>, Status> {
    let files = match index_service.search_files(&query.into_inner()).await {
        Ok(files) => files,
        Err(err) => {
            log::error!("failed to search files: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(files))
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
        #[field(name = uncased("limit"), default = 25, validate = range(2..=100))]
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
