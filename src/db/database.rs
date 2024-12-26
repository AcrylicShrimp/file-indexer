use sqlx::{migrate, migrate::Migrator, PgPool};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("environment variable `DATABASE_URL` is unable to be retrieved: {0:#?}")]
    RetrieveDatabaseUrl(std::env::VarError),

    #[error("database connection failure: {0:#?}")]
    DatabaseConnectionFailure(#[from] sqlx::Error),

    #[error("database migration failure: {0:#?}")]
    DatabaseMigrationFailure(#[from] sqlx::migrate::MigrateError),
}

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn init() -> Result<Self, DatabaseError> {
        static MIGRATOR: Migrator = migrate!("src/db/migrations");

        let database_url =
            std::env::var("DATABASE_URL").map_err(DatabaseError::RetrieveDatabaseUrl)?;
        let pool = PgPool::connect(&database_url)
            .await
            .map_err(DatabaseError::DatabaseConnectionFailure)?;

        MIGRATOR
            .run(&pool)
            .await
            .map_err(DatabaseError::DatabaseMigrationFailure)?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }
}
