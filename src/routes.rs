mod admin_tasks;
mod files;
mod searches;

use rocket::{catch, catchers, http::Status, serde::json::Json, Build, Request, Rocket};
use serde::Serialize;

pub fn register_root(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket
        .register("/", catchers![default])
        .mount("/admin-tasks", admin_tasks::routes())
        .mount("/files", files::routes())
        .mount("/searches", searches::routes())
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    pub status: u16,
    pub message: Option<&'a str>,
}

#[catch(default)]
fn default(status: Status, _req: &Request) -> Json<ErrorBody<'static>> {
    Json(ErrorBody {
        status: status.code,
        message: status.reason(),
    })
}
