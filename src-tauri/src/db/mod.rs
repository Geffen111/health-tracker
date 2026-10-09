//! Where the data lives, and opening it.
//!
//! The data folder (database, settings.json, exports) is chosen per machine and recorded
//! in `location.json` in this PC's local app data — never in the data folder itself, since
//! each computer may reach a synced folder by a different path. With no choice recorded,
//! an existing `%OneDrive%\Apps\HealthTracker\health.db` (the original location) is adopted
//! automatically; otherwise the app asks on first launch (see commands/data_location.rs).
//!
//! A synced SQLite file must only be open on one computer at a time, or the sync service
//! makes conflicting copies (`health-<PC>.db`). `health.lock` in the data folder records
//! which computer has it open; see `lock`.

pub mod lock;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

pub const DB_FILE: &str = "health.db";

/// This PC's own app data. Holds what is true of this machine only: the data-folder
/// choice, the API key, the CSV import's per-file state.
pub fn local_dir() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("health-tracker")
}

fn location_file() -> PathBuf {
    local_dir().join("location.json")
}

/// The folder chosen on this machine, if any.
pub fn configured_dir() -> Option<PathBuf> {
    let text = std::fs::read_to_string(location_file()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("data_dir")?.as_str().filter(|s| !s.is_empty()).map(PathBuf::from)
}

pub fn set_configured_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(local_dir()).map_err(|e| format!("Couldn't create {}: {}", local_dir().display(), e))?;
    let json = serde_json::json!({ "data_dir": dir.to_string_lossy() });
    std::fs::write(location_file(), serde_json::to_string_pretty(&json).unwrap())
        .map_err(|e| format!("Couldn't save the data folder choice: {}", e))
}

/// The original, pre-setup location: adopted without asking when it already holds data.
pub fn legacy_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var("OneDrive").ok()?).join("Apps").join("HealthTracker");
    dir.join(DB_FILE).exists().then_some(dir)
}

/// "On this computer only".
pub fn default_local_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("health-tracker")
}

/// The directory holding the database, settings.json and exports.
pub fn get_data_dir() -> PathBuf {
    configured_dir().unwrap_or_else(default_local_data_dir)
}

/// Resolve the folder for this launch: the recorded choice, else adopt the legacy
/// OneDrive folder, else None (first run — ask).
pub fn resolve_data_dir() -> Option<PathBuf> {
    if let Some(dir) = configured_dir() {
        return Some(dir);
    }
    let legacy = legacy_dir()?;
    // Best effort: if this can't be written we'll simply adopt it again next launch.
    let _ = set_configured_dir(&legacy);
    Some(legacy)
}

pub async fn open_db(dir: &Path) -> Result<SqlitePool, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {}: {}", dir.display(), e))?;
    let db_path = dir.join(DB_FILE);
    let connect_opts = SqliteConnectOptions::new().filename(&db_path).create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .map_err(|e| format!("Couldn't open {}: {}", db_path.display(), e))?;
    // Embedded, never read from disk at runtime.
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("Couldn't update the database: {}", e))?;
    println!("Database opened at: {:?}", db_path);
    Ok(pool)
}
