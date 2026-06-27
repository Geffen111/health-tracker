// AI health insights: aggregate the log over a date range into small summary
// figures (no row-level data leaves the machine), ask the model to interpret
// them, and cache the result keyed by period + a hash of the aggregates so an
// unchanged period is served from cache until "Refresh" is pressed.

use crate::commands::ai::{call_openrouter, strip_code_fences};
use crate::models::HealthInsights;
use chrono::{Datelike, Months, NaiveDate};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri::State;

fn default_start_date() -> String {
    let today = chrono::Local::now().naive_local().date();
    let three_months_ago = today - Months::new(3);
    format!("{:04}-{:02}-01", three_months_ago.year(), three_months_ago.month())
}

fn default_end_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn resolve_dates(start: Option<String>, end: Option<String>) -> (String, String) {
    (
        start.unwrap_or_else(default_start_date),
        end.unwrap_or_else(default_end_date),
    )
}

#[tauri::command]
pub async fn get_insights(
    pool: State<'_, SqlitePool>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<HealthInsights, String> {
    let api_key = require_key().await?;
    let (sd, ed) = resolve_dates(start_date, end_date);
    fetch_health_insights(&pool, &api_key, &sd, &ed, false).await
}

#[tauri::command]
pub async fn refresh_insights(
    pool: State<'_, SqlitePool>,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<HealthInsights, String> {
    let api_key = require_key().await?;
    let (sd, ed) = resolve_dates(start_date, end_date);
    fetch_health_insights(&pool, &api_key, &sd, &ed, true).await
}

async fn require_key() -> Result<String, String> {
    crate::commands::settings::get_api_key()
        .await?
        .ok_or_else(|| "OpenRouter API key not configured. Please add your API key in Settings.".to_string())
}

// ── Aggregation ──

struct Aggregates {
    days_logged: i64,
    avg_fatigue: Option<f64>,
    avg_headache: Option<f64>,
    avg_sleep: Option<f64>,
    avg_steps: Option<f64>,
    avg_resting_hr: Option<f64>,
    sick_leave_hours: f64,
    pem_days: i64,
    crash_days: i64,
    avg_risk: Option<f64>,
    high_band: i64,
    medium_band: i64,
    low_band: i64,
    activity_by_category: Vec<(String, f64)>,
    worst_days: Vec<(String, f64)>,
    fatigue_first_half: Option<f64>,
    fatigue_second_half: Option<f64>,
    sleep_first_half: Option<f64>,
    sleep_second_half: Option<f64>,
}

async fn aggregate(pool: &SqlitePool, start: &str, end: &str) -> Result<Aggregates, String> {
    let stats: (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, f64) =
        sqlx::query_as(
            "SELECT COUNT(*),
                    CAST(AVG(fatigue_rating) AS REAL),
                    CAST(AVG(headache_rating) AS REAL),
                    CAST(AVG(sleep_avg) AS REAL),
                    CAST(AVG(steps) AS REAL),
                    CAST(AVG(ave_resting_hr) AS REAL),
                    CAST(COALESCE(SUM(sick_leave_hours), 0) AS REAL)
             FROM daily_logs WHERE log_date >= ? AND log_date <= ?",
        )
        .bind(start).bind(end)
        .fetch_one(pool).await.map_err(|e| format!("DB error stats: {}", e))?;

    let pem: (i64, i64, Option<f64>, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*),
                CAST(COALESCE(SUM(CASE WHEN crash_flag = 1 THEN 1 ELSE 0 END), 0) AS INTEGER),
                CAST(AVG(predicted_pem_risk) AS REAL),
                CAST(COALESCE(SUM(CASE WHEN risk_band = 'High' THEN 1 ELSE 0 END), 0) AS INTEGER),
                CAST(COALESCE(SUM(CASE WHEN risk_band = 'Medium' THEN 1 ELSE 0 END), 0) AS INTEGER),
                CAST(COALESCE(SUM(CASE WHEN risk_band = 'Low' THEN 1 ELSE 0 END), 0) AS INTEGER)
         FROM pem_predictions WHERE log_date >= ? AND log_date <= ?",
    )
    .bind(start).bind(end)
    .fetch_one(pool).await.map_err(|e| format!("DB error pem: {}", e))?;

    let activity_by_category: Vec<(String, f64)> = sqlx::query_as(
        "SELECT ac.name, CAST(COALESCE(SUM(al.duration_hours), 0) AS REAL) AS hours
         FROM activity_log al
         JOIN activity_types at ON al.activity_type_id = at.id
         JOIN activity_categories ac ON at.category_id = ac.id
         WHERE al.log_date >= ? AND al.log_date <= ?
         GROUP BY ac.name ORDER BY hours DESC",
    )
    .bind(start).bind(end)
    .fetch_all(pool).await.map_err(|e| format!("DB error activity: {}", e))?;

    let worst_days: Vec<(String, f64)> = sqlx::query_as(
        "SELECT log_date, CAST(fatigue_rating AS REAL)
         FROM daily_logs
         WHERE fatigue_rating IS NOT NULL AND log_date >= ? AND log_date <= ?
         ORDER BY fatigue_rating DESC, log_date DESC LIMIT 5",
    )
    .bind(start).bind(end)
    .fetch_all(pool).await.map_err(|e| format!("DB error worst days: {}", e))?;

    // First-half vs second-half trend for fatigue and sleep.
    let mid = midpoint(start, end);
    let (fatigue_first_half, sleep_first_half) = half_averages(pool, start, &mid, false).await?;
    let (fatigue_second_half, sleep_second_half) = half_averages(pool, &mid, end, true).await?;

    Ok(Aggregates {
        days_logged: stats.0,
        avg_fatigue: stats.1,
        avg_headache: stats.2,
        avg_sleep: stats.3,
        avg_steps: stats.4,
        avg_resting_hr: stats.5,
        sick_leave_hours: stats.6,
        pem_days: pem.0,
        crash_days: pem.1,
        avg_risk: pem.2,
        high_band: pem.3,
        medium_band: pem.4,
        low_band: pem.5,
        activity_by_category,
        worst_days,
        fatigue_first_half,
        fatigue_second_half,
        sleep_first_half,
        sleep_second_half,
    })
}

