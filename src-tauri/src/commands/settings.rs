// Non-secret app settings persist as JSON next to the database (OneDrive-synced
// via get_data_dir), so preferences like the CSV root, sync flags and the chosen
// AI model follow the user across machines like the DB does.
//
// The OpenRouter API key is a plaintext secret and is deliberately kept OUT of
// the OneDrive-synced file — it lives in a machine-local file (%LOCALAPPDATA%)
// so it is never uploaded to the cloud. (Same decision as the Family Finance app.)
use crate::db::get_data_dir;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

const API_KEY_SETTING: &str = "openrouter_api_key";
const MODEL_SETTING: &str = "openrouter_model";

fn settings_path() -> PathBuf {
    get_data_dir().join("settings.json")
}

// Machine-local, NOT cloud-synced. Holds secrets only.
fn secrets_path() -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| get_data_dir());
    base.join("health-tracker").join("secrets.json")
}

fn read_secrets() -> serde_json::Value {
    let path = secrets_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

fn write_secret(key: &str, value: serde_json::Value) -> Result<(), String> {
    let path = secrets_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create secrets dir: {}", e))?;
    }
    let mut secrets = read_secrets();
    secrets[key] = value;
    let json = serde_json::to_string_pretty(&secrets)
        .map_err(|e| format!("Failed to serialize secrets: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write secrets: {}", e))?;
    Ok(())
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
    write_secret(API_KEY_SETTING, serde_json::json!(key))?;
    // If an older build had stored the key in the synced settings.json, drop it
    // there now so the secret stops being uploaded to OneDrive.
    clear_synced_api_key();
    Ok(())
}

#[tauri::command]
pub async fn get_api_key() -> Result<Option<String>, String> {
    Ok(api_key())
}

/// The OpenRouter key, preferring the local secrets file. Falls back to (and
/// migrates) a key left in the old OneDrive-synced settings.json by earlier builds.
pub fn api_key() -> Option<String> {
    if let Some(k) = read_secrets()
        .get(API_KEY_SETTING)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
    {
        return Some(k);
    }
    // One-time migration: lift a legacy synced key into the local secrets file.
    if let Some(legacy) = setting_str(API_KEY_SETTING) {
        let _ = write_secret(API_KEY_SETTING, serde_json::json!(legacy));
        clear_synced_api_key();
        return Some(legacy);
    }
    None
}

fn clear_synced_api_key() {
    let mut settings = read_settings();
    if settings.get(API_KEY_SETTING).is_some() {
        if let Some(obj) = settings.as_object_mut() {
            obj.remove(API_KEY_SETTING);
        }
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = fs::write(settings_path(), json);
        }
    }
}

// ── AI model selection (non-secret; synced) ──

/// The configured OpenRouter model id, or the default if none has been chosen.
pub fn model() -> String {
    setting_str(MODEL_SETTING).unwrap_or_else(|| crate::commands::ai::MODEL.to_string())
}

#[tauri::command]
pub async fn get_ai_model() -> Result<String, String> {
    Ok(model())
}

#[tauri::command]
pub async fn save_ai_model(model: String) -> Result<(), String> {
    let m = model.trim();
    let value = if m.is_empty() {
        crate::commands::ai::MODEL.to_string()
    } else {
        m.to_string()
    };
    write_setting(MODEL_SETTING, serde_json::json!(value))
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

// ── Health Records vault (read-only Obsidian browser) ──

#[derive(Serialize)]
pub struct VaultSettings {
    /// Folder of the Obsidian "Health Records" vault, or None to use the default.
    pub vault_root: Option<String>,
}

#[tauri::command]
pub async fn get_vault_settings() -> Result<VaultSettings, String> {
    Ok(VaultSettings { vault_root: setting_str("vault_root") })
}

#[tauri::command]
pub async fn save_vault_settings(vault_root: Option<String>) -> Result<(), String> {
    let cleaned = vault_root.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    put_setting("vault_root", serde_json::json!(cleaned))?;
    Ok(())
}

// ── App preferences (work defaults, activity defaults) ──

#[derive(Serialize)]
pub struct AppPrefs {
    /// Default hours for a full work day (rostered/office prefill).
    pub work_hours: f64,
    /// Weekday indices that are work days (0=Sun … 6=Sat). Default Mon–Fri.
    pub work_days: Vec<i64>,
    /// Activity names always shown ready for time entry. Default Phone, Walking.
    pub activity_defaults: Vec<String>,
}

fn default_work_days() -> Vec<i64> {
    vec![1, 2, 3, 4, 5]
}
fn default_activity_defaults() -> Vec<String> {
    vec!["Phone".to_string(), "Walking".to_string()]
}

#[tauri::command]
pub async fn get_app_prefs() -> Result<AppPrefs, String> {
    let s = read_settings();
    let work_hours = s.get("work_hours").and_then(|v| v.as_f64()).unwrap_or(7.5);
    let work_days = s
        .get("work_days")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_work_days);
    let activity_defaults = s
        .get("activity_defaults")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(default_activity_defaults);
    Ok(AppPrefs { work_hours, work_days, activity_defaults })
}

#[tauri::command]
pub async fn save_app_prefs(
    work_hours: f64,
    work_days: Vec<i64>,
    activity_defaults: Vec<String>,
) -> Result<(), String> {
    put_setting("work_hours", serde_json::json!(work_hours))?;
    put_setting("work_days", serde_json::json!(work_days))?;
    put_setting("activity_defaults", serde_json::json!(activity_defaults))?;
    Ok(())
}
