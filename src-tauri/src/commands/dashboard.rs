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