/// Midpoint date between start and end (inclusive); falls back to start on parse error.
fn midpoint(start: &str, end: &str) -> String {
    match (
        NaiveDate::parse_from_str(start, "%Y-%m-%d"),
        NaiveDate::parse_from_str(end, "%Y-%m-%d"),
    ) {
        (Ok(s), Ok(e)) if e > s => (s + chrono::Duration::days((e - s).num_days() / 2))
            .format("%Y-%m-%d")
            .to_string(),
        _ => start.to_string(),
    }
}

/// Avg fatigue & sleep over [lo, hi). When `inclusive_end`, the upper bound is <=.
async fn half_averages(
    pool: &SqlitePool,
    lo: &str,
    hi: &str,
    inclusive_end: bool,
) -> Result<(Option<f64>, Option<f64>), String> {
    let sql = if inclusive_end {
        "SELECT CAST(AVG(fatigue_rating) AS REAL), CAST(AVG(sleep_avg) AS REAL)
         FROM daily_logs WHERE log_date >= ? AND log_date <= ?"
    } else {
        "SELECT CAST(AVG(fatigue_rating) AS REAL), CAST(AVG(sleep_avg) AS REAL)
         FROM daily_logs WHERE log_date >= ? AND log_date < ?"
    };
    let row: (Option<f64>, Option<f64>) = sqlx::query_as(sql)
        .bind(lo).bind(hi)
        .fetch_one(pool).await.map_err(|e| format!("DB error trend: {}", e))?;
    Ok(row)
}

// ── Orchestration + cache ──

