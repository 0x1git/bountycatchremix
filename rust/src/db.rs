use anyhow::{Context, Result};
use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

use crate::config::PostgresConfig;

pub async fn create_pool(config: &PostgresConfig) -> Result<Pool> {
    let mut cfg = Config::new();
    cfg.host = Some(config.host.clone());
    cfg.port = Some(config.port);
    cfg.dbname = Some(config.database.clone());
    cfg.user = Some(config.user.clone());
    cfg.password = Some(config.password.clone());

    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .context("Failed to create connection pool")?;

    Ok(pool)
}

pub async fn init_schema(pool: &Pool) -> Result<()> {
    let client = pool.get().await?;
    
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS domains (domain TEXT PRIMARY KEY)",
            &[],
        )
        .await?;

    client
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_domains_domain ON domains (domain text_pattern_ops)",
            &[],
        )
        .await?;

    Ok(())
}
