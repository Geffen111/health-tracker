use crate::models::{
    Medication, MedicationDose, MedicationFill, MedicationHistoryEntry, MedicationScheduleItem, MedicationSupply,
};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
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
    sqlx::query("DELETE FROM medication_supplies WHERE medication_id = ?")
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

// ── Supply ("x days left"), one row per tablet strength / script ──
//
// On hand = the count taken at `base_at`, plus fills entered since, less the tablets in each
// dose logged after it. A dose's tablets come from splitting its amount over the med's
// strengths, largest first (12 mg over 8 mg + 4 mg tablets = one of each). Days left divides
// by the tablets used a day over recent days with any dose logged (so a skipped dose counts),
// falling back to the schedule while there's too little history. All computed here; the page
// only shows `SupplyView`.

/// How many recent days the daily use is averaged over, and the fewest it needs.
const USAGE_WINDOW_DAYS: i64 = 14;
const USAGE_MIN_DAYS: usize = 3;

fn parse_day(d: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// A blank expiry clears it; anything else must be a real date.
fn expiry_or_none(d: Option<String>) -> Result<Option<String>, String> {
    match d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(s) if parse_day(&s).is_none() => Err(format!("Invalid script expiry date: {s}")),
        other => Ok(other),
    }
}

/// True when the med's doses are counted in tablets/capsules rather than mg etc.
fn dosed_in_tablets(med: &Medication) -> bool {
    let u = med.dose_unit.as_deref().unwrap_or("").trim().to_lowercase();
    u.starts_with("tab") || u.starts_with("cap") || u.starts_with("pill")
}

/// Tablets of each supply (by index into `supplies`) that one dose of `amount` uses.
fn split_dose(med: &Medication, supplies: &[&MedicationSupply], amount: Option<f64>) -> Vec<f64> {
    let mut out = vec![0.0; supplies.len()];
    if supplies.is_empty() {
        return out;
    }
    let amount = amount.or(med.default_dose);
    if dosed_in_tablets(med) {
        out[0] = amount.unwrap_or(1.0);
        return out;
    }
    let mut by_strength: Vec<usize> = (0..supplies.len())
        .filter(|&i| supplies[i].strength.map_or(false, |s| s > 0.0))
        .collect();
    let Some(amount) = amount.filter(|a| *a > 0.0) else {
        out[by_strength.first().copied().unwrap_or(0)] = 1.0;
        return out;
    };
    if by_strength.is_empty() {
        // No strength set yet: a dose is one tablet (the page asks for the strength).
        out[0] = 1.0;
        return out;
    }
    by_strength.sort_by(|&a, &b| supplies[b].strength.partial_cmp(&supplies[a].strength).unwrap());
    let mut rem = amount;
    for &i in &by_strength {
        let s = supplies[i].strength.unwrap();
        let n = (rem / s + 1e-9).floor();
        out[i] += n;
        rem -= n * s;
    }
    // What's left over is part of the smallest tablet (a halved one, say).
    if rem > 1e-6 {
        let smallest = *by_strength.last().unwrap();
        out[smallest] += rem / supplies[smallest].strength.unwrap();
    }
    out
}

#[derive(Debug, Serialize)]
pub struct SupplyView {
    #[serde(flatten)]
    pub supply: MedicationSupply,
    pub on_hand: f64,
    /// Tablets used a day, and where that figure came from: 'doses' (the last `usage_days`
    /// days with any dose logged) or 'schedule' (the quick-add buttons, while history is short).
    pub per_day: Option<f64>,
    pub usage_source: Option<String>,
    pub usage_days: i64,
    pub days_left: Option<i64>,
    /// The med is dosed in mg (or similar) but this supply has no strength, so doses can't
    /// be turned into tablets properly.
    pub needs_strength: bool,
}

