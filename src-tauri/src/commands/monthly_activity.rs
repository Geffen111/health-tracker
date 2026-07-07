use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use tauri::State;

/// One calendar month's daily-average steps & active calories.
#[derive(Debug, Serialize)]
pub struct MonthlyActivityRow {
    pub year: i64,
    pub month: i64,
    pub steps_avg: Option<f64>,
    pub calories_avg: Option<f64>,
    /// True when the value is computed live from daily_logs (no stored/seeded row).
    /// The dashboard shows these as editable-but-derived; editing writes a stored row.
    pub computed: bool,
}

/// Merged monthly series: seeded/manual rows from `monthly_activity` take precedence,
/// and any month that has daily_logs data but no stored row is computed on the fly.
#[tauri::command]
pub async fn get_monthly_activity(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<MonthlyActivityRow>, String> {
    // Computed monthly daily-averages from the app's own daily logs.
    let computed: Vec<(String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT substr(log_date, 1, 7) AS ym, AVG(steps) AS s, AVG(activity_calories) AS c \
         FROM daily_logs GROUP BY ym",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut map: BTreeMap<(i64, i64), MonthlyActivityRow> = BTreeMap::new();
    for (ym, s, c) in computed {
        // ym is "YYYY-MM"
        let (Some(y), Some(m)) = (
            ym.get(0..4).and_then(|v| v.parse::<i64>().ok()),
            ym.get(5..7).and_then(|v| v.parse::<i64>().ok()),
        ) else {
            continue;
        };
        map.insert(
            (y, m),
            MonthlyActivityRow { year: y, month: m, steps_avg: s, calories_avg: c, computed: true },
        );
    }

    // Stored (seeded or manually-overridden) rows win over the computed value.
    let stored: Vec<(i64, i64, Option<f64>, Option<f64>)> =
        sqlx::query_as("SELECT year, month, steps_avg, calories_avg FROM monthly_activity")
            .fetch_all(&*pool)
            .await
            .map_err(|e| e.to_string())?;
    for (y, m, s, c) in stored {
        map.insert(
            (y, m),
            MonthlyActivityRow { year: y, month: m, steps_avg: s, calories_avg: c, computed: false },
        );
    }

    Ok(map.into_values().collect())
}

/// Store a manual monthly value (used when the user edits a cell in the dashboard
/// table). Persisting a row makes that month authoritative over the computed average.
#[tauri::command]
pub async fn upsert_monthly_activity(
    pool: State<'_, SqlitePool>,
    year: i64,
    month: i64,
    steps_avg: Option<f64>,
    calories_avg: Option<f64>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO monthly_activity (year, month, steps_avg, calories_avg) VALUES (?, ?, ?, ?) \
         ON CONFLICT(year, month) DO UPDATE SET \
         steps_avg = COALESCE(excluded.steps_avg, monthly_activity.steps_avg), \
         calories_avg = COALESCE(excluded.calories_avg, monthly_activity.calories_avg)",
    )
    .bind(year)
    .bind(month)
    .bind(steps_avg)
    .bind(calories_avg)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}
