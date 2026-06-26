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
         steps, activity_calories, ave_resting_hr, ave_hr, hr_min, hr_max, rostered_hours, sick_leave_hours,
         office_hours, wfh_hours, alcohol_std_drinks, multivitamin, vitamin_c, add_meds,
         compression_socks, notes, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
         -- COALESCE so a page that only manages some fields (e.g. the Work page sends
         -- null for sleep/steps/meds) does NOT wipe values another page already saved
         -- for that day. A null in the incoming row means \"leave this field as-is\".
         ON CONFLICT(log_date) DO UPDATE SET
         day_name=COALESCE(excluded.day_name, daily_logs.day_name),
         fatigue_desc=COALESCE(excluded.fatigue_desc, daily_logs.fatigue_desc),
         fatigue_rating=COALESCE(excluded.fatigue_rating, daily_logs.fatigue_rating),
         headache_desc=COALESCE(excluded.headache_desc, daily_logs.headache_desc),
         headache_rating=COALESCE(excluded.headache_rating, daily_logs.headache_rating),
         headache_duration_hours=COALESCE(excluded.headache_duration_hours, daily_logs.headache_duration_hours),
         other_symptoms=COALESCE(excluded.other_symptoms, daily_logs.other_symptoms),
         my_sleep_rating=COALESCE(excluded.my_sleep_rating, daily_logs.my_sleep_rating),
         phone_sleep_rating=COALESCE(excluded.phone_sleep_rating, daily_logs.phone_sleep_rating),
         sleep_avg=COALESCE(excluded.sleep_avg, daily_logs.sleep_avg),
         sleep_time_head_on_pillow=COALESCE(excluded.sleep_time_head_on_pillow, daily_logs.sleep_time_head_on_pillow),
         sleep_actual_asleep=COALESCE(excluded.sleep_actual_asleep, daily_logs.sleep_actual_asleep),
         sleep_rem=COALESCE(excluded.sleep_rem, daily_logs.sleep_rem),
         sleep_deep=COALESCE(excluded.sleep_deep, daily_logs.sleep_deep),
         sleep_awake=COALESCE(excluded.sleep_awake, daily_logs.sleep_awake),
         steps=COALESCE(excluded.steps, daily_logs.steps),
         activity_calories=COALESCE(excluded.activity_calories, daily_logs.activity_calories),
         ave_resting_hr=COALESCE(excluded.ave_resting_hr, daily_logs.ave_resting_hr),
         ave_hr=COALESCE(excluded.ave_hr, daily_logs.ave_hr),
         hr_min=COALESCE(excluded.hr_min, daily_logs.hr_min),
         hr_max=COALESCE(excluded.hr_max, daily_logs.hr_max),
         rostered_hours=COALESCE(excluded.rostered_hours, daily_logs.rostered_hours),
         sick_leave_hours=COALESCE(excluded.sick_leave_hours, daily_logs.sick_leave_hours),
         office_hours=COALESCE(excluded.office_hours, daily_logs.office_hours),
         wfh_hours=COALESCE(excluded.wfh_hours, daily_logs.wfh_hours),
         alcohol_std_drinks=COALESCE(excluded.alcohol_std_drinks, daily_logs.alcohol_std_drinks),
         multivitamin=COALESCE(excluded.multivitamin, daily_logs.multivitamin),
         vitamin_c=COALESCE(excluded.vitamin_c, daily_logs.vitamin_c),
         add_meds=COALESCE(excluded.add_meds, daily_logs.add_meds),
         compression_socks=COALESCE(excluded.compression_socks, daily_logs.compression_socks),
         notes=COALESCE(excluded.notes, daily_logs.notes),
         updated_at=datetime('now')"
    )
    .bind(&log.log_date).bind(&log.day_name).bind(&log.fatigue_desc).bind(log.fatigue_rating)
    .bind(&log.headache_desc).bind(log.headache_rating).bind(log.headache_duration_hours)
    .bind(&log.other_symptoms).bind(log.my_sleep_rating).bind(log.phone_sleep_rating)
    .bind(log.sleep_avg).bind(log.sleep_time_head_on_pillow).bind(log.sleep_actual_asleep)
    .bind(log.sleep_rem).bind(log.sleep_deep).bind(log.sleep_awake)
    .bind(log.steps).bind(log.activity_calories).bind(log.ave_resting_hr).bind(log.ave_hr)
    .bind(log.hr_min).bind(log.hr_max)
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