use crate::{
    interfaces::files::{File, FileSearchQuery},
    services::index_service::IndexService,
};
use rocket::{http::Status, post, routes, serde::json::Json, Route, State};

pub fn routes() -> Vec<Route> {
    routes![searches_files]
}

#[post("/files", data = "<query>")]
async fn searches_files(
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
