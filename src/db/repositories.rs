use thiserror::Error;

pub mod admin;
pub mod collections;
pub mod file;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("database error: {0:#?}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("duplicated entity: `{key}` = `{value}`")]
    Conflict { key: String, value: String },
}

impl RepositoryError {
    pub fn from_sqlx_err(err: sqlx::Error, f: impl FnOnce(&str) -> String) -> Self {
        match err {
            sqlx::Error::Database(err) if err.is_unique_violation() => {
                let key = err.constraint().unwrap_or("__unknown__").to_owned();
                let value = f(&key);
                Self::Conflict { key, value }
            }
            err => err.into(),
        }
    }
}
