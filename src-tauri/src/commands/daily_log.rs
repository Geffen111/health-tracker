use crate::models::DailyLog;
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn get_daily_log(pool: State<'_, SqlitePool>, date: String) -> Result<Option<DailyLog>, String> {
    sqlx::query_as::<_, DailyLog>("SELECT * FROM daily_logs WHERE log_date = ?")
        .bind(&date)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_daily_log(
    pool: State<'_, SqlitePool>,
    log: DailyLog,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO daily_logs (log_date, day_name, fatigue_desc, fatigue_rating, headache_desc,
         headache_rating, headache_duration_hours, other_symptoms, my_sleep_rating, phone_sleep_rating,
         sleep_avg, sleep_time_head_on_pillow, sleep_actual_asleep, sleep_rem, sleep_deep, sleep_awake,
         steps, activity_calories, ave_resting_hr, ave_hr, rostered_hours, sick_leave_hours,
         office_hours, wfh_hours, alcohol_std_drinks, multivitamin, vitamin_c, add_meds,
         compression_socks, notes, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(log_date) DO UPDATE SET
         fatigue_desc=excluded.fatigue_desc, fatigue_rating=excluded.fatigue_rating,
         headache_desc=excluded.headache_desc, headache_rating=excluded.headache_rating,
         headache_duration_hours=excluded.headache_duration_hours,
         other_symptoms=excluded.other_symptoms, my_sleep_rating=excluded.my_sleep_rating,
         phone_sleep_rating=excluded.phone_sleep_rating, sleep_avg=excluded.sleep_avg,
         sleep_time_head_on_pillow=excluded.sleep_time_head_on_pillow,
         sleep_actual_asleep=excluded.sleep_actual_asleep, sleep_rem=excluded.sleep_rem,
         sleep_deep=excluded.sleep_deep, sleep_awake=excluded.sleep_awake,
         steps=excluded.steps, activity_calories=excluded.activity_calories,
         ave_resting_hr=excluded.ave_resting_hr, ave_hr=excluded.ave_hr,
         rostered_hours=excluded.rostered_hours, sick_leave_hours=excluded.sick_leave_hours,
         office_hours=excluded.office_hours, wfh_hours=excluded.wfh_hours,
         alcohol_std_drinks=excluded.alcohol_std_drinks, multivitamin=excluded.multivitamin,
         vitamin_c=excluded.vitamin_c, add_meds=excluded.add_meds,
         compression_socks=excluded.compression_socks, notes=excluded.notes,
         updated_at=datetime('now')"
    )
    .bind(&log.log_date).bind(&log.day_name).bind(&log.fatigue_desc).bind(log.fatigue_rating)
    .bind(&log.headache_desc).bind(log.headache_rating).bind(log.headache_duration_hours)
    .bind(&log.other_symptoms).bind(log.my_sleep_rating).bind(log.phone_sleep_rating)
    .bind(log.sleep_avg).bind(log.sleep_time_head_on_pillow).bind(log.sleep_actual_asleep)
    .bind(log.sleep_rem).bind(log.sleep_deep).bind(log.sleep_awake)
    .bind(log.steps).bind(log.activity_calories).bind(log.ave_resting_hr).bind(log.ave_hr)
    .bind(log.rostered_hours).bind(log.sick_leave_hours).bind(log.office_hours).bind(log.wfh_hours)
    .bind(log.alcohol_std_drinks).bind(log.multivitamin).bind(log.vitamin_c).bind(&log.add_meds)
    .bind(log.compression_socks).bind(&log.notes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_daily_logs(
    pool: State<'_, SqlitePool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<DailyLog>, String> {
    let limit = limit.unwrap_or(30);
    let offset = offset.unwrap_or(0);
    sqlx::query_as::<_, DailyLog>("SELECT * FROM daily_logs ORDER BY log_date DESC LIMIT ? OFFSET ?")
        .bind(limit).bind(offset)
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}