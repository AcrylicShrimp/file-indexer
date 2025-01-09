use super::RepositoryError;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminEntity {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub joined_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminEntityForLogin {
    pub id: Uuid,
    pub pw_hash: String,
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
    pub joined_at: NaiveDateTime,
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
    pub joined_at: NaiveDateTime,
}

#[derive(Clone)]
pub struct AdminRepository {
    db_pool: PgPool,
}

impl AdminRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn find_admin_by_id(&self, id: Uuid) -> Result<Option<AdminEntity>, RepositoryError> {
        let admin = sqlx::query_as!(
            AdminEntity,
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

        Ok(admin)
    }

    pub async fn find_admin_by_username_for_login(
        &self,
        username: impl AsRef<str>,
    ) -> Result<Option<AdminEntityForLogin>, RepositoryError> {
        let for_login = sqlx::query_as!(
            AdminEntityForLogin,
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

        Ok(for_login)
    }

    pub async fn find_admin_by_email_for_login(
        &self,
        email: impl AsRef<str>,
    ) -> Result<Option<AdminEntityForLogin>, RepositoryError> {
        let for_login = sqlx::query_as!(
            AdminEntityForLogin,
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

        Ok(for_login)
    }

    pub async fn create_admin(
        &self,
        admin: AdminEntityForCreation,
    ) -> Result<AdminEntity, RepositoryError> {
        let after_creation = sqlx::query_as!(
            AdminEntityAfterCreation,
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

        Ok(AdminEntity {
            id: after_creation.id,
            username: admin.username,
            email: admin.email,
            joined_at: after_creation.joined_at,
        })
    }

    pub async fn update_admin(
        &self,
        admin: AdminEntityForUpdate,
    ) -> Result<AdminEntity, RepositoryError> {
        let after_update = sqlx::query_as!(
            AdminEntityAfterUpdate,
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

        Ok(AdminEntity {
            id: admin.id,
            username: after_update.username,
            email: after_update.email,
            joined_at: after_update.joined_at,
        })
    }
}
