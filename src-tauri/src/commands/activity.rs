use crate::models::{ActivityCategory, ActivityType, ActivityEntry};
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn list_activity_categories(pool: State<'_, SqlitePool>) -> Result<Vec<ActivityCategory>, String> {
    sqlx::query_as::<_, ActivityCategory>("SELECT * FROM activity_categories ORDER BY name")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_activity_types(
    pool: State<'_, SqlitePool>,
    category_id: Option<i64>,
) -> Result<Vec<ActivityType>, String> {
    if let Some(cat_id) = category_id {
        sqlx::query_as::<_, ActivityType>(
            "SELECT * FROM activity_types WHERE category_id = ? ORDER BY name"
        )
        .bind(cat_id)
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
    } else {
        sqlx::query_as::<_, ActivityType>("SELECT * FROM activity_types ORDER BY name")
            .fetch_all(&*pool)
            .await
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn get_activities_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<ActivityEntry>, String> {
    sqlx::query_as::<_, ActivityEntry>(
        "SELECT * FROM activity_log WHERE log_date = ? ORDER BY duration_hours DESC"
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_activity_entry(pool: State<'_, SqlitePool>, entry: ActivityEntry) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO activity_log (log_date, activity_type_id, duration_hours, energy_cost, notes)
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&entry.log_date).bind(entry.activity_type_id).bind(entry.duration_hours)
    .bind(&entry.energy_cost).bind(&entry.notes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Upsert a single day's duration for one activity type. Energy cost is taken
/// from the activity type's default (auto-detected — the UI no longer asks).
/// A duration of 0 (or less) clears any existing entry for that day+type.
#[tauri::command]
pub async fn set_activity_duration(
    pool: State<'_, SqlitePool>,
    log_date: String,
    activity_type_id: i64,
    duration_hours: f64,
) -> Result<(), String> {
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM activity_log WHERE log_date = ? AND activity_type_id = ?")
        .bind(&log_date).bind(activity_type_id)
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    if duration_hours <= 0.0 {
        if let Some((id,)) = existing {
            sqlx::query("DELETE FROM activity_log WHERE id = ?")
                .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let energy: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT default_energy_cost FROM activity_types WHERE id = ?")
        .bind(activity_type_id)
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;
    let energy_cost = energy.and_then(|r| r.0).unwrap_or_else(|| "Medium".to_string());

    match existing {
        Some((id,)) => {
            sqlx::query("UPDATE activity_log SET duration_hours = ?, energy_cost = ? WHERE id = ?")
                .bind(duration_hours).bind(&energy_cost).bind(id)
                .execute(&*pool).await.map_err(|e| e.to_string())?;
        }
        None => {
            sqlx::query(
                "INSERT INTO activity_log (log_date, activity_type_id, duration_hours, energy_cost)
                 VALUES (?, ?, ?, ?)")
                .bind(&log_date).bind(activity_type_id).bind(duration_hours).bind(&energy_cost)
                .execute(&*pool).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_activity_entry(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM activity_log WHERE id = ?")
        .bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
// ── Category & activity-type management ──
//
// Categories and types were seed-only until now: the migrations created them and nothing in
// the app could add, rename or retune one. These commands make both editable. Deletes are
// refused while anything still points at the row, so a category can never be orphaned and a
// logged day can never lose the activity it referenced.

/// Load groups a category may contribute to. Anything else is rejected rather than silently
/// stored, since `pacing.rs` matches on these exact values.
const LOAD_GROUPS: [&str; 3] = ["physical", "cognitive", "sensory"];
/// Energy costs an activity type may default to — the factors applied in `pacing.rs`.
const ENERGY_COSTS: [&str; 3] = ["Low", "Medium", "High"];

fn validate(value: &str, allowed: &[&str], field: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("Invalid {}: '{}'. Expected one of {:?}.", field, value, allowed))
    }
}

fn clean_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Name cannot be empty.".into());
    }
    Ok(n.to_string())
}

/// An activity type plus how many logged entries reference it, so the UI can show usage and
/// disable deletion of a type that is in use.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ActivityTypeUsage {
    pub id: i64,
    pub name: String,
    pub category_id: i64,
    pub default_energy_cost: Option<String>,
    pub entry_count: i64,
}

#[tauri::command]
pub async fn list_activity_types_with_usage(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<ActivityTypeUsage>, String> {
    sqlx::query_as::<_, ActivityTypeUsage>(
        "SELECT at.id, at.name, at.category_id, at.default_energy_cost, \
                CAST(COUNT(al.id) AS INTEGER) AS entry_count \
         FROM activity_types at \
         LEFT JOIN activity_log al ON al.activity_type_id = at.id \
         GROUP BY at.id ORDER BY at.name",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_activity_category(
    pool: State<'_, SqlitePool>,
    name: String,
    energy_weight: f64,
    load_group: String,
) -> Result<i64, String> {
    let name = clean_name(&name)?;
    validate(&load_group, &LOAD_GROUPS, "load group")?;
    sqlx::query("INSERT INTO activity_categories (name, energy_weight, load_group) VALUES (?, ?, ?)")
        .bind(&name).bind(energy_weight).bind(&load_group)
        .execute(&*pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| if e.to_string().contains("UNIQUE") {
            format!("A category called '{}' already exists.", name)
        } else {
            e.to_string()
        })
}

#[tauri::command]
pub async fn update_activity_category(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: String,
    energy_weight: f64,
    load_group: String,
) -> Result<(), String> {
    let name = clean_name(&name)?;
    validate(&load_group, &LOAD_GROUPS, "load group")?;
    sqlx::query("UPDATE activity_categories SET name = ?, energy_weight = ?, load_group = ? WHERE id = ?")
        .bind(&name).bind(energy_weight).bind(&load_group).bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| if e.to_string().contains("UNIQUE") {
            format!("A category called '{}' already exists.", name)
        } else {
            e.to_string()
        })?;
    Ok(())
}

/// Refused while any activity type still belongs to the category — deleting it would leave
/// those types pointing at a missing row, and every load figure that uses them would break.
#[tauri::command]
pub async fn delete_activity_category(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let used: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM activity_types WHERE category_id = ?")
        .bind(id)
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?;
    if used.0 > 0 {
        return Err(format!(
            "{} {} still in this category. Move {} to another category first.",
            used.0,
            if used.0 == 1 { "activity is" } else { "activities are" },
            if used.0 == 1 { "it" } else { "them" },
        ));
    }
    sqlx::query("DELETE FROM activity_categories WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn create_activity_type(
    pool: State<'_, SqlitePool>,
    name: String,
    category_id: i64,
    default_energy_cost: String,
) -> Result<i64, String> {
    let name = clean_name(&name)?;
    validate(&default_energy_cost, &ENERGY_COSTS, "energy cost")?;
    sqlx::query("INSERT INTO activity_types (name, category_id, default_energy_cost) VALUES (?, ?, ?)")
        .bind(&name).bind(category_id).bind(&default_energy_cost)
        .execute(&*pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| if e.to_string().contains("UNIQUE") {
            format!("An activity called '{}' already exists.", name)
        } else {
            e.to_string()
        })
}

/// Editing the energy cost here changes the DEFAULT applied to future entries. Days already
/// logged keep the cost stored on their `activity_log` row, so history doesn't shift under you.
#[tauri::command]
pub async fn update_activity_type(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: String,
    category_id: i64,
    default_energy_cost: String,
) -> Result<(), String> {
    let name = clean_name(&name)?;
    validate(&default_energy_cost, &ENERGY_COSTS, "energy cost")?;
    sqlx::query("UPDATE activity_types SET name = ?, category_id = ?, default_energy_cost = ? WHERE id = ?")
        .bind(&name).bind(category_id).bind(&default_energy_cost).bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| if e.to_string().contains("UNIQUE") {
            format!("An activity called '{}' already exists.", name)
        } else {
            e.to_string()
        })?;
    Ok(())
}

/// Refused while any day has this activity logged, so deleting from the manage panel can
/// never silently erase entries out of the history.
#[tauri::command]
pub async fn delete_activity_type(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let used: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM activity_log WHERE activity_type_id = ?")
        .bind(id)
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?;
    if used.0 > 0 {
        return Err(format!(
            "This activity is logged on {} {}. Clear those entries first if you really want it gone.",
            used.0,
            if used.0 == 1 { "day" } else { "days" },
        ));
    }
    sqlx::query("DELETE FROM activity_types WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}
