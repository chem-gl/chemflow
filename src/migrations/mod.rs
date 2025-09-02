use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, Executor};
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Runs pending SQL migrations located in the `migrations/` directory (or
/// MIGRATIONS_DIR env var). Each `*.sql` file is treated as one migration and
/// applied only once. Applied migrations are tracked in the `schema_migrations`
/// table.
pub async fn run_migrations() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL")?;
    let migrations_dir = env::var("MIGRATIONS_DIR").unwrap_or_else(|_| "migrations".to_string());
    let migrations_path = Path::new(&migrations_dir);

    if !migrations_path.exists() {
        println!("[migrations] directory '{}' not found, skipping.", migrations_path.display());
        return Ok(());
    }

    // Intentar conectar; si la BD no existe (3D000) intentar crearla (replicar
    // lógica simplificada de config::ensure_database_exists)
    let pool = match PgPoolOptions::new().max_connections(5).connect(&database_url).await {
        Ok(p) => p,
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("3D000") => {
            eprintln!("[migrations] Target DB missing. Attempting create...");
            // Crear DB
            if let Some(pos) = database_url.rfind('/') {
                let (base, tail) = database_url.split_at(pos);
                let db_part = &tail[1..];
                let db_only = db_part.split('?').next().unwrap_or(db_part);
                if !db_only.is_empty() {
                    let admin_url = if base.ends_with("/postgres") || db_only == "postgres" { database_url.clone() } else { format!("{}/postgres", base) };
                    if let Ok(admin_pool) = PgPoolOptions::new().max_connections(1).connect(&admin_url).await {
                        let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pg_database WHERE datname = $1").bind(db_only).fetch_one(&admin_pool).await?;
                        if exists.0 == 0 {
                            if db_only.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                                let create_stmt = format!("CREATE DATABASE \"{}\"", db_only.replace('"', ""));
                                admin_pool.execute(sqlx::query(&create_stmt)).await?;
                                eprintln!("[migrations] Database '{}' created", db_only);
                            } else {
                                eprintln!("[migrations] Unsafe db name, abort auto-create: {}", db_only);
                            }
                        }
                    }
                }
            }
            // Reintentar conexión
            PgPoolOptions::new().max_connections(5).connect(&database_url).await?
        }
        Err(e) => return Err(Box::new(e)),
    };

    // Ensure table exists
    sqlx::query("CREATE TABLE IF NOT EXISTS schema_migrations (\n           version TEXT PRIMARY KEY,\n           applied_at TIMESTAMPTZ NOT NULL\n         )").execute(&pool)
                                                                                                                                                               .await?;

    // Collect sql files
    let mut files: Vec<PathBuf> = fs::read_dir(migrations_path)?.filter_map(|e| e.ok())
                                                                .map(|e| e.path())
                                                                .filter(|p| p.is_file() && p.extension().map(|e| e == "sql").unwrap_or(false))
                                                                .collect();
    files.sort();

    let mut applied_any = false;
    for file in files {
        let version = file.file_name().unwrap().to_string_lossy().to_string();
        let already: Option<(String,)> = sqlx::query_as("SELECT version FROM schema_migrations WHERE version = $1").bind(&version).fetch_optional(&pool).await?;
        if already.is_some() {
            continue;
        }

        let sql_content = fs::read_to_string(&file)?;
        if sql_content.trim().is_empty() {
            continue;
        }
        println!("[migrations] Applying {version} ...");

        // Transactional apply
        let mut tx = pool.begin().await?;
        for statement in sql_content.split(';') {
            // naive splitter; keep migrations simple
            let stmt = statement.trim();
            if stmt.is_empty() {
                continue;
            }
            tx.execute(sqlx::query(stmt)).await?;
        }
        sqlx::query("INSERT INTO schema_migrations (version, applied_at) VALUES ($1, $2)").bind(&version).bind(Utc::now()).execute(&mut *tx).await?;
        tx.commit().await?;
        println!("[migrations] Applied {version} ✔");
        applied_any = true;
    }

    if !applied_any {
        println!("[migrations] No pending migrations.");
    }
    Ok(())
}
