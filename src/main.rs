#![forbid(unsafe_code)]

mod db;
mod forms;
mod interfaces;
mod routes;
mod services;

use rocket::launch;
use services::{
    admin_task_service::AdminTaskService, file_service::FileService, index_service::IndexService,
};

#[launch]
async fn rocket() -> _ {
    let database = db::database::Database::init()
        .await
        .expect("failed to initialize database module");
    let search_engine = db::search_engine::SearchEngine::init()
        .await
        .expect("failed to initialize search engine module");

    let admin_task_service = AdminTaskService::new(database.pool());
    let file_service = FileService::new(database.pool());
    let index_service = IndexService::new(search_engine.into_client());

    let rocket = rocket::build()
        .manage(admin_task_service)
        .manage(file_service)
        .manage(index_service);
    let rocket = routes::register_root(rocket);

    #[allow(clippy::let_and_return)]
    rocket
}
