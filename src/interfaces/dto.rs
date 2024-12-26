use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub struct CreatingFile {
    pub name: String,
    pub size: usize,
    pub mime_type: String,
    pub tags: Option<Vec<String>>,
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
    pub limit: usize,
    pub filters: Vec<Vec<FileSearchQueryFilter>>,
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
