// App settings persisted as JSON next to the database (OneDrive-synced via
// get_data_dir), so the API key follows the user across machines like the DB does.
use crate::db::get_data_dir;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

fn settings_path() -> PathBuf {
    get_data_dir().join("settings.json")
}

fn read_settings() -> serde_json::Value {
    let path = settings_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

fn write_setting(key: &str, value: serde_json::Value) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create settings dir: {}", e))?;
    }
    let mut settings = read_settings();
    settings[key] = value;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn save_api_key(key: String) -> Result<(), String> {
    write_setting("openrouter_api_key", serde_json::json!(key))
}

#[tauri::command]
pub async fn get_api_key() -> Result<Option<String>, String> {
    Ok(setting_str("openrouter_api_key"))
}

// ── Generic helpers (used by csv_import) ──

pub fn setting_str(key: &str) -> Option<String> {
    read_settings()
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

pub fn setting_i64(key: &str) -> Option<i64> {
    read_settings().get(key).and_then(|v| v.as_i64())
}

pub fn put_setting(key: &str, value: serde_json::Value) -> Result<(), String> {
    write_setting(key, value)
}

// ── Health Sync (CSV import) settings ──

#[derive(Serialize)]
pub struct SyncSettings {
    pub csv_root: Option<String>,
    pub auto_import: bool,
    pub last_sync: Option<String>,
}

#[tauri::command]
pub async fn get_sync_settings() -> Result<SyncSettings, String> {
    Ok(SyncSettings {
        csv_root: setting_str("csv_root"),
        // Default ON: the user opted into on-launch auto-import.
        auto_import: read_settings()
            .get("auto_import")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        last_sync: setting_str("last_sync"),
    })
}

#[tauri::command]
pub async fn save_sync_settings(csv_root: Option<String>, auto_import: bool) -> Result<(), String> {
    put_setting("csv_root", serde_json::json!(csv_root))?;
    put_setting("auto_import", serde_json::json!(auto_import))?;
    Ok(())
}
