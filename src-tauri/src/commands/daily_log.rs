use crate::models::{DailyLog, SleepBreakdown};
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

/// Columns `patch_daily_log` may write. The map keys are interpolated into the
/// SQL, so this list is also the injection guard — reject anything not on it.
const PATCHABLE: &[&str] = &[
    "day_name", "fatigue_desc", "fatigue_rating", "headache_desc", "headache_rating",
    "headache_duration_hours", "other_symptoms", "my_sleep_rating", "phone_sleep_rating",
    "sleep_avg", "steps", "activity_calories", "alcohol_std_drinks", "multivitamin",
    "vitamin_c", "add_meds", "compression_socks", "notes",
];

/// Build the upsert for a set of columns, rejecting any that isn't in
/// `PATCHABLE`. Column names reach the SQL by interpolation (they can't be bound),
/// so the whitelist check has to happen here and nowhere else.
fn build_patch_sql(cols: &[&str]) -> Result<String, String> {
    if let Some(bad) = cols.iter().find(|c| !PATCHABLE.contains(c)) {
        return Err(format!("field not patchable: {}", bad));
    }
    let placeholders = vec!["?"; cols.len()].join(", ");
    let sets = cols
        .iter()
        .map(|c| format!("{c}=excluded.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "INSERT INTO daily_logs (log_date, {}, updated_at) VALUES (?, {}, datetime('now'))
         ON CONFLICT(log_date) DO UPDATE SET {}, updated_at=datetime('now')",
        cols.join(", "),
        placeholders,
        sets
    ))
}

/// Write exactly the fields a page owns, and no others.
///
/// `upsert_daily_log` COALESCEs, so a null means "leave as-is" — which makes it
/// impossible to *clear* a value. That was tolerable behind a Save button; with
/// the Daily Log page autosaving it isn't, because emptying a field would appear
/// to work and then silently revert on reload. Here the JSON map distinguishes
/// the two cases properly: a key that is absent is left alone, a key whose value
/// is null is cleared.
#[tauri::command]
pub async fn patch_daily_log(
    pool: State<'_, SqlitePool>,
    date: String,
    fields: serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if fields.is_empty() {
        return Ok(());
    }
    let cols: Vec<&str> = fields.keys().map(|k| k.as_str()).collect();
    let sql = build_patch_sql(&cols)?;

    let mut q = sqlx::query(&sql).bind(&date);
    for col in &cols {
        // SQLite column affinity converts a bound f64 back to INTEGER where the
        // column calls for it (e.g. steps), so numbers need no per-column typing.
        q = match &fields[*col] {
            serde_json::Value::Null => q.bind(None::<f64>),
            serde_json::Value::Bool(b) => q.bind(*b),
            serde_json::Value::Number(n) => q.bind(n.as_f64()),
            serde_json::Value::String(s) => q.bind(s.clone()),
            other => return Err(format!("unsupported value for {}: {}", col, other)),
        };
    }
    q.execute(&*pool).await.map(|_| ()).map_err(|e| e.to_string())
}

/// Manual sleep entry from the Sleep page. Unlike `upsert_daily_log` this writes
/// the five sleep columns *explicitly* rather than with COALESCE: a blank field
/// means "clear it", which is what you want when the watch sync recorded a night
/// wrongly (e.g. merged an afternoon nap into it). Every other column on the row
/// is left untouched.
///
/// Saving marks the day `sleep_source = 'manual'`, which makes the entry sticky —
/// a later CSV sync won't overwrite it. Saving with every field blank clears the
/// marker instead, handing the night back to the sync.
#[tauri::command]
pub async fn upsert_sleep_breakdown(
    pool: State<'_, SqlitePool>,
    breakdown: SleepBreakdown,
) -> Result<(), String> {
    let has_value = breakdown.sleep_time_head_on_pillow.is_some()
        || breakdown.sleep_actual_asleep.is_some()
        || breakdown.sleep_rem.is_some()
        || breakdown.sleep_deep.is_some()
        || breakdown.sleep_awake.is_some();
    let source = if has_value { Some("manual") } else { None };

    sqlx::query(
        "INSERT INTO daily_logs (log_date, sleep_time_head_on_pillow, sleep_actual_asleep,
         sleep_rem, sleep_deep, sleep_awake, sleep_source, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(log_date) DO UPDATE SET
         sleep_time_head_on_pillow=excluded.sleep_time_head_on_pillow,
         sleep_actual_asleep=excluded.sleep_actual_asleep,
         sleep_rem=excluded.sleep_rem,
         sleep_deep=excluded.sleep_deep,
         sleep_awake=excluded.sleep_awake,
         sleep_source=excluded.sleep_source,
         updated_at=datetime('now')"
    )
    .bind(&breakdown.log_date)
    .bind(breakdown.sleep_time_head_on_pillow)
    .bind(breakdown.sleep_actual_asleep)
    .bind(breakdown.sleep_rem)
    .bind(breakdown.sleep_deep)
    .bind(breakdown.sleep_awake)
    .bind(source)
    .execute(&*pool)
    .await
    .map(|_| ())
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_sql_sets_only_the_given_columns() {
        let sql = build_patch_sql(&["fatigue_rating", "notes"]).unwrap();
        assert!(sql.contains("INSERT INTO daily_logs (log_date, fatigue_rating, notes, updated_at)"));
        assert!(sql.contains("VALUES (?, ?, ?, datetime('now'))"));
        assert!(sql.contains("fatigue_rating=excluded.fatigue_rating"));
        assert!(sql.contains("notes=excluded.notes"));
        // No COALESCE: a bound null must clear the column, not preserve it.
        assert!(!sql.contains("COALESCE"));
        // Columns the caller didn't name are untouched.
        assert!(!sql.contains("sleep_rem"));
    }

    #[test]
    fn patch_sql_rejects_unknown_columns() {
        // Column names are interpolated, so anything off the whitelist — typo or
        // injection attempt — must not reach the SQL.
        assert!(build_patch_sql(&["notes", "sleep_rem"]).is_err());
        assert!(build_patch_sql(&["notes = 1, id"]).is_err());
    }
}
