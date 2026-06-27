use crate::models::{ActivityCategory, ActivityType, ActivityEntry};
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn list_activity_categories(pool: State<'_, SqlitePool>) -> Result<Vec<ActivityCategory>, String> {
    sqlx::query_as::<_, ActivityCategory>("SELECT * FROM activity_categories ORDER BY name")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_activity_types(
    pool: State<'_, SqlitePool>,
    category_id: Option<i64>,
) -> Result<Vec<ActivityType>, String> {
    if let Some(cat_id) = category_id {
        sqlx::query_as::<_, ActivityType>(
            "SELECT * FROM activity_types WHERE category_id = ? ORDER BY name"
        )
        .bind(cat_id)
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
    } else {
        sqlx::query_as::<_, ActivityType>("SELECT * FROM activity_types ORDER BY name")
            .fetch_all(&*pool)
            .await
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn get_activities_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<ActivityEntry>, String> {
    sqlx::query_as::<_, ActivityEntry>(
        "SELECT * FROM activity_log WHERE log_date = ? ORDER BY duration_hours DESC"
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_activity_entry(pool: State<'_, SqlitePool>, entry: ActivityEntry) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO activity_log (log_date, activity_type_id, duration_hours, energy_cost, notes)
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&entry.log_date).bind(entry.activity_type_id).bind(entry.duration_hours)
    .bind(&entry.energy_cost).bind(&entry.notes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Upsert a single day's duration for one activity type. Energy cost is taken
/// from the activity type's default (auto-detected — the UI no longer asks).
/// A duration of 0 (or less) clears any existing entry for that day+type.
#[tauri::command]
pub async fn set_activity_duration(
    pool: State<'_, SqlitePool>,
    log_date: String,
    activity_type_id: i64,
    duration_hours: f64,
) -> Result<(), String> {
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM activity_log WHERE log_date = ? AND activity_type_id = ?")
        .bind(&log_date).bind(activity_type_id)
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    if duration_hours <= 0.0 {
        if let Some((id,)) = existing {
            sqlx::query("DELETE FROM activity_log WHERE id = ?")
                .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let energy: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT default_energy_cost FROM activity_types WHERE id = ?")
        .bind(activity_type_id)
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;
    let energy_cost = energy.and_then(|r| r.0).unwrap_or_else(|| "Medium".to_string());

    match existing {
        Some((id,)) => {
            sqlx::query("UPDATE activity_log SET duration_hours = ?, energy_cost = ? WHERE id = ?")
                .bind(duration_hours).bind(&energy_cost).bind(id)
                .execute(&*pool).await.map_err(|e| e.to_string())?;
        }
        None => {
            sqlx::query(
                "INSERT INTO activity_log (log_date, activity_type_id, duration_hours, energy_cost)
                 VALUES (?, ?, ?, ?)")
                .bind(&log_date).bind(activity_type_id).bind(duration_hours).bind(&energy_cost)
                .execute(&*pool).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_activity_entry(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM activity_log WHERE id = ?")
        .bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}