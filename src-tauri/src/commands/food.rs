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
    /// Log entries for this item (a day can hold several) — the "most added" sort.
    pub times_logged: i64,
    /// Most recent day it was logged — the "recently used" sort.
    pub last_logged: Option<String>,
    pub category_id: Option<i64>,
    /// NULL = not yet tagged, 'ai' or 'user' — see commands/food_tags.rs.
    pub tag_source: Option<String>,
    #[sqlx(skip)]
    pub flag_ids: Vec<i64>,
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
    /// When it was saved. Items logged in one action share it, which is how the Food page
    /// spots a meal even when no time was entered.
    pub created_at: String,
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
    let mut foods = sqlx::query_as::<_, Food>(
        "SELECT f.id, f.name, f.kind, f.regular, f.active, f.category_id, f.tag_source, \
                CAST(COUNT(DISTINCT fl.log_date) AS INTEGER) AS days_logged,                 CAST(COUNT(fl.id) AS INTEGER) AS times_logged,                 MAX(fl.log_date) AS last_logged \
         FROM foods f LEFT JOIN food_log fl ON fl.food_id = f.id \
         GROUP BY f.id ORDER BY f.name COLLATE NOCASE",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    let flags: Vec<(i64, i64)> = sqlx::query_as("SELECT food_id, flag_id FROM food_item_flags")
        .fetch_all(&*pool).await.map_err(|e| e.to_string())?;
    for f in &mut foods {
        f.flag_ids = flags.iter().filter(|(fid, _)| *fid == f.id).map(|(_, g)| *g).collect();
    }
    Ok(foods)
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
    sqlx::query("DELETE FROM food_item_flags WHERE food_id = ?")
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
                fl.amount, fl.group_id, g.name AS group_name, fl.created_at \
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

/// Every entry between two dates (inclusive), oldest first — for "Same as yesterday",
/// recent meals and suggested groups on the Food page.
#[tauri::command]
pub async fn get_food_log_range(
    pool: State<'_, SqlitePool>,
    from: String,
    to: String,
) -> Result<Vec<FoodLogEntry>, String> {
    sqlx::query_as::<_, FoodLogEntry>(
        "SELECT fl.id, fl.log_date, fl.time_taken, fl.food_id, f.name AS food_name, f.kind, \
                fl.amount, fl.group_id, g.name AS group_name, fl.created_at \
         FROM food_log fl \
         JOIN foods f ON f.id = fl.food_id \
         LEFT JOIN food_groups g ON g.id = fl.group_id \
         WHERE fl.log_date BETWEEN ? AND ? \
         ORDER BY fl.log_date, fl.time_taken IS NULL, fl.time_taken, fl.id",
    )
    .bind(&from)
    .bind(&to)
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

// ── Photo recognition ──

#[derive(Debug, Serialize, Deserialize)]
pub struct FoodSuggestion {
    pub name: String,
    pub kind: String,
}

#[derive(Deserialize)]
struct RecognisedItems {
    #[serde(default)]
    items: Vec<FoodSuggestion>,
}

const MAX_SUGGESTIONS: usize = 12;

fn recognition_prompt(known: &[String]) -> String {
    let known_list = if known.is_empty() {
        "(none yet)".to_string()
    } else {
        known.join(", ")
    };
    format!(
        r#"List the distinct foods and drinks you can see in this photo of a meal.

Rules:
- One entry per item a person would log separately ("Toast", "Scrambled eggs", "Flat white"), not the whole dish as one entry unless it really is one item (e.g. "Lasagne").
- If an item matches one of the names the person already uses, return that exact name. Their names: {known_list}
- kind is "drink" for anything drunk, otherwise "food".
- No quantities, brands or calories. Skip cutlery, plates and anything you are unsure is food.
- Return ONLY JSON, no commentary: {{"items":[{{"name":"...","kind":"food"}}]}}
- If there is no food or drink in the photo, return {{"items":[]}}."#
    )
}

/// Parse the model's reply, tolerating a code fence or stray text around the JSON.
fn parse_suggestions(reply: &str) -> Result<Vec<FoodSuggestion>, String> {
    let body = crate::commands::ai::strip_code_fences(reply);
    let json = match (body.find('{'), body.rfind('}')) {
        (Some(a), Some(b)) if b > a => &body[a..=b],
        _ => return Err("The model didn't return a list of items.".into()),
    };
    let parsed: RecognisedItems =
        serde_json::from_str(json).map_err(|_| "The model's reply couldn't be read as a list of items.".to_string())?;
    let mut out: Vec<FoodSuggestion> = Vec::new();
    for s in parsed.items {
        let name = s.name.trim().to_string();
        if name.is_empty() || out.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let kind = if s.kind.eq_ignore_ascii_case("drink") { "drink" } else { "food" };
        out.push(FoodSuggestion { name, kind: kind.into() });
        if out.len() == MAX_SUGGESTIONS {
            break;
        }
    }
    Ok(out)
}

/// Send a meal photo to the vision model and return what it recognised. Nothing is
/// logged here — the Food page shows the suggestions and the person picks which to add.
/// The photo is sent only when they drop one in; it isn't stored.
#[tauri::command]
pub async fn recognise_food_photo(
    pool: State<'_, SqlitePool>,
    data_base64: String,
    mime_type: String,
) -> Result<Vec<FoodSuggestion>, String> {
    let api_key = crate::commands::settings::get_api_key()
        .await?
        .ok_or_else(|| "Add an OpenRouter API key in Settings to use photo recognition.".to_string())?;
    let known: Vec<(String,)> = sqlx::query_as("SELECT name FROM foods WHERE active = 1 ORDER BY name COLLATE NOCASE")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    let known: Vec<String> = known.into_iter().map(|r| r.0).collect();
    let data_url = format!("data:{};base64,{}", mime_type, data_base64);
    let reply = crate::commands::ai::call_openrouter_vision(&api_key, &recognition_prompt(&known), &data_url, 2048).await?;
    parse_suggestions(&reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json_and_normalises() {
        let reply = "```json\n{\"items\":[{\"name\":\" Toast \",\"kind\":\"food\"},{\"name\":\"Coffee\",\"kind\":\"Drink\"},{\"name\":\"toast\",\"kind\":\"food\"},{\"name\":\"Jam\",\"kind\":\"condiment\"}]}\n```";
        let s = parse_suggestions(reply).unwrap();
        let got: Vec<(&str, &str)> = s.iter().map(|x| (x.name.as_str(), x.kind.as_str())).collect();
        assert_eq!(got, vec![("Toast", "food"), ("Coffee", "drink"), ("Jam", "food")]);
    }

    #[test]
    fn tolerates_text_around_the_json() {
        let s = parse_suggestions("Here you go: {\"items\":[{\"name\":\"Tea\",\"kind\":\"drink\"}]} Enjoy").unwrap();
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn empty_and_garbage_replies() {
        assert!(parse_suggestions("{\"items\":[]}").unwrap().is_empty());
        assert!(parse_suggestions("I can't see any food").is_err());
    }
}
