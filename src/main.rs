#![forbid(unsafe_code)]

mod db;
mod forms;
mod interfaces;
mod routes;
mod services;

use db::Db;
use rocket::launch;
use services::file_service::FileService;

#[launch]
async fn rocket() -> _ {
    let db = Db::init()
        .await
        .expect("failed to initialize database module");
    let file_service = FileService::new(db.pool());

    let rocket = rocket::build().manage(file_service);
    let rocket = routes::register_root(rocket);

    #[allow(clippy::let_and_return)]
    rocket
}
