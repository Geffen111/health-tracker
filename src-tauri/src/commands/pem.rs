use crate::models::PemCalibration;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

// ── Output types ──

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PemPrediction {
    pub id: i64,
    pub log_date: String,
    pub physical_load: Option<f64>,
    pub cognitive_load: Option<f64>,
    pub sensory_social_load: Option<f64>,
    pub three_day_weighted_load: Option<f64>,
    pub recovery_debt: Option<f64>,
    pub threshold_penalty: Option<f64>,
    pub predicted_pem_risk: Option<f64>,
    pub risk_band: Option<String>,
    pub crash_flag: Option<bool>,
    pub predicted_next_day_fatigue: Option<f64>,
    pub predicted_low: Option<f64>,
    pub predicted_high: Option<f64>,
    pub computed_at: Option<String>,
}

/// Activity row from ActivityLog for PEM computation
#[derive(Debug, sqlx::FromRow)]
struct ActivityPemRow {
    pub duration_hours: f64,
    pub category_name: String,
    pub energy_cost: String,
}

/// Daily summary for a date (aggregated from ActivityLog)
#[derive(Debug, Default)]
struct DailySummary {
    pub hobby_creative_hrs: f64,
    pub screen_sedentary_hrs: f64,
    pub social_hrs: f64,
    pub domestic_hrs: f64,
    pub physical_active_hrs: f64,
    pub cognitive_active_hrs: f64,
    pub total_hours: f64,
    pub high_energy_hours: f64,
}

/// Daily log fields needed for PEM
#[derive(Debug, sqlx::FromRow)]
struct PemDailyLog {
    pub fatigue_rating: Option<f64>,
    pub sleep_avg: Option<f64>,
    pub steps: Option<i64>,
    pub activity_calories: Option<f64>,
    pub office_hours: Option<f64>,
    pub wfh_hours: Option<f64>,
}

// ── Public commands ──

