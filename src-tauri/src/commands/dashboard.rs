use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub date_count: i64,
    pub avg_fatigue: Option<f64>,
    pub avg_sleep: Option<f64>,
    pub avg_steps: Option<f64>,
    pub avg_resting_hr: Option<f64>,
    pub fatigue_last_7d: Option<f64>,
    pub fatigue_last_30d: Option<f64>,
    pub sleep_last_7d: Option<f64>,
    pub sleep_last_30d: Option<f64>,
    pub steps_last_7d: Option<f64>,
    pub steps_last_30d: Option<f64>,
    pub current_risk_band: Option<String>,
    pub crash_count_30d: i64,
    pub sick_leave_total: Option<f64>,
    pub headache_days_30d: i64,
}

#[tauri::command]
pub async fn get_dashboard_summary(pool: State<'_, SqlitePool>) -> Result<DashboardSummary, String> {
    // Use a transaction to get all stats in one go
    let date_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_logs")
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?;

    let avg_fatigue: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(fatigue_rating) FROM daily_logs WHERE fatigue_rating IS NOT NULL")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let avg_sleep: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating)) FROM daily_logs WHERE COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating) IS NOT NULL")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let avg_steps: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(steps) FROM daily_logs WHERE steps IS NOT NULL")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let avg_hr: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(ave_resting_hr) FROM daily_logs WHERE ave_resting_hr IS NOT NULL")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let f7: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(fatigue_rating) FROM daily_logs WHERE fatigue_rating IS NOT NULL AND log_date >= date('now', '-7 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let f30: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(fatigue_rating) FROM daily_logs WHERE fatigue_rating IS NOT NULL AND log_date >= date('now', '-30 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let s7: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating)) FROM daily_logs WHERE COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating) IS NOT NULL AND log_date >= date('now', '-7 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let s30: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating)) FROM daily_logs WHERE COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating) IS NOT NULL AND log_date >= date('now', '-30 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let st7: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(steps) FROM daily_logs WHERE steps IS NOT NULL AND log_date >= date('now', '-7 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let st30: Option<(Option<f64>,)> = sqlx::query_as("SELECT AVG(steps) FROM daily_logs WHERE steps IS NOT NULL AND log_date >= date('now', '-30 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let risk: Option<(Option<String>,)> = sqlx::query_as("SELECT risk_band FROM pem_predictions WHERE log_date = date('now')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let crashes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pem_predictions WHERE crash_flag = 1 AND log_date >= date('now', '-30 days')")
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?;

    let sick: Option<(Option<f64>,)> = sqlx::query_as("SELECT SUM(sick_leave_hours) FROM daily_logs WHERE sick_leave_hours IS NOT NULL AND log_date >= date('now', '-30 days')")
        .fetch_optional(&*pool).await.map_err(|e| e.to_string())?;

    let headaches: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM daily_logs WHERE headache_rating IS NOT NULL AND headache_rating > 0 AND log_date >= date('now', '-30 days')")
        .fetch_one(&*pool).await.map_err(|e| e.to_string())?;

    Ok(DashboardSummary {
        date_count: date_count.0,
        avg_fatigue: avg_fatigue.and_then(|r| r.0),
        avg_sleep: avg_sleep.and_then(|r| r.0),
        avg_steps: avg_steps.and_then(|r| r.0),
        avg_resting_hr: avg_hr.and_then(|r| r.0),
        fatigue_last_7d: f7.and_then(|r| r.0),
        fatigue_last_30d: f30.and_then(|r| r.0),
        sleep_last_7d: s7.and_then(|r| r.0),
        sleep_last_30d: s30.and_then(|r| r.0),
        steps_last_7d: st7.and_then(|r| r.0),
        steps_last_30d: st30.and_then(|r| r.0),
        current_risk_band: risk.and_then(|r| r.0),
        crash_count_30d: crashes.0,
        sick_leave_total: sick.and_then(|r| r.0),
        headache_days_30d: headaches.0,
    })
}

/// One row of the dashboard "Rolling averages" table: a metric with its value over
/// each trailing window. `dp` is the decimal places the UI should render with.
#[derive(Debug, Serialize)]
pub struct RollingMetric {
    pub key: String,
    pub label: String,
    pub unit: String,
    pub dp: i64,
    pub d7: Option<f64>,
    pub d30: Option<f64>,
    pub d60: Option<f64>,
    pub d90: Option<f64>,
}

/// Aggregates for a single trailing window, pulled from daily_logs + blood_pressure.
#[derive(Debug, Default, Clone)]
struct WindowAgg {
    sleep: Option<f64>,
    fatigue: Option<f64>,
    headache_rating: Option<f64>,
    headache_days: Option<f64>,
    sick_hours: Option<f64>,
    rostered_hours: Option<f64>,
    alcohol: Option<f64>,
    steps: Option<f64>,
    resting_hr: Option<f64>,
    avg_hr: Option<f64>,
    calories: Option<f64>,
    bp_sys: Option<f64>,
    bp_dia: Option<f64>,
}

async fn window_agg(pool: &SqlitePool, offset: &str) -> Result<WindowAgg, String> {
    // One pass over daily_logs for every daily-sourced metric in the window.
    let row: (
        Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>,
        Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>,
    ) = sqlx::query_as(
        "SELECT \
         AVG(COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating)), \
         AVG(fatigue_rating), \
         AVG(headache_rating), \
         SUM(CASE WHEN headache_rating > 0 THEN 1 ELSE 0 END), \
         SUM(sick_leave_hours), \
         SUM(rostered_hours), \
         SUM(alcohol_std_drinks), \
         AVG(steps), \
         AVG(ave_resting_hr), \
         AVG(ave_hr), \
         AVG(activity_calories) \
         FROM daily_logs WHERE log_date >= date('now', ?)",
    )
    .bind(offset)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Blood pressure lives in its own per-reading table.
    let bp: (Option<f64>, Option<f64>) = sqlx::query_as(
        "SELECT AVG(systolic), AVG(diastolic) FROM blood_pressure WHERE log_date >= date('now', ?)",
    )
    .bind(offset)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(WindowAgg {
        sleep: row.0,
        fatigue: row.1,
        headache_rating: row.2,
        headache_days: row.3,
        sick_hours: row.4,
        rostered_hours: row.5,
        alcohol: row.6,
        steps: row.7,
        resting_hr: row.8,
        avg_hr: row.9,
        calories: row.10,
        bp_sys: bp.0,
        bp_dia: bp.1,
    })
}

/// Trailing 7/30/60/90-day rollups for the dashboard table (mirrors the spreadsheet's
/// "Rolling averages" block).
#[tauri::command]
pub async fn get_rolling_averages(pool: State<'_, SqlitePool>) -> Result<Vec<RollingMetric>, String> {
    let w7 = window_agg(&pool, "-7 days").await?;
    let w30 = window_agg(&pool, "-30 days").await?;
    let w60 = window_agg(&pool, "-60 days").await?;
    let w90 = window_agg(&pool, "-90 days").await?;
    let windows = [&w7, &w30, &w60, &w90];

    // Each metric names its per-window extractor once; we then fan it across the 4 windows.
    let metric = |key: &str, label: &str, unit: &str, dp: i64, f: &dyn Fn(&WindowAgg) -> Option<f64>| -> RollingMetric {
        RollingMetric {
            key: key.to_string(),
            label: label.to_string(),
            unit: unit.to_string(),
            dp,
            d7: f(windows[0]),
            d30: f(windows[1]),
            d60: f(windows[2]),
            d90: f(windows[3]),
        }
    };

    let sick_pct = |w: &WindowAgg| match (w.sick_hours, w.rostered_hours) {
        (Some(s), Some(r)) if r > 0.0 => Some(s / r * 100.0),
        _ => None,
    };
    let steps_k = |w: &WindowAgg| w.steps.map(|s| s / 1000.0);

    Ok(vec![
        metric("sleep", "Sleep (avg)", "/10", 1, &|w| w.sleep),
        metric("fatigue", "Fatigue rating (avg)", "/10", 1, &|w| w.fatigue),
        metric("headache_rating", "Headache rating (avg)", "/10", 1, &|w| w.headache_rating),
        metric("headache_days", "Headache days (total)", "", 0, &|w| w.headache_days),
        metric("sick_hours", "Sick leave (hours)", "h", 1, &|w| w.sick_hours),
        metric("sick_pct", "Sick leave (% of rostered)", "%", 1, &sick_pct),
        metric("alcohol", "Alcohol (approx std drinks)", "", 1, &|w| w.alcohol),
        metric("steps_k", "Steps (thousands, avg)", "k", 1, &steps_k),
        metric("resting_hr", "Resting Heart Rate (avg)", "bpm", 1, &|w| w.resting_hr),
        metric("avg_hr", "Heart Rate (avg)", "bpm", 1, &|w| w.avg_hr),
        metric("calories", "Activity calories (avg)", "kcal", 1, &|w| w.calories),
        metric("bp_sys", "Blood Pressure Sys (avg)", "", 1, &|w| w.bp_sys),
        metric("bp_dia", "Blood Pressure Dia (avg)", "", 1, &|w| w.bp_dia),
    ])
}