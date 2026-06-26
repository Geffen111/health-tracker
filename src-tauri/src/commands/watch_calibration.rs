use crate::models::WatchCalibration;
use sqlx::SqlitePool;
use tauri::State;

/// Log a watch/HR-sensor calibration. Defaults to today / now when not supplied.
#[tauri::command]
pub async fn log_watch_calibration(
    pool: State<'_, SqlitePool>,
    cal_date: Option<String>,
    cal_time: Option<String>,
    notes: Option<String>,
) -> Result<i64, String> {
    let now = chrono::Local::now();
    let date = cal_date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    let time = cal_time.or_else(|| Some(now.format("%H:%M").to_string()));
    sqlx::query(
        "INSERT INTO watch_calibration (cal_date, cal_time, notes) VALUES (?, ?, ?)",
    )
    .bind(&date).bind(&time).bind(&notes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_watch_calibrations(
    pool: State<'_, SqlitePool>,
    limit: Option<i64>,
) -> Result<Vec<WatchCalibration>, String> {
    sqlx::query_as::<_, WatchCalibration>(
        "SELECT * FROM watch_calibration ORDER BY cal_date DESC, cal_time DESC LIMIT ?",
    )
    .bind(limit.unwrap_or(50))
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_watch_calibration(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM watch_calibration WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Whole days since the most recent calibration; None when none logged yet.
/// The UI uses this to flag the ~30-day recalibration.
#[tauri::command]
pub async fn days_since_calibration(pool: State<'_, SqlitePool>) -> Result<Option<i64>, String> {
    let last: Option<(String,)> =
        sqlx::query_as("SELECT cal_date FROM watch_calibration ORDER BY cal_date DESC LIMIT 1")
            .fetch_optional(&*pool)
            .await
            .map_err(|e| e.to_string())?;
    match last {
        Some((date_str,)) => {
            let parsed = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| e.to_string())?;
            let today = chrono::Local::now().date_naive();
            Ok(Some((today - parsed).num_days()))
        }
        None => Ok(None),
    }
}
