//! Which parts of the app are switched on.
//!
//! Stored as `features` in the synced settings.json, so the choice follows the person to
//! each computer. Core modules default on; the "Advanced" set — the watch sync, the
//! Records vault and every AI feature — defaults off for a new user, and each AI feature
//! has its own switch under the master `ai` one.
//!
//! An install from before this existed (data already logged, no `features` saved) gets
//! everything switched on, so nothing disappears on update.

use crate::commands::settings;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
use tauri::State;

const KEY: &str = "features";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    /// The first-run "what do you want to track?" step has been done.
    pub onboarded: bool,
    // Modules (pages in the sidebar).
    pub sleep: bool,
    pub activity: bool,
    pub cardio: bool,
    pub medication: bool,
    pub food: bool,
    pub work: bool,
    pub pacing: bool,
    // Advanced.
    /// Health Sync CSV import (watch steps, HR, sleep, BP) and watch calibration.
    pub health_sync: bool,
    /// The Records page, reading a folder of Markdown notes.
    pub vault: bool,
    /// Master switch; each feature below also needs its own.
    pub ai: bool,
    pub ai_ask: bool,
    pub ai_weekly: bool,
    pub ai_food_photo: bool,
    pub ai_food_tags: bool,
    pub ai_records: bool,
}

impl Default for Features {
    fn default() -> Self {
        Features {
            onboarded: false,
            sleep: true,
            activity: true,
            cardio: true,
            medication: true,
            food: true,
            work: true,
            pacing: true,
            health_sync: false,
            vault: false,
            ai: false,
            ai_ask: false,
            ai_weekly: false,
            ai_food_photo: false,
            ai_food_tags: false,
            ai_records: false,
        }
    }
}

impl Features {
    fn everything() -> Self {
        Features {
            onboarded: true,
            health_sync: true,
            vault: true,
            ai: true,
            ai_ask: true,
            ai_weekly: true,
            ai_food_photo: true,
            ai_food_tags: true,
            ai_records: true,
            ..Features::default()
        }
    }
}

/// The saved switches, or the new-user defaults when none are saved. For backend checks.
pub fn current() -> Features {
    settings::get_setting(KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// True when AI is on and so is the feature `pick` selects.
pub fn ai(pick: fn(&Features) -> bool) -> bool {
    let f = current();
    f.ai && pick(&f)
}

/// Where the Records vault was before it had to be chosen. Only adopted for an install
/// that already used it.
const LEGACY_VAULT: &str = "C:\\Users\\gavin\\OneDrive\\Obsidian\\Health-Records";

#[tauri::command]
pub async fn get_features(pool: State<'_, SqlitePool>) -> Result<Features, String> {
    if let Some(v) = settings::get_setting(KEY) {
        return serde_json::from_value(v).map_err(|e| format!("Bad saved features: {}", e));
    }
    let logged: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_logs")
        .fetch_one(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    let f = if logged.0 > 0 {
        // Already in use: keep everything it had. The vault used to default to a fixed
        // path; record it so it keeps working now that there's no default.
        let mut f = Features::everything();
        if settings::setting_str("vault_root").is_none() && Path::new(LEGACY_VAULT).is_dir() {
            settings::put_setting("vault_root", serde_json::json!(LEGACY_VAULT))?;
        }
        f.vault = settings::setting_str("vault_root").is_some();
        f.health_sync = settings::setting_str("csv_root").is_some() || settings::setting_str("last_sync").is_some();
        f
    } else {
        Features::default()
    };
    settings::put_setting(KEY, serde_json::to_value(&f).unwrap())?;
    Ok(f)
}

#[tauri::command]
pub async fn save_features(features: Features) -> Result<(), String> {
    settings::put_setting(KEY, serde_json::to_value(&features).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_users_start_with_modules_on_and_advanced_off() {
        let f = Features::default();
        assert!(f.food && f.sleep && f.cardio && !f.onboarded);
        assert!(!f.ai && !f.ai_ask && !f.health_sync && !f.vault);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let f: Features = serde_json::from_value(serde_json::json!({ "onboarded": true, "ai": true })).unwrap();
        assert!(f.onboarded && f.ai && f.food && !f.ai_weekly);
    }
}
