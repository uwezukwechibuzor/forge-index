//! `forge migrate` — run database migrations manually.

/// Runs the migrate command.
pub async fn run(database_url: Option<String>) -> anyhow::Result<()> {
    let url = database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No database URL provided. Use --database-url or set DATABASE_URL env var."
            )
        })?;

    println!("Connecting to database...");

    let pool = sqlx::PgPool::connect(&url).await?;
    let cache_store = forge_index_rpc::RpcCacheStore::new(pool);
    cache_store.setup().await?;

    println!("✅ Migrations applied successfully");
    Ok(())
}
