use crate::models::{Medication, MedicationDose};
use sqlx::SqlitePool;
use tauri::State;

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
) -> Result<Medication, String> {
    sqlx::query_as::<_, Medication>(
        "INSERT INTO medications (name, short_code, default_dose, dose_unit, category)
         VALUES (?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&name).bind(&short_code).bind(default_dose).bind(&dose_unit).bind(&category)
    .fetch_one(&*pool)
    .await
    .map_err(|e| e.to_string())
}

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
) -> Result<(), String> {
    if let Some(name) = name {
        sqlx::query("UPDATE medications SET name = ? WHERE id = ?")
            .bind(&name).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = short_code {
        sqlx::query("UPDATE medications SET short_code = ? WHERE id = ?")
            .bind(&val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(val) = default_dose {
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
    if let Some(val) = active {
        sqlx::query("UPDATE medications SET active = ? WHERE id = ?")
            .bind(val).bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn archive_medication(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("UPDATE medications SET active = 0 WHERE id = ?")
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