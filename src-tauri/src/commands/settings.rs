// App settings persisted as JSON next to the database (OneDrive-synced via
// get_data_dir), so the API key follows the user across machines like the DB does.
use crate::db::get_data_dir;
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
    Ok(read_settings()
        .get("openrouter_api_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string()))
}
