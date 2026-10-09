//! Categories and flags on food items, and AI help keeping the food list tidy.
//!
//! An item has at most one category ("Dairy") and any number of flags ("Gluten",
//! "High-histamine"), both picked from lists the person edits on the Food page.
//! `foods.tag_source` records who set them: the model only tags items that have never
//! been tagged (NULL), so anything set or corrected by hand ('user') is never overwritten.
//!
//! Clean-ups (merging duplicates, fixing names, splitting "toast and jam") rewrite the log,
//! so the model only *suggests* them; each is applied when the person accepts it.

use crate::commands::{ai, settings};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;

#[derive(Debug, Serialize, FromRow)]
pub struct FoodTag {
    pub id: i64,
    pub name: String,
    /// Items carrying it — shown in the manager, and a reason to think before deleting.
    pub items: i64,
}

fn clean_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Name cannot be empty.".into());
    }
    Ok(n.to_string())
}

fn dup_err(e: sqlx::Error, name: &str) -> String {
    if e.to_string().contains("UNIQUE") {
        format!("'{}' already exists.", name)
    } else {
        e.to_string()
    }
}

// ── The two lists ──

#[tauri::command]
pub async fn list_food_categories(pool: State<'_, SqlitePool>) -> Result<Vec<FoodTag>, String> {
    sqlx::query_as::<_, FoodTag>(
        "SELECT c.id, c.name, CAST(COUNT(f.id) AS INTEGER) AS items \
         FROM food_categories c LEFT JOIN foods f ON f.category_id = c.id \
         GROUP BY c.id ORDER BY c.sort, c.name COLLATE NOCASE",
    )
    .fetch_all(&*pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_food_flags(pool: State<'_, SqlitePool>) -> Result<Vec<FoodTag>, String> {
    sqlx::query_as::<_, FoodTag>(
        "SELECT g.id, g.name, CAST(COUNT(i.food_id) AS INTEGER) AS items \
         FROM food_flags g LEFT JOIN food_item_flags i ON i.flag_id = g.id \
         GROUP BY g.id ORDER BY g.sort, g.name COLLATE NOCASE",
    )
    .fetch_all(&*pool).await.map_err(|e| e.to_string())
}

/// Add (`id` = None) or rename a category / flag. `table` is fixed by the caller.
async fn save_tag(pool: &SqlitePool, table: &str, id: Option<i64>, name: &str) -> Result<i64, String> {
    let name = clean_name(name)?;
    match id {
        Some(id) => sqlx::query(&format!("UPDATE {} SET name = ? WHERE id = ?", table))
            .bind(&name).bind(id)
            .execute(pool).await
            .map(|_| id)
            .map_err(|e| dup_err(e, &name)),
        None => sqlx::query(&format!(
            "INSERT INTO {t} (name, sort) VALUES (?, (SELECT COALESCE(MAX(sort), 0) + 1 FROM {t}))",
            t = table
        ))
        .bind(&name)
        .execute(pool).await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| dup_err(e, &name)),
    }
}

#[tauri::command]
pub async fn save_food_category(pool: State<'_, SqlitePool>, id: Option<i64>, name: String) -> Result<i64, String> {
    save_tag(&pool, "food_categories", id, &name).await
}

#[tauri::command]
pub async fn save_food_flag(pool: State<'_, SqlitePool>, id: Option<i64>, name: String) -> Result<i64, String> {
    save_tag(&pool, "food_flags", id, &name).await
}

/// Items in a deleted category become uncategorised (not re-tagged: their flags still stand).
#[tauri::command]
pub async fn delete_food_category(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE foods SET category_id = NULL WHERE category_id = ?")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM food_categories WHERE id = ?")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_food_flag(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM food_item_flags WHERE flag_id = ?")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM food_flags WHERE id = ?")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

// ── One item's tags ──

/// Set an item's category and flags by hand. Marks them 'user', so the model leaves them be.
#[tauri::command]
pub async fn set_food_tags(
    pool: State<'_, SqlitePool>,
    food_id: i64,
    category_id: Option<i64>,
    flag_ids: Vec<i64>,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE foods SET category_id = ?, tag_source = 'user' WHERE id = ?")
        .bind(category_id).bind(food_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM food_item_flags WHERE food_id = ?")
        .bind(food_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    for fid in flag_ids {
        sqlx::query("INSERT OR IGNORE INTO food_item_flags (food_id, flag_id) VALUES (?, ?)")
            .bind(food_id).bind(fid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

// ── AI tagging ──

#[derive(Deserialize)]
struct TagReply {
    #[serde(default)]
    items: Vec<TagItem>,
}

#[derive(Deserialize)]
struct TagItem {
    id: i64,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    flags: Vec<String>,
}

/// Items per model call — enough that a first run over the whole list is a few calls.
const TAG_BATCH: usize = 60;
static TAGGING: AtomicBool = AtomicBool::new(false);

struct TagBusy;
impl Drop for TagBusy {
    fn drop(&mut self) {
        TAGGING.store(false, Ordering::SeqCst);
    }
}

fn tag_prompt(categories: &[String], flags: &[String], items: &[(i64, String, String)]) -> String {
    let list = items
        .iter()
        .map(|(id, name, kind)| format!("{}: {} [{}]", id, name, kind))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"Categorise these items from one person's food and drink log.

Categories — pick exactly one per item, or null if none fits: {cats}
Flags — pick every one that applies to the item as it is usually made and eaten (none is fine): {flags}

Guidance:
- Judge the item as named. For a dish, flag its usual ingredients (e.g. lasagne: Gluten, Dairy).
- "Protein" = a meaningful protein source; "High-fiber" and "High-carb" = notably high, not just present.
- "High-histamine": aged, fermented, cured, smoked or leftover-prone foods, and the usual listed ones (tomato, spinach, avocado, vinegar, chocolate, citrus...). Only flag clear cases.
- Use the category and flag names exactly as written. If unsure about a flag, leave it off.

Items (id: name [kind]):
{list}

Return ONLY JSON, no commentary: {{"items":[{{"id":1,"category":"Dairy","flags":["Dairy","Protein"]}}]}}"#,
        cats = categories.join(", "),
        flags = flags.join(", "),
        list = list,
    )
}

fn parse_json<T: for<'de> Deserialize<'de>>(reply: &str) -> Result<T, String> {
    let body = ai::strip_code_fences(reply);
    let json = match (body.find('{'), body.rfind('}')) {
        (Some(a), Some(b)) if b > a => &body[a..=b],
        _ => return Err("The model didn't return JSON.".into()),
    };
    serde_json::from_str(json).map_err(|e| format!("The model's reply couldn't be read: {}", e))
}

/// Tag every item that has never been tagged. Returns how many were tagged. Without an
/// API key it does nothing (not an error: tagging is a nicety). Called after an item is
/// created, by "Tidy up" on the Food page, and before each weekly summary.
pub async fn tag_untagged(pool: &SqlitePool) -> Result<i64, String> {
    if !crate::commands::features::ai(|f| f.ai_food_tags) {
        return Ok(0);
    }
    let Some(api_key) = settings::get_api_key().await? else { return Ok(0) };
    if TAGGING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Ok(0); // a run is already going; it re-queries, so it will pick these up
    }
    let _busy = TagBusy;

    let cats: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM food_categories ORDER BY sort")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    let flags: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM food_flags ORDER BY sort")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    let cat_by_name: HashMap<String, i64> = cats.iter().map(|(id, n)| (n.to_lowercase(), *id)).collect();
    let flag_by_name: HashMap<String, i64> = flags.iter().map(|(id, n)| (n.to_lowercase(), *id)).collect();
    let cat_names: Vec<String> = cats.into_iter().map(|c| c.1).collect();
    let flag_names: Vec<String> = flags.into_iter().map(|f| f.1).collect();

    let mut tagged = 0i64;
    let mut seen: Vec<i64> = Vec::new();
    // Re-query each round, so items created while a call was out are picked up too.
    for _ in 0..20 {
        let items: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, name, kind FROM foods WHERE tag_source IS NULL ORDER BY id")
                .fetch_all(pool).await.map_err(|e| e.to_string())?;
        // Skip anything the model already declined to tag this run, or we'd loop on it.
        let batch: Vec<(i64, String, String)> =
            items.into_iter().filter(|i| !seen.contains(&i.0)).take(TAG_BATCH).collect();
        if batch.is_empty() {
            break;
        }
        seen.extend(batch.iter().map(|i| i.0));
        let reply = ai::call_openrouter(&api_key, &tag_prompt(&cat_names, &flag_names, &batch), 0.1, 6000).await?;
        let parsed: TagReply = parse_json(&reply)?;

        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        for item in parsed.items {
            if !batch.iter().any(|b| b.0 == item.id) {
                continue;
            }
            let category = item.category.and_then(|c| cat_by_name.get(&c.trim().to_lowercase()).copied());
            // Only an untagged row: someone may have set it by hand while the call was out.
            let done = sqlx::query("UPDATE foods SET category_id = ?, tag_source = 'ai' WHERE id = ? AND tag_source IS NULL")
                .bind(category).bind(item.id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            if done.rows_affected() == 0 {
                continue;
            }
            for f in item.flags {
                if let Some(fid) = flag_by_name.get(&f.trim().to_lowercase()) {
                    sqlx::query("INSERT OR IGNORE INTO food_item_flags (food_id, flag_id) VALUES (?, ?)")
                        .bind(item.id).bind(fid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
                }
            }
            tagged += 1;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
    }
    Ok(tagged)
}

#[tauri::command]
pub async fn tag_untagged_foods(pool: State<'_, SqlitePool>) -> Result<i64, String> {
    tag_untagged(&pool).await
}

/// Hand an item back to the model: clears its tags so the next tagging run redoes them.
#[tauri::command]
pub async fn retag_food(pool: State<'_, SqlitePool>, food_id: i64) -> Result<i64, String> {
    sqlx::query("DELETE FROM food_item_flags WHERE food_id = ?")
        .bind(food_id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE foods SET category_id = NULL, tag_source = NULL WHERE id = ?")
        .bind(food_id).execute(&*pool).await.map_err(|e| e.to_string())?;
    tag_untagged(&pool).await
}

// ── Clean-up suggestions ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CleanupSuggestion {
    /// "merge" (food_ids are the same thing; names[0] is the name to keep),
    /// "rename" (one id, names[0] is the better name) or
    /// "split" (one id that is really several items; names are the parts).
    pub action: String,
    pub food_ids: Vec<i64>,
    pub names: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Deserialize)]
struct CleanupReply {
    #[serde(default)]
    suggestions: Vec<CleanupSuggestion>,
}

const MAX_CLEANUPS: usize = 25;

fn cleanup_prompt(items: &[(i64, String, String, i64)]) -> String {
    let list = items
        .iter()
        .map(|(id, name, kind, days)| format!("{}: {} [{}, logged on {} days]", id, name, kind, days))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"Here is one person's list of logged food and drink items. Suggest clean-ups that would make it more consistent. Be conservative: only suggest a change you are confident about, and suggest nothing if the list is fine.

Kinds of suggestion:
- "merge": two or more ids that are clearly the same thing written differently (spelling, plural, capitalisation, "Coffee" vs "Coffee (black)"). names: [the single best name]. Do NOT merge things that are genuinely different (e.g. "Coffee" and "Decaf coffee", "Milk" and "Oat milk").
- "rename": one id with a typo or unclear name. names: [the corrected name].
- "split": one id that is really several separate items logged as one ("Toast and jam", "Eggs & bacon"). names: [each part]. Don't split a single dish ("Spaghetti bolognese", "Fish and chips").
Keep their wording and spelling conventions; reuse an existing item's exact name when a merge or split part matches it.

Items (id: name [kind, days]):
{list}

Return ONLY JSON: {{"suggestions":[{{"action":"merge","food_ids":[3,8],"names":["Flat white"],"reason":"same drink"}}]}}"#
    )
}

/// Ask the model for clean-ups. Nothing is changed here: each suggestion is shown and
/// applied with merge_foods / update_food / split_food when accepted.
#[tauri::command]
pub async fn suggest_food_cleanup(pool: State<'_, SqlitePool>) -> Result<Vec<CleanupSuggestion>, String> {
    let api_key = settings::get_api_key()
        .await?
        .ok_or_else(|| "Add an OpenRouter API key in Settings to use Tidy up.".to_string())?;
    let items: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT f.id, f.name, f.kind, CAST(COUNT(DISTINCT fl.log_date) AS INTEGER) \
         FROM foods f LEFT JOIN food_log fl ON fl.food_id = f.id \
         GROUP BY f.id ORDER BY f.name COLLATE NOCASE",
    )
    .fetch_all(&*pool).await.map_err(|e| e.to_string())?;
    if items.len() < 2 {
        return Ok(vec![]);
    }
    let reply = ai::call_openrouter(&api_key, &cleanup_prompt(&items), 0.1, 6000).await?;
    let parsed: CleanupReply = parse_json(&reply)?;
    Ok(validate_cleanups(parsed.suggestions, &items.iter().map(|i| i.0).collect::<Vec<_>>()))
}

/// Drop anything malformed or pointing at an id that isn't in the list.
fn validate_cleanups(suggestions: Vec<CleanupSuggestion>, known: &[i64]) -> Vec<CleanupSuggestion> {
    suggestions
        .into_iter()
        .filter_map(|mut s| {
            s.names = s.names.iter().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
            s.food_ids.dedup();
            let ok_ids = !s.food_ids.is_empty() && s.food_ids.iter().all(|id| known.contains(id));
            let ok = ok_ids
                && match s.action.as_str() {
                    "merge" => s.food_ids.len() >= 2 && !s.names.is_empty(),
                    "rename" => s.food_ids.len() == 1 && !s.names.is_empty(),
                    "split" => s.food_ids.len() == 1 && s.names.len() >= 2,
                    _ => false,
                };
            ok.then_some(s)
        })
        .take(MAX_CLEANUPS)
        .collect()
}

/// Fold `other_ids` into `keep_id`: their log entries and group memberships move across,
/// then they are deleted, and `keep_id` takes `name`. History is kept, just relabelled.
#[tauri::command]
pub async fn merge_foods(
    pool: State<'_, SqlitePool>,
    keep_id: i64,
    other_ids: Vec<i64>,
    name: String,
) -> Result<(), String> {
    merge(&pool, keep_id, &other_ids, &name).await
}

async fn merge(pool: &SqlitePool, keep_id: i64, other_ids: &[i64], name: &str) -> Result<(), String> {
    let name = clean_name(name)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for id in other_ids.iter().filter(|id| **id != keep_id) {
        sqlx::query("UPDATE food_log SET food_id = ? WHERE food_id = ?")
            .bind(keep_id).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT OR IGNORE INTO food_group_items (group_id, food_id) \
             SELECT group_id, ? FROM food_group_items WHERE food_id = ?",
        )
        .bind(keep_id).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        for sql in [
            "DELETE FROM food_group_items WHERE food_id = ?",
            "DELETE FROM food_item_flags WHERE food_id = ?",
            "DELETE FROM foods WHERE id = ?",
        ] {
            sqlx::query(sql).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
    }
    sqlx::query("UPDATE foods SET name = ?, active = 1 WHERE id = ?")
        .bind(&name).bind(keep_id)
        .execute(&mut *tx).await.map_err(|e| dup_err(e, &name))?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Replace one item with several: each of its log entries becomes one entry per part (same
/// day, time, amount and group), group memberships likewise. Parts that already exist are
/// reused; new ones are created untagged, for the next tagging run.
#[tauri::command]
pub async fn split_food(pool: State<'_, SqlitePool>, id: i64, names: Vec<String>) -> Result<(), String> {
    split(&pool, id, &names).await
}

async fn split(pool: &SqlitePool, id: i64, names: &[String]) -> Result<(), String> {
    let names: Vec<String> = names.iter().map(|n| clean_name(n)).collect::<Result<_, _>>()?;
    if names.len() < 2 {
        return Err("A split needs at least two parts.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let (kind, regular): (String, bool) = sqlx::query_as("SELECT kind, regular FROM foods WHERE id = ?")
        .bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    let mut part_ids = Vec::new();
    for n in &names {
        let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM foods WHERE name = ? AND id != ?")
            .bind(n).bind(id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
        let pid = match existing {
            Some((pid,)) => pid,
            None => sqlx::query("INSERT INTO foods (name, kind, regular) VALUES (?, ?, ?)")
                .bind(n).bind(&kind).bind(regular)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?
                .last_insert_rowid(),
        };
        if !part_ids.contains(&pid) {
            part_ids.push(pid);
        }
    }
    for pid in &part_ids {
        sqlx::query(
            "INSERT INTO food_log (log_date, time_taken, food_id, amount, group_id, created_at) \
             SELECT log_date, time_taken, ?, amount, group_id, created_at FROM food_log WHERE food_id = ?",
        )
        .bind(pid).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT OR IGNORE INTO food_group_items (group_id, food_id) \
             SELECT group_id, ? FROM food_group_items WHERE food_id = ?",
        )
        .bind(pid).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    for sql in [
        "DELETE FROM food_log WHERE food_id = ?",
        "DELETE FROM food_group_items WHERE food_id = ?",
        "DELETE FROM food_item_flags WHERE food_id = ?",
        "DELETE FROM foods WHERE id = ?",
    ] {
        sqlx::query(sql).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        for sql in [
            "INSERT INTO foods (id, name) VALUES (1, 'Coffee'), (2, 'coffee black'), (3, 'Toast and jam'), (4, 'Jam')",
            "INSERT INTO food_groups (id, name) VALUES (1, 'Breakfast')",
            "INSERT INTO food_group_items VALUES (1, 2), (1, 3)",
            "INSERT INTO food_log (log_date, time_taken, food_id, amount) VALUES
               ('2026-10-01', '08:00', 1, NULL), ('2026-10-02', '08:00', 2, 'large'),
               ('2026-10-01', '08:00', 3, NULL), ('2026-10-02', NULL, 3, '2 slices')",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn rows(pool: &SqlitePool, sql: &str) -> Vec<(String,)> {
        sqlx::query_as(sql).fetch_all(pool).await.unwrap()
    }

    #[tokio::test]
    async fn merge_moves_history_and_groups_onto_the_kept_item() {
        let pool = pool().await;
        merge(&pool, 1, &[2], "Coffee").await.unwrap();
        let log = rows(&pool, "SELECT log_date || '|' || COALESCE(amount, '') FROM food_log WHERE food_id = 1 ORDER BY log_date").await;
        assert_eq!(log, vec![("2026-10-01|".into(),), ("2026-10-02|large".into(),)]);
        assert!(rows(&pool, "SELECT name FROM foods WHERE id = 2").await.is_empty());
        assert_eq!(rows(&pool, "SELECT CAST(food_id AS TEXT) FROM food_group_items WHERE food_id IN (1, 2)").await, vec![("1".into(),)]);
    }

    #[tokio::test]
    async fn split_copies_each_entry_to_every_part_and_reuses_existing_items() {
        let pool = pool().await;
        split(&pool, 3, &["Toast".to_string(), "jam".to_string()]).await.unwrap();
        // "jam" matches the existing "Jam" (NOCASE); "Toast" is new and untagged.
        let log = rows(&pool, "SELECT f.name || '|' || fl.log_date || '|' || COALESCE(fl.amount, '') FROM food_log fl JOIN foods f ON f.id = fl.food_id WHERE fl.food_id != 1 AND fl.food_id != 2 ORDER BY 1").await;
        let log: Vec<String> = log.into_iter().map(|r| r.0).collect();
        assert_eq!(log, vec!["Jam|2026-10-01|", "Jam|2026-10-02|2 slices", "Toast|2026-10-01|", "Toast|2026-10-02|2 slices"]);
        assert!(rows(&pool, "SELECT name FROM foods WHERE id = 3").await.is_empty());
        assert_eq!(rows(&pool, "SELECT COUNT(*) || '' FROM food_group_items WHERE group_id = 1").await, vec![("3".into(),)]);
        assert_eq!(rows(&pool, "SELECT COUNT(*) || '' FROM foods WHERE name = 'Toast' AND tag_source IS NULL").await, vec![("1".into(),)]);
    }

    fn s(action: &str, ids: &[i64], names: &[&str]) -> CleanupSuggestion {
        CleanupSuggestion {
            action: action.into(),
            food_ids: ids.to_vec(),
            names: names.iter().map(|n| n.to_string()).collect(),
            reason: String::new(),
        }
    }

    #[test]
    fn keeps_only_well_formed_cleanups() {
        let got = validate_cleanups(
            vec![
                s("merge", &[1, 2], &["Coffee"]),
                s("merge", &[1], &["Coffee"]),        // nothing to merge with
                s("rename", &[3], &[" Banana "]),
                s("split", &[4], &["Toast"]),         // one part isn't a split
                s("split", &[4], &["Toast", "Jam"]),
                s("rename", &[99], &["Ghost"]),       // unknown id
                s("delete", &[1], &["x"]),            // not an action
            ],
            &[1, 2, 3, 4],
        );
        let kinds: Vec<&str> = got.iter().map(|c| c.action.as_str()).collect();
        assert_eq!(kinds, vec!["merge", "rename", "split"]);
        assert_eq!(got[1].names, vec!["Banana"]);
    }

    #[test]
    fn reads_tags_from_a_fenced_reply() {
        let r: TagReply = parse_json("```json\n{\"items\":[{\"id\":4,\"category\":\"Dairy\",\"flags\":[\"Dairy\"]},{\"id\":5,\"category\":null}]}\n```").unwrap();
        assert_eq!(r.items.len(), 2);
        assert!(r.items[1].flags.is_empty() && r.items[1].category.is_none());
    }
}
