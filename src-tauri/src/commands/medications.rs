use crate::models::{Medication, MedicationDose, MedicationHistoryEntry, MedicationScheduleItem};
use chrono::Local;
use sqlx::SqlitePool;
use tauri::State;

/// Append a row to medication_history. Best-effort: never fails the caller.
async fn record_history(
    pool: &SqlitePool,
    med_id: i64,
    med_name: &str,
    event_type: &str,
    detail: &str,
    old_value: Option<String>,
    new_value: Option<String>,
) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let _ = sqlx::query(
        "INSERT INTO medication_history
         (medication_id, medication_name, event_type, event_date, detail, old_value, new_value)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(med_id).bind(med_name).bind(event_type).bind(&today)
    .bind(detail).bind(old_value).bind(new_value)
    .execute(pool)
    .await;
}

#[tauri::command]
pub async fn list_medications(pool: State<'_, SqlitePool>) -> Result<Vec<Medication>, String> {
    sqlx::query_as::<_, Medication>("SELECT * FROM medications ORDER BY active DESC, name")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_medication(
    pool: State<'_, SqlitePool>,
    name: String,
    short_code: Option<String>,
    default_dose: Option<f64>,
    dose_unit: Option<String>,
    category: Option<String>,
    default_time: Option<String>,
    med_type: Option<String>,
) -> Result<Medication, String> {
    let med = sqlx::query_as::<_, Medication>(
        "INSERT INTO medications (name, short_code, default_dose, dose_unit, category, default_time, med_type)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&name).bind(&short_code).bind(default_dose).bind(&dose_unit).bind(&category)
    .bind(&default_time).bind(med_type.as_deref().unwrap_or("regular"))
    .fetch_one(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    record_history(&pool, med.id, &med.name, "started", &format!("Started {}", med.name), None, None).await;
    Ok(med)
}

/// Update a medication. Dose changes and active/ceased transitions are recorded in
/// medication_history, and a human-readable banner message is returned for any such
/// change so the UI can surface it (None when nothing notable changed).
#[tauri::command]
pub async fn update_medication(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: Option<String>,
    short_code: Option<String>,
    default_dose: Option<f64>,
    dose_unit: Option<String>,
    category: Option<String>,
    active: Option<bool>,
    default_time: Option<String>,
    med_type: Option<String>,
    // An omitted `default_dose` means "leave unchanged" (so callers like the
    // cease/restart toggle don't wipe it). To actually clear a saved dose, the
    // edit form sends `clear_dose: true`.
    clear_dose: Option<bool>,
) -> Result<Option<String>, String> {
    // Snapshot the current state so we can detect notable changes.
    let before = sqlx::query_as::<_, Medication>("SELECT * FROM medications WHERE id = ?")
        .bind(id)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Medication {} not found", id))?;
    let med_name = name.clone().unwrap_or_else(|| before.name.clone());
    let unit = dose_unit.clone().or(before.dose_unit.clone()).unwrap_or_else(|| "mg".into());
    let mut banner: Option<String> = None;

    if let Some(val) = name {
        sqlx::query("UPDATE medications SET name = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = short_code {
        sqlx::query("UPDATE medications SET short_code = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if clear_dose == Some(true) {
        if before.default_dose.is_some() {
            let old = before.default_dose.map(|d| format!("{}", d));
            record_history(
                &pool, id, &med_name, "dose_changed",
                &format!("Default dose removed (was {} {})", old.clone().unwrap_or_else(|| "—".into()), unit),
                old, None,
            ).await;
            banner = Some(format!("Recorded: {} default dose cleared", med_name));
        }
        sqlx::query("UPDATE medications SET default_dose = NULL WHERE id = ?")
            .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    } else if let Some(val) = default_dose {
        if before.default_dose != Some(val) {
            let old = before.default_dose.map(|d| format!("{}", d));
            record_history(
                &pool, id, &med_name, "dose_changed",
                &format!(
                    "Default dose changed {} → {} {}",
                    old.clone().unwrap_or_else(|| "—".into()), val, unit
                ),
                old, Some(format!("{}", val)),
            ).await;
            banner = Some(format!("Recorded: {} default dose now {} {}", med_name, val, unit));
        }
        sqlx::query("UPDATE medications SET default_dose = ? WHERE id = ?")
            .bind(val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = dose_unit {
        sqlx::query("UPDATE medications SET dose_unit = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = category {
        sqlx::query("UPDATE medications SET category = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = default_time {
        sqlx::query("UPDATE medications SET default_time = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = med_type {
        sqlx::query("UPDATE medications SET med_type = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = active {
        if before.active != Some(val) {
            let (event, verb) = if val { ("reactivated", "Restarted") } else { ("ceased", "Ceased") };
            record_history(&pool, id, &med_name, event, &format!("{} {}", verb, med_name), None, None).await;
            banner = Some(format!("Recorded: {} {}", verb.to_lowercase(), med_name));
        }
        sqlx::query("UPDATE medications SET active = ? WHERE id = ?")
            .bind(val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    Ok(banner)
}

#[tauri::command]
pub async fn archive_medication(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    if let Some(med) = sqlx::query_as::<_, Medication>("SELECT * FROM medications WHERE id = ?")
        .bind(id).fetch_optional(&*pool).await.map_err(|e| e.to_string())?
    {
        if med.active != Some(false) {
            record_history(&pool, id, &med.name, "ceased", &format!("Ceased {}", med.name), None, None).await;
        }
    }
    sqlx::query("UPDATE medications SET active = 0 WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ── Medication history ──

#[tauri::command]
pub async fn get_medication_history(
    pool: State<'_, SqlitePool>,
    medication_id: Option<i64>,
) -> Result<Vec<MedicationHistoryEntry>, String> {
    let rows = match medication_id {
        Some(mid) => sqlx::query_as::<_, MedicationHistoryEntry>(
            "SELECT * FROM medication_history WHERE medication_id = ? ORDER BY event_date DESC, id DESC")
            .bind(mid).fetch_all(&*pool).await,
        None => sqlx::query_as::<_, MedicationHistoryEntry>(
            "SELECT * FROM medication_history ORDER BY event_date DESC, id DESC")
            .fetch_all(&*pool).await,
    };
    rows.map_err(|e| e.to_string())
}

/// Manually add a history entry (e.g. backfilling a past start/cease date).
#[tauri::command]
pub async fn add_medication_history(
    pool: State<'_, SqlitePool>,
    medication_id: Option<i64>,
    medication_name: String,
    event_type: String,
    event_date: String,
    detail: Option<String>,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO medication_history
         (medication_id, medication_name, event_type, event_date, detail)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(medication_id).bind(&medication_name).bind(&event_type).bind(&event_date).bind(&detail)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Edit an existing history entry's date/type/detail.
#[tauri::command]
pub async fn update_medication_history(
    pool: State<'_, SqlitePool>,
    id: i64,
    event_type: Option<String>,
    event_date: Option<String>,
    detail: Option<String>,
) -> Result<(), String> {
    if let Some(v) = event_type {
        sqlx::query("UPDATE medication_history SET event_type = ? WHERE id = ?")
            .bind(&v).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(v) = event_date {
        sqlx::query("UPDATE medication_history SET event_date = ? WHERE id = ?")
            .bind(&v).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(v) = detail {
        sqlx::query("UPDATE medication_history SET detail = ? WHERE id = ?")
            .bind(&v).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_medication_history(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM medication_history WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Permanently remove a medication and its doses/schedule (cascade). Use
/// `update_medication(active=false)` to merely cease one while keeping history.
#[tauri::command]
pub async fn delete_medication(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM medication_schedule WHERE medication_id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM medications WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ── Medication dose schedule (default dose+time slots, e.g. Dex morning/midday/arvo) ──

#[tauri::command]
pub async fn get_medication_schedule(
    pool: State<'_, SqlitePool>,
    medication_id: Option<i64>,
) -> Result<Vec<MedicationScheduleItem>, String> {
    let rows = match medication_id {
        Some(mid) => sqlx::query_as::<_, MedicationScheduleItem>(
            "SELECT * FROM medication_schedule WHERE medication_id = ? ORDER BY sort_order")
            .bind(mid).fetch_all(&*pool).await,
        None => sqlx::query_as::<_, MedicationScheduleItem>(
            "SELECT * FROM medication_schedule ORDER BY medication_id, sort_order")
            .fetch_all(&*pool).await,
    };
    rows.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_schedule_item(
    pool: State<'_, SqlitePool>,
    medication_id: i64,
    label: Option<String>,
    dose_amount: Option<f64>,
    time_of_day: Option<String>,
) -> Result<i64, String> {
    // Next free slot for this medication.
    let next: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM medication_schedule WHERE medication_id = ?")
        .bind(medication_id).fetch_one(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
         VALUES (?, ?, ?, ?, ?)")
        .bind(medication_id).bind(next.0).bind(&label).bind(dose_amount).bind(&time_of_day)
        .execute(&*pool).await.map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_schedule_item(
    pool: State<'_, SqlitePool>,
    id: i64,
    label: Option<String>,
    dose_amount: Option<f64>,
    time_of_day: Option<String>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE medication_schedule SET label = ?, dose_amount = ?, time_of_day = ? WHERE id = ?")
        .bind(&label).bind(dose_amount).bind(&time_of_day).bind(id)
        .execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_schedule_item(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM medication_schedule WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_doses_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<MedicationDose>, String> {
    sqlx::query_as::<_, MedicationDose>(
        "SELECT md.* FROM medication_doses md
         JOIN medications m ON md.medication_id = m.id
         WHERE md.log_date = ?
         ORDER BY md.time_taken"
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

/// Every logged dose, most recent first — feeds the collapsible month→week→day
/// dose history on the Medication page.
#[tauri::command]
pub async fn get_all_doses(pool: State<'_, SqlitePool>) -> Result<Vec<MedicationDose>, String> {
    sqlx::query_as::<_, MedicationDose>(
        "SELECT md.* FROM medication_doses md
         JOIN medications m ON md.medication_id = m.id
         ORDER BY md.log_date DESC, md.time_taken DESC"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_dose(
    pool: State<'_, SqlitePool>,
    dose: MedicationDose,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO medication_doses (medication_id, log_date, time_taken, dose_amount, notes)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(medication_id, log_date, time_taken) DO UPDATE SET
         dose_amount=excluded.dose_amount, notes=excluded.notes"
    )
    .bind(dose.medication_id).bind(&dose.log_date).bind(&dose.time_taken)
    .bind(dose.dose_amount).bind(&dose.notes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Permanently remove a logged dose (identified by its unique med+date+time key).
#[tauri::command]
pub async fn delete_dose(
    pool: State<'_, SqlitePool>,
    medication_id: i64,
    log_date: String,
    time_taken: Option<String>,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM medication_doses \
         WHERE medication_id = ? AND log_date = ? AND time_taken IS ?"
    )
    .bind(medication_id).bind(&log_date).bind(&time_taken)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}