use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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
pub struct AdminTask {
    pub id: Uuid,
    pub initiator: AdminTaskInitiator,
    pub name: String,
    pub metadata: Value,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: Uuid,
    pub name: String,
    pub size: usize,
    pub mime_type: String,
    pub uploaded_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileDownloadUrl {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreatingFile {
    pub name: String,
    pub size: usize,
    pub mime_type: String,
    pub tags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreatedFile {
    pub id: Uuid,
    pub name: String,
    pub size: usize,
    pub mime_type: String,
    pub uploaded_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub upload_url: String,
    pub upload_url_expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileUploadUrl {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdatingFile {
    pub name: Option<String>,
    pub size: Option<usize>,
    pub mime_type: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchQuery {
    pub q: String,
    #[serde(default = "file_search_query_default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub filters: Vec<Vec<FileSearchQueryFilter>>,
}

fn file_search_query_default_limit() -> usize {
    25
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum FileSearchQueryFilter {
    Size {
        operator: FileSearchQueryFilterOperator,
        value: usize,
    },
    MimeType {
        value: String,
    },
    Tag {
        value: String,
    },
    TagIsEmpty,
    TagIsNotEmpty,
    UploadedAt {
        operator: FileSearchQueryFilterOperator,
        value: DateTime<Utc>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum FileSearchQueryFilterOperator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

impl FileSearchQueryFilterOperator {
    pub fn to_str(self) -> &'static str {
        match self {
            FileSearchQueryFilterOperator::Eq => "=",
            FileSearchQueryFilterOperator::Neq => "!=",
            FileSearchQueryFilterOperator::Gt => ">",
            FileSearchQueryFilterOperator::Gte => ">=",
            FileSearchQueryFilterOperator::Lt => "<",
            FileSearchQueryFilterOperator::Lte => "<=",
        }
    }
}
