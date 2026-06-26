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

#[tauri::command]
pub async fn delete_activity_entry(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM activity_log WHERE id = ?")
        .bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}