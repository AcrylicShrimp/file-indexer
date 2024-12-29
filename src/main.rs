#![forbid(unsafe_code)]

mod db;
mod fairings;
mod forms;
mod interfaces;
mod routes;
mod services;

use fairings::re_indexer::ReIndexer;
use services::{
    admin_task_service::AdminTaskService, file_service::FileService, index_service::IndexService,
    s3_service::S3Service,
};
use std::net::{IpAddr, Ipv4Addr};

#[rocket::launch]
async fn rocket() -> _ {
    let database = db::database::Database::init()
        .await
        .expect("failed to initialize database module");
    let search_engine = db::search_engine::SearchEngine::init()
        .await
        .expect("failed to initialize search engine module");

    let s3_service = S3Service::init()
        .await
        .expect("failed to initialize s3 service");

    let admin_task_service = AdminTaskService::new(database.pool());
    let file_service = FileService::new(database.pool());
    let index_service = IndexService::new(search_engine.into_client());

    let re_indexer = ReIndexer::new(
        admin_task_service.clone(),
        file_service.clone(),
        index_service.clone(),
    );

    let config = rocket::Config {
        address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port: 8000,
        ..rocket::Config::default()
    };
    let rocket = rocket::custom(&config)
        .attach(re_indexer)
        .manage(admin_task_service)
        .manage(file_service)
        .manage(index_service)
        .manage(s3_service);
    let rocket = routes::register_root(rocket);

    #[allow(clippy::let_and_return)]
    rocket
}
