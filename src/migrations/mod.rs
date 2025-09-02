use std::{env, fs};
use std::path::{Path, PathBuf};
use sqlx::{postgres::PgPoolOptions, Executor};
use chrono::Utc;

/// Runs pending SQL migrations located in the `migrations/` directory (or MIGRATIONS_DIR env var).
/// Each `*.sql` file is treated as one migration and applied only once.
/// Applied migrations are tracked in the `schema_migrations` table.
pub async fn run_migrations() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL")?;
    let migrations_dir = env::var("MIGRATIONS_DIR").unwrap_or_else(|_| "migrations".to_string());
    let migrations_path = Path::new(&migrations_dir);

    if !migrations_path.exists() {
        println!("[migrations] directory '{}' not found, skipping.", migrations_path.display());
        return Ok(());
    }

    let pool = PgPoolOptions::new().max_connections(5).connect(&database_url).await?;

    // Ensure table exists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\n           version TEXT PRIMARY KEY,\n           applied_at TIMESTAMPTZ NOT NULL\n         )"
    ).execute(&pool).await?;

    // Collect sql files
    let mut files: Vec<PathBuf> = fs::read_dir(migrations_path)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map(|e| e == "sql").unwrap_or(false))
        .collect();
    files.sort();

    let mut applied_any = false;
    for file in files {
        let version = file.file_name().unwrap().to_string_lossy().to_string();
        let already: Option<(String,)> = sqlx::query_as("SELECT version FROM schema_migrations WHERE version = $1")
            .bind(&version)
            .fetch_optional(&pool)
            .await?;
        if already.is_some() { continue; }

        let sql_content = fs::read_to_string(&file)?;
        if sql_content.trim().is_empty() { continue; }
        println!("[migrations] Applying {version} ...");

        // Transactional apply
        let mut tx = pool.begin().await?;
        for statement in sql_content.split(';') { // naive splitter; keep migrations simple
            let stmt = statement.trim();
            if stmt.is_empty() { continue; }
            tx.execute(sqlx::query(stmt)).await?;
        }
        sqlx::query("INSERT INTO schema_migrations (version, applied_at) VALUES ($1, $2)")
            .bind(&version)
            .bind(Utc::now())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        println!("[migrations] Applied {version} ✔");
        applied_any = true;
    }

    if !applied_any { println!("[migrations] No pending migrations."); }
    Ok(())
}
