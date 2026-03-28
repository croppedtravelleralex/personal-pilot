use std::path::Path;

use anyhow::Result;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, Sqlite};
use tokio::fs;

use super::schema::ALL_SCHEMA_SQL;

pub type DbPool = Pool<Sqlite>;

async fn ensure_sqlite_parent_dir(database_url: &str) -> Result<()> {
    if let Some(path_str) = database_url.strip_prefix("sqlite://") {
        let path = Path::new(path_str);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).await?;
            }
        }
    }
    Ok(())
}

pub async fn init_db(database_url: &str) -> Result<DbPool> {
    ensure_sqlite_parent_dir(database_url).await?;

    let options = database_url
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    for stmt in ALL_SCHEMA_SQL {
        sqlx::query(stmt).execute(&pool).await?;
    }

    Ok(pool)
}