/// Every supply with what's on hand and how long it lasts, for the given meds (all if None).
async fn supply_views(pool: &SqlitePool, medication_id: Option<i64>) -> Result<Vec<SupplyView>, String> {
    let supplies = sqlx::query_as::<_, MedicationSupply>(
        "SELECT * FROM medication_supplies WHERE (? IS NULL OR medication_id = ?) ORDER BY medication_id, strength DESC, id",
    )
    .bind(medication_id).bind(medication_id)
    .fetch_all(pool).await.map_err(|e| e.to_string())?;
    if supplies.is_empty() {
        return Ok(vec![]);
    }
    let meds = sqlx::query_as::<_, Medication>("SELECT * FROM medications")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    let doses = sqlx::query_as::<_, MedicationDose>("SELECT * FROM medication_doses")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    let slots = sqlx::query_as::<_, MedicationScheduleItem>("SELECT * FROM medication_schedule")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    let fills = sqlx::query_as::<_, MedicationFill>("SELECT * FROM medication_fills")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    let today = Local::now().date_naive();
    let window_from = (today - chrono::Duration::days(USAGE_WINDOW_DAYS)).to_string();
    let today_s = today.to_string();
    // Days the window can use: recent, finished, and with at least one dose of anything
    // logged — a day with nothing logged at all says nothing about a skip.
    let logged_days: std::collections::BTreeSet<&str> = doses
        .iter()
        .map(|d| d.log_date.as_str())
        .filter(|d| *d >= window_from.as_str() && *d < today_s.as_str())
        .collect();

    let mut out = Vec::with_capacity(supplies.len());
    for med in &meds {
        let mine: Vec<&MedicationSupply> = supplies.iter().filter(|s| s.medication_id == med.id).collect();
        if mine.is_empty() {
            continue;
        }
        let med_doses: Vec<&MedicationDose> = doses.iter().filter(|d| d.medication_id == med.id).collect();
        // Tablets per supply for each of this med's doses, worked out once.
        let split: Vec<Vec<f64>> = med_doses.iter().map(|d| split_dose(med, &mine, d.dose_amount)).collect();
        let med_slots: Vec<&MedicationScheduleItem> = slots.iter().filter(|s| s.medication_id == med.id).collect();
        let needs_strength = !dosed_in_tablets(med);

        for (i, s) in mine.iter().enumerate() {
            let filled: f64 = fills
                .iter()
                .filter(|f| f.supply_id == Some(s.id) && f.created_at.as_str() > s.base_at.as_str())
                .map(|f| f.quantity)
                .sum();
            let used: f64 = med_doses
                .iter()
                .zip(&split)
                .filter(|(d, _)| format!("{} {}", d.log_date, d.time_taken.as_deref().unwrap_or("00:00")) > s.base_local)
                .map(|(_, t)| t[i])
                .sum();
            let on_hand = (s.base_count + filled - used).max(0.0);

            let days: Vec<&&str> = logged_days.iter().filter(|d| **d >= s.in_use_since.as_str()).collect();
            let (per_day, source, n_days) = if days.len() >= USAGE_MIN_DAYS {
                let total: f64 = med_doses
                    .iter()
                    .zip(&split)
                    .filter(|(d, _)| d.log_date.as_str() >= s.in_use_since.as_str() && logged_days.contains(d.log_date.as_str()))
                    .map(|(_, t)| t[i])
                    .sum();
                (Some(total / days.len() as f64), Some("doses"), days.len() as i64)
            } else if !med_slots.is_empty() {
                let total: f64 = med_slots.iter().map(|sl| split_dose(med, &mine, sl.dose_amount)[i]).sum();
                (Some(total), Some("schedule"), 0)
            } else {
                (None, None, 0)
            };
            let days_left = per_day.filter(|p| *p > 1e-9).map(|p| (on_hand / p + 1e-9).floor() as i64);
            out.push(SupplyView {
                supply: (*s).clone(),
                on_hand,
                per_day,
                usage_source: source.map(String::from),
                usage_days: n_days,
                days_left,
                needs_strength: needs_strength && s.strength.is_none(),
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn list_medication_supplies(pool: State<'_, SqlitePool>) -> Result<Vec<SupplyView>, String> {
    supply_views(&pool, None).await
}

#[derive(Debug, Deserialize)]
pub struct SupplyInput {
    /// None adds a new strength / script.
    pub id: Option<i64>,
    pub medication_id: i64,
    pub strength: Option<f64>,
    pub label: Option<String>,
    /// Count the tablets as of now; None keeps the running count (a new supply starts at 0).
    pub on_hand: Option<f64>,
    pub box_size: Option<f64>,
    pub boxes_per_fill: f64,
    pub warn_days: i64,
    pub repeats_left: Option<i64>,
    pub needs_script: bool,
    pub script_expiry: Option<String>,
}

#[tauri::command]
pub async fn save_medication_supply(pool: State<'_, SqlitePool>, supply: SupplyInput) -> Result<(), String> {
    let now = Local::now();
    let local = now.format("%Y-%m-%d %H:%M").to_string();
    let today = now.format("%Y-%m-%d").to_string();
    let expiry = expiry_or_none(supply.script_expiry)?;
    let label = supply.label.filter(|l| !l.trim().is_empty());
    let strength = supply.strength.filter(|s| *s > 0.0);
    let repeats = supply.repeats_left.map(|r| r.max(0));
    let id = match supply.id {
        Some(id) => id,
        None => sqlx::query_scalar::<_, i64>(
            "INSERT INTO medication_supplies (medication_id, base_count, base_at, base_local, in_use_since)
             VALUES (?, 0, datetime('now'), ?, ?) RETURNING id",
        )
        .bind(supply.medication_id).bind(&local).bind(&today)
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?,
    };
    sqlx::query(
        "UPDATE medication_supplies SET strength = ?, label = ?, box_size = ?, boxes_per_fill = ?,
            warn_days = ?, repeats_left = ?, needs_script = ?, script_expiry = ? WHERE id = ?",
    )
    .bind(strength).bind(label).bind(supply.box_size.filter(|b| *b > 0.0))
    .bind(supply.boxes_per_fill.max(0.0)).bind(supply.warn_days.max(0))
    .bind(repeats).bind(supply.needs_script).bind(expiry).bind(id)
    .execute(&*pool).await.map_err(|e| e.to_string())?;
    if let Some(n) = supply.on_hand {
        sqlx::query("UPDATE medication_supplies SET base_count = ?, base_at = datetime('now'), base_local = ? WHERE id = ?")
            .bind(n.max(0.0)).bind(&local).bind(id)
            .execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_medication_supply(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM medication_fills WHERE supply_id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM medication_supplies WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Record a script fill / top-up of one supply. The tablets join the count from now; the
/// repeats left, new-script flag and expiry come from the fill form. `box_size` becomes the
/// usual box size only when none is set yet.
#[tauri::command]
pub async fn record_medication_fill(
    pool: State<'_, SqlitePool>,
    supply_id: i64,
    fill_date: String,
    quantity: f64,
    repeats_left: Option<i64>,
    needs_script: bool,
    box_size: Option<f64>,
    script_expiry: Option<String>,
    note: Option<String>,
) -> Result<(), String> {
    parse_day(&fill_date).ok_or("Invalid fill date")?;
    if !(quantity > 0.0) {
        return Err("Quantity must be more than zero".into());
    }
    let expiry = expiry_or_none(script_expiry)?;
    let note = note.filter(|n| !n.trim().is_empty());
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let med_id = sqlx::query_scalar::<_, i64>(
        "UPDATE medication_supplies SET repeats_left = ?, needs_script = ?, script_expiry = ?,
            box_size = COALESCE(box_size, ?) WHERE id = ? RETURNING medication_id",
    )
    .bind(repeats_left.map(|r| r.max(0))).bind(needs_script).bind(expiry)
    .bind(box_size.filter(|b| *b > 0.0)).bind(supply_id)
    .fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO medication_fills (medication_id, supply_id, fill_date, quantity, note) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(med_id).bind(supply_id).bind(&fill_date).bind(quantity).bind(note)
    .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
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

/// Undo a fill entered by mistake. One entered after the last count comes off the total; one
/// from before was already in that count, so removing it changes nothing on hand.
#[tauri::command]
pub async fn delete_medication_fill(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM medication_fills WHERE id = ?")
        .bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn med(unit: &str, default_dose: Option<f64>) -> Medication {
        Medication {
            id: 1, name: "M".into(), short_code: None, default_dose, dose_unit: Some(unit.into()),
            category: None, active: Some(true), notes: None, default_time: None, med_type: None,
        }
    }
    fn supply(id: i64, strength: Option<f64>) -> MedicationSupply {
        MedicationSupply {
            id, medication_id: 1, strength, label: None, base_count: 0.0, base_at: String::new(),
            base_local: String::new(), in_use_since: String::new(), box_size: None, boxes_per_fill: 1.0,
            warn_days: 7, repeats_left: None, needs_script: false, script_expiry: None, created_at: String::new(),
        }
    }

    #[test]
    fn splits_a_dose_over_strengths_largest_first() {
        let m = med("mg", Some(12.0));
        let (a, b) = (supply(1, Some(4.0)), supply(2, Some(8.0)));
        assert_eq!(split_dose(&m, &[&a, &b], Some(12.0)), vec![1.0, 1.0]);
        assert_eq!(split_dose(&m, &[&a, &b], Some(16.0)), vec![0.0, 2.0]);
        assert_eq!(split_dose(&m, &[&a, &b], None), vec![1.0, 1.0]); // default dose
        assert_eq!(split_dose(&m, &[&a, &b], Some(2.0)), vec![0.5, 0.0]); // half a 4 mg
    }

    #[test]
    fn one_strength_counts_tablets_per_dose() {
        let m = med("mg", None);
        let dex = supply(1, Some(5.0));
        assert_eq!(split_dose(&m, &[&dex], Some(15.0)), vec![3.0]);
        assert_eq!(split_dose(&m, &[&dex], Some(10.0)), vec![2.0]);
        // Without a strength each dose is one tablet; in tablets the amount is the count.
        assert_eq!(split_dose(&m, &[&supply(1, None)], Some(15.0)), vec![1.0]);
        assert_eq!(split_dose(&med("tablets", None), &[&supply(1, None)], Some(2.0)), vec![2.0]);
    }
}
