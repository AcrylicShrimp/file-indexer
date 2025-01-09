use crate::{interfaces::dto, services::token_service::TokenService};
use sqlx::PgPool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdminServiceError {
    #[error("database error: {0:#?}")]
    DbError(#[from] sqlx::Error),
    #[error("password error: {0:#?}")]
    PwError(#[from] argon2::password_hash::Error),
}

pub struct AdminService {
    db_pool: PgPool,
}

impl AdminService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn create_admin(
        &self,
        admin: dto::CreatingAdmin,
    ) -> Result<dto::Admin, AdminServiceError> {
        const TOKEN_SERVICE: TokenService = TokenService::new();

        let pw_hash = TOKEN_SERVICE.hash_password(&admin.password)?;
        let admin = sqlx::query_as!(
            row_types::Admin,
            "
INSERT INTO admins (
    username,
    email,
    pw_hash
) VALUES ($1, $2, $3)
RETURNING
    id,
    username,
    email,
    joined_at",
            admin.username,
            admin.email,
            pw_hash,
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(admin.into())
    }
}

mod row_types {
    use crate::interfaces::dto;
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    pub struct Admin {
        pub id: Uuid,
        pub username: String,
        pub email: String,
        pub joined_at: NaiveDateTime,
    }

    impl From<Admin> for dto::Admin {
        fn from(admin: Admin) -> Self {
            Self {
                id: admin.id,
                username: admin.username,
                email: admin.email,
                joined_at: admin.joined_at.and_utc(),
            }
        }
    }
}
