use crate::{
    interfaces::dto::{CreatingFile, File, UpdatingFile},
    services::file_service::{FileCursor, FileService},
};
use rocket::{get, http::Status, patch, post, routes, serde::json::Json, Route, State};
use std::vec;
use uuid::Uuid;

pub fn routes() -> Vec<Route> {
    routes![list, get, create, update]
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

#[post("/", data = "<file>")]
async fn create(
    file_service: &State<FileService>,
    file: Json<CreatingFile>,
) -> Result<Json<File>, Status> {
    let file = match file_service.create_file(file.into_inner()).await {
        Ok(file) => file,
        Err(err) => {
            log::error!("failed to create file: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    // TODO: index the file

    Ok(Json(file))
}

#[patch("/<file_id>", data = "<file>")]
async fn update(
    file_service: &State<FileService>,
    file_id: Uuid,
    file: Json<UpdatingFile>,
) -> Result<Json<File>, Status> {
    let file = match file_service.update_file(file_id, file.into_inner()).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to update file: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    // TODO: re-index the file

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
