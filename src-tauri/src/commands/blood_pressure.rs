use crate::models::BloodPressure;
use sqlx::SqlitePool;
use tauri::State;

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