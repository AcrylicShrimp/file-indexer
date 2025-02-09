use crate::{
    interfaces::{
        collections::{
            Collection, CollectionCursor, CollectionFileCursor, CreatingCollection,
            UpdatingCollection,
        },
        files::File,
        SimpleOk,
    },
    services::collection_service::CollectionService,
};
use rocket::{delete, get, http::Status, patch, post, routes, serde::json::Json, Route, State};
use uuid::Uuid;

pub fn routes() -> Vec<Route> {
    routes![
        collections_list,
        collections_get,
        collections_list_files,
        collections_create,
        collections_update,
        collections_delete,
    ]
}

#[get("/?<query..>")]
async fn collections_list(
    collection_service: &State<CollectionService>,
    query: forms::CollectionListQuery,
) -> Result<Json<Vec<Collection>>, Status> {
    let cursor = match (query.last_collection_id, query.last_collection_name) {
        (Some(last_collection_id), Some(last_collection_name)) => Some(CollectionCursor {
            id: last_collection_id,
            name: last_collection_name,
        }),
        _ => None,
    };

    let collections = match collection_service
        .list_collections(query.limit, cursor)
        .await
    {
        Ok(collections) => collections,
        Err(err) => {
            log::error!("failed to list collections: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(collections))
}

#[get("/<collection_id>")]
async fn collections_get(
    collection_service: &State<CollectionService>,
    collection_id: Uuid,
) -> Result<Json<Collection>, Status> {
    let collection = match collection_service.get_collection(collection_id).await {
        Ok(Some(collection)) => collection,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to get collection: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(collection))
}

#[get("/<collection_id>/files?<query..>")]
async fn collections_list_files(
    collection_service: &State<CollectionService>,
    collection_id: Uuid,
    query: forms::CollectionFileListQuery,
) -> Result<Json<Vec<File>>, Status> {
    let cursor = match (query.last_file_id, query.last_file_name) {
        (Some(last_file_id), Some(last_file_name)) => Some(CollectionFileCursor {
            id: last_file_id,
            name: last_file_name,
        }),
        _ => None,
    };

    let files = match collection_service
        .list_collection_files(collection_id, query.limit, cursor)
        .await
    {
        Ok(files) => files,
        Err(err) => {
            log::error!("failed to list collection files: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(files))
}

#[post("/", data = "<body>")]
async fn collections_create(
    collection_service: &State<CollectionService>,
    body: Json<CreatingCollection>,
) -> Result<Json<Collection>, Status> {
    let collection = match collection_service
        .create_collection(body.into_inner())
        .await
    {
        Ok(collection) => collection,
        Err(err) => {
            log::error!("failed to create collection: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(collection))
}

#[patch("/<collection_id>", data = "<body>")]
async fn collections_update(
    collection_service: &State<CollectionService>,
    collection_id: Uuid,
    body: Json<UpdatingCollection>,
) -> Result<Json<Collection>, Status> {
    let collection = match collection_service
        .update_collection(collection_id, body.into_inner())
        .await
    {
        Ok(Some(collection)) => collection,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to update collection: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(collection))
}

#[delete("/<collection_id>")]
async fn collections_delete(
    collection_service: &State<CollectionService>,
    collection_id: Uuid,
) -> Result<Json<SimpleOk>, Status> {
    if let Err(err) = collection_service.delete_collection(collection_id).await {
        log::error!("failed to delete collection: {err:#?}");
        return Err(Status::InternalServerError);
    }

    Ok(Json(SimpleOk { ok: true }))
}

mod forms {
    use rocket::{
        form::{Error, Result},
        FromForm,
    };
    use uuid::Uuid;

    #[derive(FromForm, Debug)]
    pub struct CollectionListQuery {
        #[field(name = uncased("limit"), default = 25, validate = range(1..=100))]
        pub limit: usize,
        #[field(name = uncased("last-collection-id"), validate = __collection_list_query_is_last_collection_id_valid(&self.last_collection_name))]
        pub last_collection_id: Option<Uuid>,
        #[field(name = uncased("last-collection-name"), validate = __collection_list_query_is_last_collection_name_valid(&self.last_collection_id))]
        pub last_collection_name: Option<String>,
    }

    fn __collection_list_query_is_last_collection_id_valid<'v>(
        this: &Option<Uuid>,
        last_collection_name: &Option<String>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_collection_name.is_none() {
            Err(Error::validation(
                "`last-collection-name` must be provided if `last-collection-id` is provided",
            ))?;
        }

        Ok(())
    }

    fn __collection_list_query_is_last_collection_name_valid<'v>(
        this: &Option<String>,
        last_collection_id: &Option<Uuid>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_collection_id.is_none() {
            Err(Error::validation(
                "`last-collection-id` must be provided if `last-collection-name` is provided",
            ))?;
        }

        Ok(())
    }

    #[derive(FromForm, Debug)]
    pub struct CollectionFileListQuery {
        #[field(name = uncased("limit"), default = 25, validate = range(1..=100))]
        pub limit: usize,
        #[field(name = uncased("last-file-id"), validate = __collection_file_list_query_is_last_file_id_valid(&self.last_file_name))]
        pub last_file_id: Option<Uuid>,
        #[field(name = uncased("last-file-name"), validate = __collection_file_list_query_is_last_file_name_valid(&self.last_file_id))]
        pub last_file_name: Option<String>,
    }

    fn __collection_file_list_query_is_last_file_id_valid<'v>(
        this: &Option<Uuid>,
        last_file_name: &Option<String>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_file_name.is_none() {
            Err(Error::validation(
                "`last-file-name` must be provided if `last-file-id` is provided",
            ))?;
        }

        Ok(())
    }

    fn __collection_file_list_query_is_last_file_name_valid<'v>(
        this: &Option<String>,
        last_file_id: &Option<Uuid>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_file_id.is_none() {
            Err(Error::validation(
                "`last-file-id` must be provided if `last-file-name` is provided",
            ))?;
        }

        Ok(())
    }
}
