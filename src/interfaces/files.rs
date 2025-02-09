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
pub struct FileCursor {
    pub id: Uuid,
    pub uploaded_at: DateTime<Utc>,
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
pub struct FileUploadUrl {
    pub id: String,
    pub parts: Vec<FileUploadUrlPart>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileUploadUrlPart {
    pub part_number: u32,
    pub url: String,
    pub offset: u64,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UploadedParts {
    pub parts: Vec<UploadedPart>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UploadedPart {
    pub part_number: u32,
    pub e_tag: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdatingFile {
    pub name: Option<String>,
    pub size: Option<usize>,
    pub mime_type: Option<String>,
    pub tags_for_creation: Option<Vec<String>>,
    pub tags_for_deletion: Option<Vec<String>>,
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
