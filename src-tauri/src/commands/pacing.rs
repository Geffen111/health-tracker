//! Pacing — descriptive activity/exertion history.
//!
//! This module replaces the old PEM risk model (`commands/pem.rs`), which predicted a
//! next-day fatigue score and a Low/Medium/High risk band. Migration
//! 20240622_retire_pem_predictions.sql records the measurements that retired it: over
//! 102 consecutive-day pairs the prediction scored RMSE 1.88 against actual next-day
//! fatigue while a constant scored 1.90, and every exertion input was uncorrelated with
//! the next day (|r| <= 0.10). Nothing here predicts forward — these commands only
//! report what was logged.
//!
//! "Load" is duration x the activity category's energy weight x the entry's energy-cost
//! factor, matching `src/lib/load.ts` on the frontend so the same day reads the same on
//! every screen.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

/// Load = duration x category energy weight x energy-cost factor. The cost falls back to
/// the activity type's default when an entry has none (older rows predate the auto-fill).
const LOAD_EXPR: &str = "al.duration_hours * COALESCE(ac.energy_weight, 1.0) * \
     CASE COALESCE(al.energy_cost, at.default_energy_cost) \
       WHEN 'Low' THEN 0.7 WHEN 'High' THEN 2.0 ELSE 1.0 END";

/// Which of the three load buckets a category feeds. Stored per category and edited on the
/// Activity page (migration 20240623); previously guessed from substrings of the name, in two
/// places that had to be kept in step by hand. Mirrors `computeDayLoad` in `src/lib/load.ts`.
const BUCKET_EXPR: &str = "ac.load_group";

/// One day's logged activity, rolled up. Purely descriptive.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DailyLoad {
    pub log_date: String,
    pub physical_load: f64,
    pub cognitive_load: f64,
    pub sensory_social_load: f64,
    pub total_load: f64,
    pub total_hours: f64,
    pub high_energy_hours: f64,
}

/// One (day, activity type) pair — the grain the activity chart buckets into weeks or
/// months, and groups by either category or individual activity.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ActivityHistoryRow {
    pub log_date: String,
    pub category: String,
    pub activity_type: String,
    pub energy_cost: Option<String>,
    pub hours: f64,
    pub load: f64,
}

/// Per-day activity load for every day with logged activity, oldest first.
/// `from` is an inclusive `YYYY-MM-DD` bound; omit it for the full history.
#[tauri::command]
pub async fn get_daily_loads(
    pool: State<'_, SqlitePool>,
    from: Option<String>,
) -> Result<Vec<DailyLoad>, String> {
    let sql = format!(
        "SELECT al.log_date, \
           CAST(COALESCE(SUM(CASE WHEN {bucket} = 'physical'  THEN {load} END), 0) AS REAL) AS physical_load, \
           CAST(COALESCE(SUM(CASE WHEN {bucket} = 'cognitive' THEN {load} END), 0) AS REAL) AS cognitive_load, \
           CAST(COALESCE(SUM(CASE WHEN {bucket} = 'sensory'   THEN {load} END), 0) AS REAL) AS sensory_social_load, \
           CAST(COALESCE(SUM({load}), 0) AS REAL) AS total_load, \
           CAST(COALESCE(SUM(al.duration_hours), 0) AS REAL) AS total_hours, \
           CAST(COALESCE(SUM(CASE WHEN COALESCE(al.energy_cost, at.default_energy_cost) = 'High' \
                THEN al.duration_hours END), 0) AS REAL) AS high_energy_hours \
         FROM activity_log al \
         JOIN activity_types at ON al.activity_type_id = at.id \
         JOIN activity_categories ac ON at.category_id = ac.id \
         WHERE al.log_date >= ? \
         GROUP BY al.log_date ORDER BY al.log_date",
        bucket = BUCKET_EXPR,
        load = LOAD_EXPR
    );
    sqlx::query_as::<_, DailyLoad>(&sql)
        .bind(from.unwrap_or_default())
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

/// Every logged activity by day and type, oldest first — the feed for the activity
/// chart. ~900 rows over the full history, so it is fetched whole and bucketed
/// client-side rather than paged.
#[tauri::command]
pub async fn get_activity_history(
    pool: State<'_, SqlitePool>,
    from: Option<String>,
) -> Result<Vec<ActivityHistoryRow>, String> {
    let sql = format!(
        "SELECT al.log_date, ac.name AS category, at.name AS activity_type, \
           COALESCE(al.energy_cost, at.default_energy_cost) AS energy_cost, \
           CAST(SUM(al.duration_hours) AS REAL) AS hours, \
           CAST(SUM({load}) AS REAL) AS load \
         FROM activity_log al \
         JOIN activity_types at ON al.activity_type_id = at.id \
         JOIN activity_categories ac ON at.category_id = ac.id \
         WHERE al.log_date >= ? \
         GROUP BY al.log_date, ac.name, at.name, COALESCE(al.energy_cost, at.default_energy_cost) \
         ORDER BY al.log_date",
        load = LOAD_EXPR
    );
    sqlx::query_as::<_, ActivityHistoryRow>(&sql)
        .bind(from.unwrap_or_default())
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}