#[tauri::command]
pub async fn get_calibration_params(pool: State<'_, SqlitePool>) -> Result<Vec<PemCalibration>, String> {
    sqlx::query_as::<_, PemCalibration>("SELECT * FROM pem_calibration ORDER BY param_name")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_calibration_param(
    pool: State<'_, SqlitePool>,
    param_name: String,
    param_value: f64,
) -> Result<(), String> {
    sqlx::query("UPDATE pem_calibration SET param_value = ? WHERE param_name = ?")
        .bind(param_value).bind(&param_name)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_pem_predictions(
    pool: State<'_, SqlitePool>,
    limit: Option<i64>,
) -> Result<Vec<PemPrediction>, String> {
    let limit = limit.unwrap_or(30);
    sqlx::query_as::<_, PemPrediction>(
        "SELECT * FROM pem_predictions ORDER BY log_date DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

/// Recompute and persist a single date's PEM prediction.
/// Shared by `run_pem_model` (one date) and `backfill_pem_predictions` (all dates).
async fn compute_and_store(pool: &SqlitePool, date: String) -> Result<(), String> {
    // ── 1. Fetch daily log ──
    let log: Option<PemDailyLog> = sqlx::query_as(
        "SELECT fatigue_rating, \
         COALESCE(sleep_avg, my_sleep_rating, phone_sleep_rating) AS sleep_avg, \
         steps, activity_calories, \
         office_hours, wfh_hours, rostered_hours \
         FROM daily_logs WHERE log_date = ?"
    )
    .bind(&date)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let log = log.ok_or_else(|| "No daily log found for this date".to_string())?;

    // ── 2. Fetch all calibration params ──
    let params: Vec<PemCalibration> = sqlx::query_as(
        "SELECT * FROM pem_calibration"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let f = |name: &str, default: f64| get_param(&params, name).unwrap_or(default);

    // ── 3. Fetch activity log entries for this date ──
    let activities: Vec<ActivityPemRow> = sqlx::query_as(
        "SELECT al.duration_hours, ac.name AS category_name, al.energy_cost \
         FROM activity_log al \
         JOIN activity_types at ON al.activity_type_id = at.id \
         JOIN activity_categories ac ON at.category_id = ac.id \
         WHERE al.log_date = ?"
    )
    .bind(&date)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // ── 4. Compute DailySummary from activities ──
    let summary = compute_daily_summary(&activities, &f);

    // ── 5. Compute activity-based load components ──
    let (activity_physical, activity_cognitive, activity_sensory_social) = compute_activity_loads(&activities, &f);

    // ── 6. Determine if we're past the ActivityLog start date ──
    let al_start_date = f("activity_log_start_date", 46151.0);
    let date_serial = date_to_excel_serial(&date);
    let use_activity_log = date_serial >= al_start_date;

    // ── 7. Extract daily log values ──
    let fatigue = log.fatigue_rating.unwrap_or(0.0);
    let sleep = log.sleep_avg;
    let steps = log.steps.unwrap_or(0) as f64;
    let calories = log.activity_calories.unwrap_or(0.0);
    let office_hours = log.office_hours.unwrap_or(0.0);
    let wfh_hours = log.wfh_hours.unwrap_or(0.0);

    // ── 8. Load parameters ──
    let steps_normaliser = f("steps_normaliser", 2000.0);
    let steps_weight = f("steps_weight", 1.0);
    let steps_load_cap = f("steps_load_cap", 4.0);
    let calories_normaliser = f("calories_normaliser", 500.0);
    let calories_weight = f("calories_weight", 0.0);
    let fatigue_sensitivity_divisor = f("fatigue_sensitivity_divisor", 9.0);
    let fatigue_load_penalty = f("fatigue_load_penalty", 0.2);
    let recovery_credit = f("recovery_credit", 0.14);
    let _active_while_fatigued_penalty = f("active_while_fatigued_penalty", 0.2);
    // Actually the spreadsheet uses high_energy_fatigued_multiplier (0.1)
    let high_energy_multiplier = f("high_energy_fatigued_multiplier", 0.1);
    let debt_persistence = f("debt_persistence", 0.55);
    let crash_threshold = f("crash_threshold", 4.0);
    let threshold_exponent = f("threshold_exponent", 1.3);
    let risk_divisor = f("risk_divisor", 2.5);
    let sleep_weight = f("sleep_weight", 0.2);
    let sleep_baseline = f("sleep_baseline", 8.0);
    // Next-day fatigue is now an OLS fit on recovery debt (the best CV predictor),
    // not the old composite-risk mapping.
    let fatigue_from_debt_intercept = f("fatigue_from_debt_intercept", 3.5696);
    let fatigue_from_debt_slope = f("fatigue_from_debt_slope", 0.5316);
    let prediction_range = f("prediction_range", 1.60);

    // ── 8b. Previous day's recovery debt (for carryover) ──
    // PEM is cumulative: debt persists and decays rather than resetting daily.
    // Use the most recent earlier prediction, decayed by one persistence factor per
    // elapsed day (so gaps in the history decay the carried debt appropriately).
    let prev_debt: Option<(String, Option<f64>)> = sqlx::query_as(
        "SELECT log_date, recovery_debt FROM pem_predictions WHERE log_date < ? ORDER BY log_date DESC LIMIT 1"
    )
    .bind(&date)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let carried_debt = match prev_debt {
        Some((prev_date, Some(d))) => {
            let gap = (date_to_excel_serial(&date) - date_to_excel_serial(&prev_date)).max(1.0);
            d * debt_persistence.powf(gap)
        }
        _ => 0.0,
    };

    // ── 9. Physical Load (G) — steps only (calories collinear, weight 0) ──
    let step_load = (steps / steps_normaliser).min(steps_load_cap);
    let cal_load = (calories / calories_normaliser).min(2.0);
    let physical_base = step_load * steps_weight + cal_load * calories_weight;

    let physical_load = if use_activity_log {
        // Post-ActivityLog: average of base and activity-based physical load
        (physical_base + activity_physical) / 3.0
    } else {
        physical_base
    };

    // ── 10. Cognitive Load (H) ──
    let cognitive_base = (office_hours * 1.2 + wfh_hours * 0.9) / 3.0;

    let cognitive_load = if use_activity_log {
        (cognitive_base + activity_cognitive) / 3.0
    } else {
        cognitive_base
    };

    // ── 11. Sensory/Social Load (I) ──
    let sensory_social_load = if use_activity_log {
        activity_sensory_social / 3.0
    } else {
        0.0
    };

    // ── 12. Sensitivity (J) = 1 + Fatigue / FatigueSensitivityDivisor ──
    let sensitivity = 1.0 + (fatigue / fatigue_sensitivity_divisor);

    // ── 13. 3-Day Weighted Load (K) = (G + H + I) * 0.55 ──
    let three_day_weighted = (physical_load + cognitive_load + sensory_social_load) * 0.55;

    // ── 14. Recovery Debt (L) ──
    // L = MAX(0, K + MAX(0, F-5)*FatigueLoadPenalty - (10-F)*RecoveryCredit
    //          + IF(F>=6, HighEnergyHours * HighEnergyFatiguedMultiplier, 0))
    let fatigue_penalty = (fatigue - 5.0).max(0.0) * fatigue_load_penalty;
    let recovery_credit_amount = (10.0 - fatigue).max(0.0) * recovery_credit;
    let active_penalty = if fatigue >= 6.0 {
        summary.high_energy_hours * high_energy_multiplier
    } else {
        0.0
    };
    let recovery_debt = (carried_debt + three_day_weighted + fatigue_penalty - recovery_credit_amount + active_penalty).max(0.0);

    // ── 15. Threshold Penalty (M) = IF(L > CrashThreshold, (L - CrashThreshold)^ThresholdExponent, 0) ──
    let threshold_penalty = if recovery_debt > crash_threshold {
        (recovery_debt - crash_threshold).powf(threshold_exponent)
    } else {
        0.0
    };

    // ── 16. Sleep Penalty ──
    let sleep_penalty = match sleep {
        Some(s) => (sleep_baseline - s).max(0.0) * sleep_weight,
        None => 0.0,
    };

    // ── 17. Predicted PEM Risk (N) ──
    // N = MIN(10, ((K * J) + (L * 0.9) + (M * 1.1) + SleepPenalty) / RiskDivisor)
    let risk = ((three_day_weighted * sensitivity)
        + (recovery_debt * 0.9)
        + (threshold_penalty * 1.1)
        + sleep_penalty)
        / risk_divisor;
    let predicted_pem_risk = risk.min(10.0);

    // ── 18. Predicted next-day fatigue (OLS on recovery debt) ──
    let predicted_next_day =
        (fatigue_from_debt_intercept + fatigue_from_debt_slope * recovery_debt).max(1.0).min(10.0);

    // ── 19. Risk Band — derived from the predicted fatigue so the stored band,
    // the dashboard and the PEM gauge all agree (Low ≤3, Medium ≤6, High >6) ──
    let risk_band = if predicted_next_day > 6.0 {
        "High"
    } else if predicted_next_day > 3.0 {
        "Medium"
    } else {
        "Low"
    };

    // ── 20. Crash Flag = sustained debt past the crash threshold ──
    let crash_flag = threshold_penalty > 0.0;

    // ── 21. Confidence range ──
    let predicted_low = (predicted_next_day - prediction_range).max(0.0);
    let predicted_high = (predicted_next_day + prediction_range).min(10.0);

    // ── 22. Upsert prediction ──
    sqlx::query(
        "INSERT INTO pem_predictions (\
         log_date, physical_load, cognitive_load, sensory_social_load, \
         three_day_weighted_load, recovery_debt, threshold_penalty, \
         predicted_pem_risk, risk_band, crash_flag, \
         predicted_next_day_fatigue, predicted_low, predicted_high, computed_at)\
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))\
         ON CONFLICT(log_date) DO UPDATE SET \
         physical_load=excluded.physical_load, cognitive_load=excluded.cognitive_load, \
         sensory_social_load=excluded.sensory_social_load, \
         three_day_weighted_load=excluded.three_day_weighted_load, \
         recovery_debt=excluded.recovery_debt, threshold_penalty=excluded.threshold_penalty, \
         predicted_pem_risk=excluded.predicted_pem_risk, risk_band=excluded.risk_band, \
         crash_flag=excluded.crash_flag, \
         predicted_next_day_fatigue=excluded.predicted_next_day_fatigue, \
         predicted_low=excluded.predicted_low, predicted_high=excluded.predicted_high, \
         computed_at=excluded.computed_at"
    )
    .bind(&date).bind(physical_load).bind(cognitive_load).bind(sensory_social_load)
    .bind(three_day_weighted).bind(recovery_debt).bind(threshold_penalty)
    .bind(predicted_pem_risk).bind(&risk_band).bind(crash_flag)
    .bind(predicted_next_day).bind(predicted_low).bind(predicted_high)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Recompute one date's prediction and return it.
#[tauri::command]
pub async fn run_pem_model(pool: State<'_, SqlitePool>, date: String) -> Result<PemPrediction, String> {
    compute_and_store(&pool, date.clone()).await?;
    sqlx::query_as::<_, PemPrediction>(
        "SELECT * FROM pem_predictions WHERE log_date = ?"
    )
    .bind(&date)
    .fetch_one(&*pool)
    .await
    .map_err(|e| e.to_string())
}

/// Recompute predictions for every logged date in the activity-tracking era so the
/// history reflects real day-to-day variation (the per-page run only ever computes
/// yesterday). Dates before `activity_log_start_date` are skipped: without activity
/// data the model is barely better than guessing (R^2 ~0.02), so those predictions
/// would only add noise to the history. Returns the number of dates recomputed.
#[tauri::command]
pub async fn backfill_pem_predictions(pool: State<'_, SqlitePool>) -> Result<i64, String> {
    let start: Option<f64> = sqlx::query_scalar(
        "SELECT param_value FROM pem_calibration WHERE param_name = 'activity_log_start_date'"
    )
    .fetch_optional(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    let start = start.unwrap_or(46151.0);

    let dates: Vec<(String,)> = sqlx::query_as(
        "SELECT log_date FROM daily_logs WHERE fatigue_rating IS NOT NULL ORDER BY log_date"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count = 0i64;
    for (date,) in dates {
        if date_to_excel_serial(&date) < start {
            continue;
        }
        compute_and_store(&pool, date).await?;
        count += 1;
    }
    Ok(count)
}

// ── Helper functions ──

fn get_param(params: &[PemCalibration], name: &str) -> Option<f64> {
    params.iter().find(|p| p.param_name == name).map(|p| p.param_value)
}

/// Compute DailySummary from ActivityLog entries for a date.
fn compute_daily_summary(activities: &[ActivityPemRow], f: &impl Fn(&str, f64) -> f64) -> DailySummary {
    let energy_factor = |cost: &str| -> f64 {
        match cost {
            "High" => f("energy_factor_high", 2.0),
            "Medium" => f("energy_factor_medium", 1.0),
            _ => f("energy_factor_low", 0.7),
        }
    };

    let mut summary = DailySummary::default();

    for act in activities {
        let cat = act.category_name.trim();
        let hrs = act.duration_hours;
        let _ef = energy_factor(&act.energy_cost);

        match cat {
            "Hobby / Creative" => summary.hobby_creative_hrs += hrs,
            "Screen / Sedentary" => summary.screen_sedentary_hrs += hrs,
            "Social" => summary.social_hrs += hrs,
            "Domestic" => summary.domestic_hrs += hrs,
            "Physical / Active" => summary.physical_active_hrs += hrs,
            "Cognitive / Active" => summary.cognitive_active_hrs += hrs,
            _ => {}
        }

        summary.total_hours += hrs;

        // High Energy Hours = duration of activities tagged "High" energy cost
        if act.energy_cost == "High" {
            summary.high_energy_hours += hrs;
        }
    }

    summary
}

/// Activity-based load components used in PEM calculation (post-ActivityLog date).
/// Returns (physical_load, cognitive_load, sensory_social_load).
fn compute_activity_loads(
    activities: &[ActivityPemRow],
    f: &impl Fn(&str, f64) -> f64,
) -> (f64, f64, f64) {
    let energy_factor = |cost: &str| -> f64 {
        match cost {
            "High" => f("energy_factor_high", 2.0),
            "Medium" => f("energy_factor_medium", 1.0),
            _ => f("energy_factor_low", 0.7),
        }
    };

    let weight_physical = f("weight_physical_active", 1.0);
    let weight_domestic = f("weight_domestic", 0.5);
    let weight_cognitive = f("weight_cognitive_active", 1.4);
    let weight_hobby = f("weight_hobby_creative", 0.5);
    let weight_social = f("weight_social", 0.6);
    let weight_screen = f("weight_screen_sedentary", 0.3);

    let mut physical = 0.0;
    let mut cognitive = 0.0;
    let mut sensory_social = 0.0;

    for act in activities {
        let cat = act.category_name.trim();
        let hrs = act.duration_hours;
        let ef = energy_factor(&act.energy_cost);

        match cat {
            // Physical / Active + Domestic → Physical Load
            "Physical / Active" => physical += hrs * weight_physical * ef,
            "Domestic" => physical += hrs * weight_domestic * ef,
            // Cognitive / Active + Hobby / Creative → Cognitive Load
            "Cognitive / Active" => cognitive += hrs * weight_cognitive * ef,
            "Hobby / Creative" => cognitive += hrs * weight_hobby * ef,
            // Social + Screen / Sedentary → Sensory/Social Load
            "Social" => sensory_social += hrs * weight_social * ef,
            "Screen / Sedentary" => sensory_social += hrs * weight_screen * ef,
            _ => {}
        }
    }

    (physical, cognitive, sensory_social)
}

/// Convert "YYYY-MM-DD" to Excel serial date number.
/// Excel serial 1 = 1900-01-01. Skepticism: 1900-02-28 is 59, 1900-03-01 is 61 (Excel thinks 1900 is a leap year).
/// We use chrono to compute the correct serial.
fn date_to_excel_serial(date_str: &str) -> f64 {
    if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
        let days = d.signed_duration_since(epoch).num_days() as f64;
        // Excel leap-year bug: dates after 1900-02-28 need +1 (Excel treats 1900-02-29 as valid)
        if d > chrono::NaiveDate::from_ymd_opt(1900, 2, 28).unwrap() {
            days + 1.0
        } else {
            days
        }
    } else {
        0.0
    }
}
