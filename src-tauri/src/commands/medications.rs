use crate::models::{Medication, MedicationDose, MedicationFill, MedicationHistoryEntry, MedicationScheduleItem};
use chrono::{Local, NaiveDate};
use serde::Deserialize;
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

    // An as-needed med isn't "started" the way a daily one is — it's added to the list.
    if med.med_type.as_deref() == Some("occasional") {
        record_history(&pool, med.id, &med.name, "added", "Occasional medication added", None, None).await;
    } else {
        record_history(&pool, med.id, &med.name, "started", &format!("Started {}", med.name), None, None).await;
    }
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
    hide_from_timeline: Option<bool>,
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
    if let Some(v) = hide_from_timeline {
        sqlx::query("UPDATE medication_history SET hide_from_timeline = ? WHERE id = ?")
            .bind(v).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
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
    sqlx::query("DELETE FROM medication_fills WHERE medication_id = ?")
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
    bulk_routine: Option<String>,
) -> Result<i64, String> {
    // Next free slot for this medication.
    let next: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM medication_schedule WHERE medication_id = ?")
        .bind(medication_id).fetch_one(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day, bulk_routine)
         VALUES (?, ?, ?, ?, ?, ?)")
        .bind(medication_id).bind(next.0).bind(&label).bind(dose_amount).bind(&time_of_day).bind(&bulk_routine)
        .execute(&*pool).await.map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_schedule_item(
    pool: State<'_, SqlitePool>,
    id: i64,
    label: Option<String>,
    dose_amount: Option<f64>,
    time_of_day: Option<String>,
    bulk_routine: Option<String>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE medication_schedule SET label = ?, dose_amount = ?, time_of_day = ?, bulk_routine = ? WHERE id = ?")
        .bind(&label).bind(dose_amount).bind(&time_of_day).bind(&bulk_routine).bind(id)
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

// ── Database Settings (Synced) ──

#[tauri::command]
pub async fn get_db_setting(pool: State<'_, SqlitePool>, key: String) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|r| r.0))
}

#[tauri::command]
pub async fn save_db_setting(pool: State<'_, SqlitePool>, key: String, value: String) -> Result<(), String> {
    sqlx::query("INSERT INTO app_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key).bind(value)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
// ── Day note (free text shown above the day's doses) ──

#[tauri::command]
pub async fn get_medication_note(pool: State<'_, SqlitePool>, date: String) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT note FROM medication_notes WHERE log_date = ?")
        .bind(&date)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|r| r.0))
}

/// Save the day's note; a blank note removes the row rather than storing an empty string.
#[tauri::command]
pub async fn save_medication_note(pool: State<'_, SqlitePool>, date: String, note: String) -> Result<(), String> {
    if note.trim().is_empty() {
        sqlx::query("DELETE FROM medication_notes WHERE log_date = ?")
            .bind(&date).execute(&*pool).await.map_err(|e| e.to_string())?;
    } else {
        sqlx::query(
            "INSERT INTO medication_notes (log_date, note) VALUES (?, ?)
             ON CONFLICT(log_date) DO UPDATE SET note = excluded.note, updated_at = datetime('now')")
            .bind(&date).bind(&note).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Supply counter ("x days left") ──

fn parse_day(d: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// Tablets on hand on `day`: the recorded count, less a day's use for each day since it
/// was recorded. Never below zero. `$lib/medSupply.ts` (onHandAt) is the same sum.
fn on_hand_at(med: &Medication, day: NaiveDate) -> f64 {
    let count = med.supply_count.unwrap_or(0.0);
    let since = med.supply_date.as_deref().and_then(parse_day);
    let per_day = med.supply_per_day.unwrap_or(0.0);
    let days = since.map(|s| (day - s).num_days().max(0)).unwrap_or(0) as f64;
    (count - per_day * days).max(0.0)
}

async fn fetch_med(pool: &SqlitePool, id: i64) -> Result<Medication, String> {
    sqlx::query_as::<_, Medication>("SELECT * FROM medications WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct SupplySettings {
    pub tracking: bool,
    /// Set the count to this many tablets as of today; None keeps the running count.
    pub on_hand: Option<f64>,
    pub per_day: Option<f64>,
    pub box_size: Option<f64>,
    pub boxes_per_fill: f64,
    pub warn_days: i64,
    pub repeats_left: Option<i64>,
    pub needs_script: bool,
}

/// Save a medication's supply settings. Changing the daily use re-bases the count at
/// today first, so the days already gone are charged at the old rate.
#[tauri::command]
pub async fn save_medication_supply(
    pool: State<'_, SqlitePool>,
    id: i64,
    supply: SupplySettings,
) -> Result<Medication, String> {
    let med = fetch_med(&pool, id).await?;
    let today = Local::now().date_naive();
    let (count, date) = match supply.on_hand {
        Some(n) => (Some(n.max(0.0)), Some(today.to_string())),
        None if med.supply_count.is_some() && med.supply_per_day != supply.per_day => {
            (Some(on_hand_at(&med, today)), Some(today.to_string()))
        }
        None => (med.supply_count, med.supply_date.clone()),
    };
    sqlx::query(
        "UPDATE medications SET supply_tracking = ?, supply_count = ?, supply_date = ?,
            supply_per_day = ?, supply_box_size = ?, supply_boxes_per_fill = ?,
            supply_warn_days = ?, script_repeats_left = ?, needs_script = ?
         WHERE id = ?",
    )
    .bind(supply.tracking).bind(count).bind(date)
    .bind(supply.per_day).bind(supply.box_size).bind(supply.boxes_per_fill.max(0.0))
    .bind(supply.warn_days.max(0)).bind(supply.repeats_left.map(|r| r.max(0))).bind(supply.needs_script)
    .bind(id)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    fetch_med(&pool, id).await
}

/// Record a script fill / top-up: what was left on `fill_date` plus `quantity` becomes
/// the new count from that day. The repeats left and new-script flag come from the
/// fill form (it suggests one repeat fewer, and the flag once none are left).
/// `box_size` becomes the usual box size only when none is set yet.
#[tauri::command]
pub async fn record_medication_fill(
    pool: State<'_, SqlitePool>,
    id: i64,
    fill_date: String,
    quantity: f64,
    repeats_left: Option<i64>,
    needs_script: bool,
    box_size: Option<f64>,
    note: Option<String>,
) -> Result<Medication, String> {
    let day = parse_day(&fill_date).ok_or("Invalid fill date")?;
    if !(quantity > 0.0) {
        return Err("Quantity must be more than zero".into());
    }
    let med = fetch_med(&pool, id).await?;
    let left = on_hand_at(&med, day);
    let note = note.filter(|n| !n.trim().is_empty());
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE medications SET supply_tracking = 1, supply_count = ?, supply_date = ?,
            script_repeats_left = ?, needs_script = ?,
            supply_box_size = COALESCE(supply_box_size, ?) WHERE id = ?",
    )
    .bind(left + quantity).bind(&fill_date)
    .bind(repeats_left.map(|r| r.max(0))).bind(needs_script)
    .bind(box_size.filter(|b| *b > 0.0)).bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO medication_fills (medication_id, fill_date, quantity, note) VALUES (?, ?, ?, ?)")
        .bind(id).bind(&fill_date).bind(quantity).bind(note)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    fetch_med(&pool, id).await
}

#[tauri::command]
pub async fn list_medication_fills(
    pool: State<'_, SqlitePool>,
    medication_id: i64,
) -> Result<Vec<MedicationFill>, String> {
    sqlx::query_as::<_, MedicationFill>(
        "SELECT * FROM medication_fills WHERE medication_id = ? ORDER BY fill_date DESC, id DESC",
    )
    .bind(medication_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

/// Undo a fill entered by mistake: its tablets come off the count again (the repeats
/// and script flag are left for the person to set back).
#[tauri::command]
pub async fn delete_medication_fill(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let fill = sqlx::query_as::<_, MedicationFill>("SELECT * FROM medication_fills WHERE id = ?")
        .bind(id)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    let Some(fill) = fill else { return Ok(()) };
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE medications SET supply_count = MAX(0, COALESCE(supply_count, 0) - ?) WHERE id = ?")
        .bind(fill.quantity).bind(fill.medication_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM medication_fills WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}
