use crate::models::BloodPressure;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Serialize, sqlx::FromRow)]
pub struct BpDailyAvg {
    pub log_date: String,
    pub avg_systolic: Option<f64>,
    pub avg_diastolic: Option<f64>,
}

/// Daily-averaged blood pressure over the last `days` days, oldest first —
/// feeds the Cardio history chart.
#[tauri::command]
pub async fn get_bp_history(
    pool: State<'_, SqlitePool>,
    days: i64,
) -> Result<Vec<BpDailyAvg>, String> {
    sqlx::query_as::<_, BpDailyAvg>(
        "SELECT log_date, \
                AVG(systolic) AS avg_systolic, \
                AVG(diastolic) AS avg_diastolic \
         FROM blood_pressure \
         WHERE systolic IS NOT NULL AND diastolic IS NOT NULL \
           AND log_date >= date('now', ?) \
         GROUP BY log_date \
         ORDER BY log_date",
    )
    .bind(format!("-{} days", days))
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_bp_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<BloodPressure>, String> {
    sqlx::query_as::<_, BloodPressure>(
        "SELECT * FROM blood_pressure WHERE log_date = ? ORDER BY reading_num"
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_bp(
    pool: State<'_, SqlitePool>,
    bp: BloodPressure,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO blood_pressure (log_date, reading_num, time_taken, systolic, diastolic, notes)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(log_date, reading_num) DO UPDATE SET
         time_taken=excluded.time_taken, systolic=excluded.systolic,
         diastolic=excluded.diastolic, notes=excluded.notes"
    )
    .bind(&bp.log_date).bind(bp.reading_num).bind(&bp.time_taken)
    .bind(bp.systolic).bind(bp.diastolic).bind(&bp.notes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Permanently remove a single BP reading (systolic/diastolic are NOT NULL, so a
/// soft-delete by nulling them isn't possible — this deletes the row outright).
#[tauri::command]
pub async fn delete_bp(
    pool: State<'_, SqlitePool>,
    log_date: String,
    reading_num: i64,
) -> Result<(), String> {
    sqlx::query("DELETE FROM blood_pressure WHERE log_date = ? AND reading_num = ?")
        .bind(&log_date).bind(reading_num)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}