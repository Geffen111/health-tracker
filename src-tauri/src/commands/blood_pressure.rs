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

/// One reading, as measured — the input to the Cardio chart's calibration adjustment.
#[derive(Serialize, sqlx::FromRow)]
pub struct BpPoint {
    pub log_date: String,
    pub time_taken: Option<String>,
    pub systolic: i64,
    pub diastolic: i64,
    pub source: Option<String>,
}

/// Every reading, oldest first. The calibration adjustment needs the whole history:
/// each shift is measured against the readings either side of a calibration, however
/// far back it was.
#[tauri::command]
pub async fn list_bp_readings(pool: State<'_, SqlitePool>) -> Result<Vec<BpPoint>, String> {
    sqlx::query_as::<_, BpPoint>(
        "SELECT log_date, time_taken, systolic, diastolic, source FROM blood_pressure          WHERE systolic IS NOT NULL AND diastolic IS NOT NULL          ORDER BY log_date, COALESCE(time_taken, '00:00'), reading_num",
    )
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
        // Chronological, not by reading_num: the sync appends watch readings as it
        // finds them, so an early-morning synced reading can carry a higher
        // reading_num than a manual one taken that afternoon. Undated readings last.
        "SELECT * FROM blood_pressure WHERE log_date = ?
         ORDER BY COALESCE(time_taken, '99:99'), reading_num"
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
        "INSERT INTO blood_pressure (log_date, reading_num, time_taken, systolic, diastolic, pulse, notes, source)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(log_date, reading_num) DO UPDATE SET
         time_taken=excluded.time_taken, systolic=excluded.systolic,
         diastolic=excluded.diastolic, pulse=excluded.pulse,
         notes=excluded.notes, source=excluded.source"
    )
    .bind(&bp.log_date).bind(bp.reading_num).bind(&bp.time_taken)
    .bind(bp.systolic).bind(bp.diastolic).bind(bp.pulse)
    .bind(&bp.notes).bind(&bp.source)
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