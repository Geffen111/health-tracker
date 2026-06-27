use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;

pub async fn init_db() -> SqlitePool {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create db directory");
    }

    let connect_opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("Failed to connect to database");

    // Run embedded migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    println!("Database initialized at: {:?}", db_path);
    pool
}

/// The directory holding the database (and other app data such as exports).
/// Windows OneDrive path when available (auto-syncs across devices); otherwise
/// the standard local data directory.
pub fn get_data_dir() -> PathBuf {
    if let Ok(onedrive) = std::env::var("OneDrive") {
        PathBuf::from(onedrive).join("Apps").join("HealthTracker")
    } else {
        // Fallback: standard local data directory
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("health-tracker")
    }
}

fn get_db_path() -> PathBuf {
    get_data_dir().join("health.db")
}