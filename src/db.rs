use sqlx::{migrate, migrate::Migrator, PgPool};

pub type DbError = sqlx::Error;

pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn init() -> Result<Self, DbError> {
        static MIGRATOR: Migrator = migrate!("src/db/migrations");

        let database_url =
            std::env::var("DATABASE_URL").expect("environment variable `DATABASE_URL` is not set");
        let pool = PgPool::connect(&database_url).await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }
}
