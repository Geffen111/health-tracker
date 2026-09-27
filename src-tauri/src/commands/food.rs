//! Food & drink log. Not calorie counting — an item is a name and a kind ('food' or
//! 'drink'), and the log records which items were had on which day, so the Food page can
//! set them beside the fatigue ratings. Groups ("Usual breakfast") log several items in
//! one go but expand into one `food_log` row per item.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use tauri::State;

const KINDS: [&str; 2] = ["food", "drink"];

#[derive(Debug, Serialize, FromRow)]
pub struct Food {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub regular: bool,
    pub active: bool,
    /// Days this item appears on — shown in the list and used to refuse a delete.
    pub days_logged: i64,
}

#[derive(Debug, Serialize)]
pub struct FoodGroup {
    pub id: i64,
    pub name: String,
    pub default_time: Option<String>,
    pub food_ids: Vec<i64>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct FoodLogEntry {
    pub id: i64,
    pub log_date: String,
    pub time_taken: Option<String>,
    pub food_id: i64,
    pub food_name: String,
    pub kind: String,
    pub amount: Option<String>,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewFoodLog {
    pub food_id: i64,
    pub time_taken: Option<String>,
    pub amount: Option<String>,
    pub group_id: Option<i64>,
}

/// One (item, day) pair — the input to the fatigue comparison on the Food page.
#[derive(Debug, Serialize, FromRow)]
pub struct FoodDay {
    pub food_id: i64,
    pub log_date: String,
}

fn clean_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Name cannot be empty.".into());
    }
    Ok(n.to_string())
}

fn check_kind(kind: &str) -> Result<(), String> {
    if KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(format!("Invalid kind '{}'. Expected one of {:?}.", kind, KINDS))
    }
}

fn dup_err(e: sqlx::Error, what: &str, name: &str) -> String {
    if e.to_string().contains("UNIQUE") {
        format!("{} called '{}' already exists.", what, name)
    } else {
        e.to_string()
    }
}

