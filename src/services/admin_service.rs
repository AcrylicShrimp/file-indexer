use crate::{
    db::repositories::admin::{self, AdminRepository},
    interfaces::dto,
    services::token_service::TokenService,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdminServiceError {
    #[error("repository error: {0:#?}")]
    RepositoryError(#[from] crate::db::repositories::RepositoryError),
    #[error("password error: {0:#?}")]
    PwError(#[from] argon2::password_hash::Error),
}

pub struct AdminService {
    admin_repository: AdminRepository,
}

impl AdminService {
    pub fn new(admin_repository: AdminRepository) -> Self {
        Self { admin_repository }
    }

    pub async fn create_admin(
        &self,
        admin: dto::CreatingAdmin,
    ) -> Result<dto::Admin, AdminServiceError> {
        const TOKEN_SERVICE: TokenService = TokenService::new();

        let pw_hash = TOKEN_SERVICE.hash_password(&admin.password)?;
        let admin = self
            .admin_repository
            .create_one(admin::entities::AdminEntityForCreation {
                username: admin.username,
                email: admin.email,
                pw_hash,
            })
            .await?;

        Ok(dto::Admin {
            id: admin.id,
            username: admin.username,
            email: admin.email,
            joined_at: admin.joined_at,
        })
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