async fn fetch_health_insights(
    pool: &SqlitePool,
    api_key: &str,
    start: &str,
    end: &str,
    force_refresh: bool,
) -> Result<HealthInsights, String> {
    let agg = aggregate(pool, start, end).await?;
    let prompt_data = render_for_prompt(&agg);
    let data_hash = {
        let mut hasher = Sha256::new();
        hasher.update(prompt_data.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    let period_label = format!("{} to {}", start, end);

    if !force_refresh {
        if let Some((data, generated_at)) = check_cache(pool, start, end, &data_hash).await? {
            let mut insights: HealthInsights = serde_json::from_str(&data)
                .map_err(|e| format!("Failed to parse cached insights: {}", e))?;
            insights.generated_at = generated_at;
            insights.period_label = period_label;
            return Ok(insights);
        }
    }

    if agg.days_logged == 0 {
        return Err("No log data in this period to analyse.".to_string());
    }

    let mut insights = call_model(api_key, start, end, &prompt_data).await?;
    insights.period_label = period_label;
    insights.generated_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    save_cache(pool, start, end, &data_hash, &insights).await?;
    Ok(insights)
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|x| format!("{:.1}", x)).unwrap_or_else(|| "n/a".to_string())
}

fn render_for_prompt(a: &Aggregates) -> String {
    let activities = if a.activity_by_category.is_empty() {
        "(none logged)".to_string()
    } else {
        a.activity_by_category
            .iter()
            .map(|(name, hours)| format!("{}: {:.1}h", name, hours))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let worst = if a.worst_days.is_empty() {
        "(none)".to_string()
    } else {
        a.worst_days
            .iter()
            .map(|(d, f)| format!("{} ({:.0}/10)", d, f))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "Days logged: {days}
Average fatigue: {fatigue}/10 (first half {f1} -> second half {f2})
Average headache: {headache}/10
Average sleep: {sleep}h (first half {s1}h -> second half {s2}h)
Average steps/day: {steps}
Average resting HR: {hr} bpm
Sick-leave hours (total): {sick:.1}
PEM days modelled: {pemdays}; crash days flagged: {crashes}; average predicted risk: {risk}/10
Risk bands: High {high}, Medium {med}, Low {low}
Activity hours by category: {activities}
Worst fatigue days: {worst}",
        days = a.days_logged,
        fatigue = fmt_opt(a.avg_fatigue),
        f1 = fmt_opt(a.fatigue_first_half),
        f2 = fmt_opt(a.fatigue_second_half),
        headache = fmt_opt(a.avg_headache),
        sleep = fmt_opt(a.avg_sleep),
        s1 = fmt_opt(a.sleep_first_half),
        s2 = fmt_opt(a.sleep_second_half),
        steps = fmt_opt(a.avg_steps),
        hr = fmt_opt(a.avg_resting_hr),
        sick = a.sick_leave_hours,
        pemdays = a.pem_days,
        crashes = a.crash_days,
        risk = fmt_opt(a.avg_risk),
        high = a.high_band,
        med = a.medium_band,
        low = a.low_band,
        activities = activities,
        worst = worst,
    )
}

async fn call_model(
    api_key: &str,
    start: &str,
    end: &str,
    data: &str,
) -> Result<HealthInsights, String> {
    let prompt = format!(
        r#"You are an analyst helping someone understand their ME/CFS (chronic fatigue) health log.
Higher fatigue/headache ratings are worse. PEM = post-exertional malaise (a crash after overexertion).

Period: {start} to {end}

Aggregated data:
{data}

Analyse the data and surface concrete, data-grounded observations (e.g. links between activity load,
sleep, and fatigue/crashes; trends across the two halves; notable outliers). Be supportive and
practical. Describe what the data shows — do NOT give medical diagnoses or prescribe treatment;
frame suggestions as gentle, pacing-oriented experiments.

Respond with JSON only, no markdown. severity is one of "positive", "warning", "critical". icon is a single emoji.
{{
  "summary": "One or two sentence overview of this period",
  "patterns": [
    {{"title": "Short pattern title", "detail": "What the data shows, with figures.", "severity": "warning", "icon": "📈"}}
  ],
  "anomalies": [
    {{"title": "Notable outlier", "detail": "...", "severity": "warning", "icon": "⚠️"}}
  ],
  "recommendations": [
    {{"title": "Gentle suggestion", "detail": "A pacing-oriented experiment to try.", "severity": "positive", "icon": "💡"}}
  ]
}}"#,
        start = start,
        end = end,
        data = data,
    );

    let content = call_openrouter(api_key, &prompt, 0.3, 4096).await?;
    let cleaned = strip_code_fences(&content);
    serde_json::from_str(cleaned)
        .map_err(|e| format!("Failed to parse AI response: {} - {}", e, cleaned))
}

async fn check_cache(
    pool: &SqlitePool,
    start: &str,
    end: &str,
    data_hash: &str,
) -> Result<Option<(String, String)>, String> {
    sqlx::query_as(
        "SELECT insight_data, generated_at FROM ai_insights
         WHERE period_start = ? AND period_end = ? AND data_hash = ?
         ORDER BY generated_at DESC LIMIT 1",
    )
    .bind(start).bind(end).bind(data_hash)
    .fetch_optional(pool).await.map_err(|e| format!("DB error check cache: {}", e))
}

async fn save_cache(
    pool: &SqlitePool,
    start: &str,
    end: &str,
    data_hash: &str,
    insights: &HealthInsights,
) -> Result<(), String> {
    let data = serde_json::to_string(insights)
        .map_err(|e| format!("Failed to serialize insights: {}", e))?;
    sqlx::query(
        "INSERT INTO ai_insights (insight_type, insight_data, period_start, period_end, data_hash)
         VALUES ('health_analysis', ?, ?, ?, ?)",
    )
    .bind(&data).bind(start).bind(end).bind(data_hash)
    .execute(pool).await.map_err(|e| format!("DB error save cache: {}", e))?;
    Ok(())
}
