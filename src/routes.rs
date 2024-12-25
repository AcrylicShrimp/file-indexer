mod files;

use rocket::{catch, catchers, http::Status, Build, Request, Rocket};

pub fn register_root(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket
        .register("/", catchers![default])
        .mount("/files", files::routes())
}

#[catch(default)]
fn default(status: Status, _req: &Request) -> String {
    format!("{}", status)
}
