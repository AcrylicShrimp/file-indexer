use crate::{
    interfaces::dto::{AdminTask, AdminTaskInitiator, AdminTaskPreview},
    services::{
        admin_task_service::{AdminTaskCursor, AdminTaskService},
        index_service::IndexService,
    },
};
use rocket::{get, http::Status, post, routes, serde::json::Json, Route, State};
use uuid::Uuid;

pub fn routes() -> Vec<Route> {
    routes![admin_tasks_list, admin_tasks_get, admin_tasks_re_index,]
}

#[get("/?<query..>")]
async fn admin_tasks_list(
    admin_task_service: &State<AdminTaskService>,
    query: forms::ListQuery,
) -> Result<Json<Vec<AdminTaskPreview>>, Status> {
    let cursor = match (query.last_admin_task_id, query.last_admin_task_updated_at) {
        (Some(last_admin_task_id), Some(last_admin_task_updated_at)) => Some(AdminTaskCursor {
            id: last_admin_task_id,
            updated_at: last_admin_task_updated_at.date_time,
        }),
        _ => None,
    };

    let tasks = match admin_task_service.list_tasks(query.limit, cursor).await {
        Ok(tasks) => tasks,
        Err(err) => {
            log::error!("failed to list admin tasks: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(tasks))
}

#[get("/<task_id>")]
async fn admin_tasks_get(
    admin_task_service: &State<AdminTaskService>,
    task_id: Uuid,
) -> Result<Json<AdminTask>, Status> {
    let task = match admin_task_service.get_task(task_id).await {
        Ok(Some(task)) => task,
        Ok(None) => {
            return Err(Status::NotFound);
        }
        Err(err) => {
            log::error!("failed to get admin task: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(task))
}

#[post("/re-index")]
async fn admin_tasks_re_index(
    admin_task_service: &State<AdminTaskService>,
    index_service: &State<IndexService>,
) -> Result<Json<AdminTask>, Status> {
    if let Err(err) = index_service.empty_index().await {
        log::error!("failed to empty index: {err:#?}");
        return Err(Status::InternalServerError);
    }

    let admin_task = admin_task_service
        .enqueue_task(
            AdminTaskInitiator::User,
            "re-index".to_owned(),
            serde_json::json!({
                "last_file_id": serde_json::Value::Null,
                "last_file_uploaded_at": serde_json::Value::Null,
            }),
            None,
            true,
        )
        .await;

    let admin_task = match admin_task {
        Ok(admin_task) => admin_task,
        Err(err) => {
            log::error!("failed to enqueue admin task: {err:#?}");
            return Err(Status::InternalServerError);
        }
    };

    Ok(Json(admin_task))
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
        #[field(name = uncased("last-admin-task-id"), validate = is_last_admin_task_id_valid(&self.last_admin_task_updated_at))]
        pub last_admin_task_id: Option<Uuid>,
        #[field(name = uncased("last-admin-task-updated-at"), validate = is_last_admin_task_updated_at_valid(&self.last_admin_task_id))]
        pub last_admin_task_updated_at: Option<DateTimeUtcFormField>,
    }

    fn is_last_admin_task_id_valid<'v>(
        this: &Option<Uuid>,
        last_admin_task_updated_at: &Option<DateTimeUtcFormField>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_admin_task_updated_at.is_none() {
            Err(Error::validation(
                "`last-admin-task-updated-at` must be provided if `last-admin-task-id` is provided",
            ))?;
        }

        Ok(())
    }

    fn is_last_admin_task_updated_at_valid<'v>(
        this: &Option<DateTimeUtcFormField>,
        last_admin_task_id: &Option<Uuid>,
    ) -> Result<'v, ()> {
        if this.is_some() && last_admin_task_id.is_none() {
            Err(Error::validation(
                "`last-admin-task-id` must be provided if `last-admin-task-updated-at` is provided",
            ))?;
        }

        Ok(())
    }
}
