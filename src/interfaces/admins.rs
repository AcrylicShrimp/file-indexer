use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Admin {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreatingAdmin {
    pub username: String,
    pub password: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdminTaskPreview {
    pub id: Uuid,
    pub initiator: AdminTaskInitiator,
    pub name: String,
    pub status: AdminTaskStatus,
    pub enqueued_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReIndexAdminTask {
    pub file_task: AdminTask,
    pub collection_task: AdminTask,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdminTask {
    pub id: Uuid,
    pub initiator: AdminTaskInitiator,
    pub name: String,
    pub metadata: serde_json::Value,
    pub status: AdminTaskStatus,
    pub enqueued_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "admin_task_initiator")]
#[sqlx(rename_all = "snake_case")]
pub enum AdminTaskInitiator {
    User,
    System,
}

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "admin_task_status")]
#[sqlx(rename_all = "snake_case")]
pub enum AdminTaskStatus {
    Pending,
    InProgress,
    Canceled,
    Completed,
    Failed,
}
