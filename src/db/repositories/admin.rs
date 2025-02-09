use super::RepositoryError;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AdminRepository {
    db_pool: PgPool,
}

impl AdminRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn find_one_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<entities::AdminEntity>, RepositoryError> {
        let admin = sqlx::query_as!(
            row_types::RawAdmin,
            "
SELECT
    id,
    username,
    email,
    joined_at
FROM admins
WHERE id = $1",
            id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(admin.map(|raw| raw.into()))
    }

    pub async fn find_one_by_username_for_login(
        &self,
        username: impl AsRef<str>,
    ) -> Result<Option<entities::AdminEntityForLogin>, RepositoryError> {
        let for_login = sqlx::query_as!(
            row_types::RawAdminForLogin,
            "
SELECT
    id,
    pw_hash
FROM admins
WHERE username = $1",
            username.as_ref()
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(for_login.map(|raw| raw.into()))
    }

    pub async fn find_one_by_email_for_login(
        &self,
        email: impl AsRef<str>,
    ) -> Result<Option<entities::AdminEntityForLogin>, RepositoryError> {
        let for_login = sqlx::query_as!(
            row_types::RawAdminForLogin,
            "
SELECT
    id,
    pw_hash
FROM admins
WHERE email = $1",
            email.as_ref()
        )
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(for_login.map(|raw| raw.into()))
    }

    pub async fn create_one(
        &self,
        admin: entities::AdminEntityForCreation,
    ) -> Result<entities::AdminEntity, RepositoryError> {
        let after_creation = sqlx::query_as!(
            row_types::RawAdminAfterCreation,
            "
INSERT INTO admins (
    username,
    email,
    pw_hash
) VALUES ($1, $2, $3)
RETURNING
    id,
    joined_at",
            admin.username,
            admin.email,
            admin.pw_hash,
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|err| {
            RepositoryError::from_sqlx_err(err, |index| match index {
                "admins_idx_username" => admin.username.clone(),
                "admins_idx_email" => admin.email.clone(),
                _ => "__unknown__".to_owned(),
            })
        })?;

        Ok(entities::AdminEntity {
            id: after_creation.id,
            username: admin.username,
            email: admin.email,
            joined_at: after_creation.joined_at.and_utc(),
        })
    }

    pub async fn update_one(
        &self,
        admin: entities::AdminEntityForUpdate,
    ) -> Result<entities::AdminEntity, RepositoryError> {
        let after_update = sqlx::query_as!(
            row_types::RawAdminAfterUpdate,
            "
UPDATE admins SET
    username = COALESCE($1, username),
    email = COALESCE($2, email),
    pw_hash = COALESCE($3, pw_hash)
WHERE id = $4
RETURNING
    username,
    email,
    joined_at",
            admin.username,
            admin.email,
            admin.pw_hash,
            admin.id,
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|err| {
            RepositoryError::from_sqlx_err(err, |index| match index {
                "admins_idx_username" => admin.username.unwrap_or("__unknown__".to_owned()),
                "admins_idx_email" => admin.email.unwrap_or("__unknown__".to_owned()),
                _ => "__unknown__".to_owned(),
            })
        })?;

        Ok(entities::AdminEntity {
            id: admin.id,
            username: after_update.username,
            email: after_update.email,
            joined_at: after_update.joined_at.and_utc(),
        })
    }
}

pub mod row_types {
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    pub struct RawAdmin {
        pub id: Uuid,
        pub username: String,
        pub email: String,
        pub joined_at: NaiveDateTime,
    }

    pub struct RawAdminForLogin {
        pub id: Uuid,
        pub pw_hash: String,
    }

    pub struct RawAdminAfterCreation {
        pub id: Uuid,
        pub joined_at: NaiveDateTime,
    }

    pub struct RawAdminAfterUpdate {
        pub username: String,
        pub email: String,
        pub joined_at: NaiveDateTime,
    }
}

pub mod entities {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AdminEntity {
        pub id: Uuid,
        pub username: String,
        pub email: String,
        pub joined_at: DateTime<Utc>,
    }

    impl From<super::row_types::RawAdmin> for AdminEntity {
        fn from(raw: super::row_types::RawAdmin) -> Self {
            Self {
                id: raw.id,
                username: raw.username,
                email: raw.email,
                joined_at: raw.joined_at.and_utc(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AdminEntityForLogin {
        pub id: Uuid,
        pub pw_hash: String,
    }

    impl From<super::row_types::RawAdminForLogin> for AdminEntityForLogin {
        fn from(raw: super::row_types::RawAdminForLogin) -> Self {
            Self {
                id: raw.id,
                pw_hash: raw.pw_hash,
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AdminEntityForCreation {
        pub username: String,
        pub email: String,
        pub pw_hash: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AdminEntityAfterCreation {
        pub id: Uuid,
        pub joined_at: DateTime<Utc>,
    }

    impl From<super::row_types::RawAdminAfterCreation> for AdminEntityAfterCreation {
        fn from(raw: super::row_types::RawAdminAfterCreation) -> Self {
            Self {
                id: raw.id,
                joined_at: raw.joined_at.and_utc(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AdminEntityForUpdate {
        pub id: Uuid,
        pub username: Option<String>,
        pub email: Option<String>,
        pub pw_hash: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AdminEntityAfterUpdate {
        pub username: String,
        pub email: String,
        pub joined_at: DateTime<Utc>,
    }

    impl From<super::row_types::RawAdminAfterUpdate> for AdminEntityAfterUpdate {
        fn from(raw: super::row_types::RawAdminAfterUpdate) -> Self {
            Self {
                username: raw.username,
                email: raw.email,
                joined_at: raw.joined_at.and_utc(),
            }
        }
    }
}