fn blank_to_none(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

// ── Items ──

#[tauri::command]
pub async fn list_foods(pool: State<'_, SqlitePool>) -> Result<Vec<Food>, String> {
    sqlx::query_as::<_, Food>(
        "SELECT f.id, f.name, f.kind, f.regular, f.active, \
                CAST(COUNT(DISTINCT fl.log_date) AS INTEGER) AS days_logged \
         FROM foods f LEFT JOIN food_log fl ON fl.food_id = f.id \
         GROUP BY f.id ORDER BY f.name COLLATE NOCASE",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_food(
    pool: State<'_, SqlitePool>,
    name: String,
    kind: String,
    regular: bool,
) -> Result<i64, String> {
    let name = clean_name(&name)?;
    check_kind(&kind)?;
    sqlx::query("INSERT INTO foods (name, kind, regular) VALUES (?, ?, ?)")
        .bind(&name).bind(&kind).bind(regular)
        .execute(&*pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| dup_err(e, "An item", &name))
}

#[tauri::command]
pub async fn update_food(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: String,
    kind: String,
    regular: bool,
    active: bool,
) -> Result<(), String> {
    let name = clean_name(&name)?;
    check_kind(&kind)?;
    sqlx::query("UPDATE foods SET name = ?, kind = ?, regular = ?, active = ? WHERE id = ?")
        .bind(&name).bind(&kind).bind(regular).bind(active).bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| dup_err(e, "An item", &name))?;
    Ok(())
}

/// Refused while the item is in the log, so a delete can never quietly erase history —
/// hiding it (active = 0) is the way to take a logged item off the lists.
#[tauri::command]
pub async fn delete_food(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let used: (i64,) = sqlx::query_as("SELECT COUNT(DISTINCT log_date) FROM food_log WHERE food_id = ?")
        .bind(id)
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?;
    if used.0 > 0 {
        return Err(format!(
            "This item is logged on {} {}. Hide it instead to keep that history.",
            used.0,
            if used.0 == 1 { "day" } else { "days" },
        ));
    }
    sqlx::query("DELETE FROM food_group_items WHERE food_id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM foods WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ── Groups ──

#[tauri::command]
pub async fn list_food_groups(pool: State<'_, SqlitePool>) -> Result<Vec<FoodGroup>, String> {
    let groups: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT id, name, default_time FROM food_groups ORDER BY name COLLATE NOCASE")
            .fetch_all(&*pool).await.map_err(|e| e.to_string())?;
    let items: Vec<(i64, i64)> = sqlx::query_as("SELECT group_id, food_id FROM food_group_items")
        .fetch_all(&*pool).await.map_err(|e| e.to_string())?;
    Ok(groups
        .into_iter()
        .map(|(id, name, default_time)| FoodGroup {
            id,
            name,
            default_time,
            food_ids: items.iter().filter(|(g, _)| *g == id).map(|(_, f)| *f).collect(),
        })
        .collect())
}

/// Create (`id` = None) or replace a group, including its full member list.
#[tauri::command]
pub async fn save_food_group(
    pool: State<'_, SqlitePool>,
    id: Option<i64>,
    name: String,
    default_time: Option<String>,
    food_ids: Vec<i64>,
) -> Result<i64, String> {
    let name = clean_name(&name)?;
    let default_time = blank_to_none(default_time);
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let gid = match id {
        Some(gid) => {
            sqlx::query("UPDATE food_groups SET name = ?, default_time = ? WHERE id = ?")
                .bind(&name).bind(&default_time).bind(gid)
                .execute(&mut *tx).await.map_err(|e| dup_err(e, "A group", &name))?;
            sqlx::query("DELETE FROM food_group_items WHERE group_id = ?")
                .bind(gid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
            gid
        }
        None => sqlx::query("INSERT INTO food_groups (name, default_time) VALUES (?, ?)")
            .bind(&name).bind(&default_time)
            .execute(&mut *tx).await
            .map(|r| r.last_insert_rowid())
            .map_err(|e| dup_err(e, "A group", &name))?,
    };
    for fid in food_ids {
        sqlx::query("INSERT OR IGNORE INTO food_group_items (group_id, food_id) VALUES (?, ?)")
            .bind(gid).bind(fid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(gid)
}

/// Removing a group keeps everything it logged — those rows just lose the group label.
#[tauri::command]
pub async fn delete_food_group(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("UPDATE food_log SET group_id = NULL WHERE group_id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM food_group_items WHERE group_id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM food_groups WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

// ── Log ──

#[tauri::command]
pub async fn get_food_log_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<FoodLogEntry>, String> {
    sqlx::query_as::<_, FoodLogEntry>(
        "SELECT fl.id, fl.log_date, fl.time_taken, fl.food_id, f.name AS food_name, f.kind, \
                fl.amount, fl.group_id, g.name AS group_name \
         FROM food_log fl \
         JOIN foods f ON f.id = fl.food_id \
         LEFT JOIN food_groups g ON g.id = fl.group_id \
         WHERE fl.log_date = ? \
         ORDER BY fl.time_taken IS NULL, fl.time_taken, fl.id",
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

/// Log one or more items against a day in a single transaction (a group logs several).
#[tauri::command]
pub async fn add_food_log(
    pool: State<'_, SqlitePool>,
    log_date: String,
    entries: Vec<NewFoodLog>,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for e in entries {
        sqlx::query(
            "INSERT INTO food_log (log_date, time_taken, food_id, amount, group_id) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&log_date)
        .bind(blank_to_none(e.time_taken))
        .bind(e.food_id)
        .bind(blank_to_none(e.amount))
        .bind(e.group_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_food_log(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM food_log WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Every distinct (item, day) pair in the log.
#[tauri::command]
pub async fn get_food_days(pool: State<'_, SqlitePool>) -> Result<Vec<FoodDay>, String> {
    sqlx::query_as::<_, FoodDay>("SELECT DISTINCT food_id, log_date FROM food_log ORDER BY log_date")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}
