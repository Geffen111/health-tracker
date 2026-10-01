// Weekly summary: each Monday-to-Sunday week gets one AI write-up of the log.
//
// Division of labour (so the model can't invent figures):
//   * Rust computes every number — the scorecard against the user's own 8-week baseline,
//     medication adherence, new foods, exposures, new lab results, changed vault notes and
//     any day-lag correlations. That is `WeekMetrics`.
//   * The model only turns `WeekMetrics` into prose (`Narrative`).
// Both are stored in `weekly_summaries` (migration 20240628), so old weeks load without
// another AI call. Generation runs from the layout once the launch CSV import has finished:
// if the last full week has no summary yet it is made then, so a missed Monday is caught up.
//
// Same rule as Pacing: descriptive only. No risk score, no forecast of tomorrow's fatigue.
//
// Privacy: the figures above plus medication names, food names, exposure descriptions and
// the titles of changed vault notes go to OpenRouter. Raw vault note text does not.

use crate::commands::ai::{call_openrouter, strip_code_fences};
use crate::commands::{pacing, settings, vault};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;

/// How many earlier weeks make up "your usual" for the scorecard.
const BASELINE_WEEKS: i64 = 8;
/// Window of daily data used for the correlation scan.
const CORR_DAYS: i64 = 84;
/// Lab results dated within this many days of the week's end can be reported as new.
const LAB_LOOKBACK_DAYS: i64 = 30;

// ── Output types ──

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreRow {
    pub key: String,
    pub label: String,
    pub unit: String,
    pub decimals: u8,
    /// `Some(true)` when a higher value is worse (fatigue), `Some(false)` when better
    /// (sleep score), `None` when it is neutral (steps, activity).
    pub higher_is_worse: Option<bool>,
    pub this_week: Option<f64>,
    pub last_week: Option<f64>,
    pub baseline: Option<f64>,
    /// "higher" / "lower" when this week sits at least one SD from the baseline mean.
    pub flag: Option<String>,
    /// True when the gap is two SDs or more.
    pub strong: bool,
    pub n_days: i64,
    /// Weekly means, oldest first: the 8 baseline weeks then this week. For the sparkline.
    pub history: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Completeness {
    pub days_in_week: i64,
    pub fatigue_days: i64,
    pub sleep_days: i64,
    pub steps_days: i64,
    pub medication_days: i64,
    pub food_days: i64,
    pub activity_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DayNote {
    pub date: String,
    pub fatigue: f64,
    pub fatigue_desc: Option<String>,
    pub prev_day_sleep_score: Option<f64>,
    pub prev_day_hours_asleep: Option<f64>,
    pub prev_day_steps: Option<f64>,
    pub prev_day_activity_hours: Option<f64>,
    pub exposures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MedAdherence {
    pub name: String,
    pub expected_doses: i64,
    pub logged_doses: i64,
    pub missed_doses: i64,
    pub days_missed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OccasionalUse {
    pub name: String,
    pub doses: i64,
    pub days: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MedsWeek {
    pub changes: Vec<String>,
    pub adherence: Vec<MedAdherence>,
    pub occasional: Vec<OccasionalUse>,
    /// Days this week with no dose of any medication logged — unknown, not necessarily missed.
    pub days_without_any_dose: Vec<String>,
    pub days_with_dose: i64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FoodWeek {
    pub top_items: Vec<(String, i64)>,
    /// Foods whose very first log entry falls in this week.
    pub new_items: Vec<String>,
    pub worst_day_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LabItem {
    pub test: String,
    pub date: String,
    pub value: String,
    pub reference: Option<String>,
    pub flag: Option<String>,
    pub previous: Option<String>,
    /// Vault note the value came from. Used only to tell results apart; not sent to the model.
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultChange {
    pub folder: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Correlation {
    pub measure: String,
    /// 0 = same day as the fatigue rating; 1 = the measure is from the day before.
    pub lag_days: u8,
    pub r: f64,
    pub n: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeekMetrics {
    pub week_start: String,
    pub week_end: String,
    pub completeness: Completeness,
    pub scorecard: Vec<ScoreRow>,
    pub bad_days: i64,
    pub good_days: i64,
    pub worst_day: Option<DayNote>,
    pub best_day: Option<DayNote>,
    pub activity_by_category: Vec<(String, f64)>,
    pub meds: MedsWeek,
    pub food: FoodWeek,
    pub exposures: Vec<String>,
    pub new_lab_results: Vec<LabItem>,
    pub vault_changes: Vec<VaultChange>,
    pub correlations: Vec<Correlation>,
    pub correlations_tested: i64,
    /// Lab results already reported by this or an earlier summary (`test|date|source`), so
    /// the next week only lists what is new.
    #[serde(default)]
    pub reported_lab_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrSection {
    pub title: String,
    #[serde(default)]
    pub icon: String,
    /// "positive" | "neutral" | "warning" | "critical"
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Narrative {
    pub headline: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub sections: Vec<NarrSection>,
    #[serde(default)]
    pub to_raise: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WeeklySummary {
    pub week_start: String,
    pub week_end: String,
    pub generated_at: String,
    pub seen: bool,
    pub metrics: WeekMetrics,
    pub narrative: Narrative,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WeeklyBanner {
    pub week_start: String,
    pub week_end: String,
    pub seen: bool,
}

// ── Daily series ──

#[derive(Clone, Default)]
struct Day {
    fatigue: Option<f64>,
    headache: Option<f64>,
    sleep: Option<f64>,
    asleep: Option<f64>,
    steps: Option<f64>,
    rhr: Option<f64>,
    alcohol: Option<f64>,
    load: Option<f64>,
    load_hours: Option<f64>,
    sys: Option<f64>,
    dia: Option<f64>,
    desc: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DbDay {
    log_date: String,
    fatigue: Option<f64>,
    headache: Option<f64>,
    sleep: Option<f64>,
    asleep: Option<f64>,
    steps: Option<f64>,
    rhr: Option<f64>,
    alcohol: Option<f64>,
    fatigue_desc: Option<String>,
}

type Getter = fn(&Day) -> Option<f64>;

/// key, label, unit, decimals, higher_is_worse, getter
const SCORECARD: &[(&str, &str, &str, u8, Option<bool>, Getter)] = &[
    ("fatigue", "Fatigue", "/10", 1, Some(true), |d| d.fatigue),
    ("headache", "Headache", "/10", 1, Some(true), |d| d.headache),
    ("sleep", "Sleep score", "/10", 1, Some(false), |d| d.sleep),
    ("asleep", "Hours asleep", "h", 1, Some(false), |d| d.asleep),
    ("steps", "Steps", "", 0, None, |d| d.steps),
    ("load", "Activity load", "", 1, None, |d| d.load),
    ("load_hours", "Logged activity", "h", 1, None, |d| d.load_hours),
    ("rhr", "Resting heart rate", "bpm", 0, Some(true), |d| d.rhr),
    ("sys", "Systolic BP", "mmHg", 0, None, |d| d.sys),
    ("dia", "Diastolic BP", "mmHg", 0, None, |d| d.dia),
    ("alcohol", "Alcohol", "drinks", 1, Some(true), |d| d.alcohol),
];

fn iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() { None } else { Some(xs.iter().sum::<f64>() / xs.len() as f64) }
}

fn sd(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 { return None; }
    let m = mean(xs)?;
    Some((xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() as f64 - 1.0)).sqrt())
}

fn pearson(pairs: &[(f64, f64)]) -> Option<f64> {
    let n = pairs.len() as f64;
    if n < 3.0 { return None; }
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in pairs {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx).powi(2);
        syy += (y - my).powi(2);
    }
    if sxx <= 0.0 || syy <= 0.0 { return None; }
    Some(sxy / (sxx * syy).sqrt())
}

/// The metric's values over the 7 days starting at `start`.
fn week_values(days: &BTreeMap<NaiveDate, Day>, get: Getter, start: NaiveDate) -> Vec<f64> {
    (0..7)
        .filter_map(|i| days.get(&(start + Duration::days(i))).and_then(get))
        .collect()
}

async fn load_days(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<BTreeMap<NaiveDate, Day>, String> {
    let (f, t) = (iso(from), iso(to));
    let mut days: BTreeMap<NaiveDate, Day> = BTreeMap::new();
    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();

    let rows: Vec<DbDay> = sqlx::query_as(
        "SELECT log_date,
                CAST(fatigue_rating AS REAL) AS fatigue,
                CAST(headache_rating AS REAL) AS headache,
                CAST(sleep_avg AS REAL) AS sleep,
                CAST(sleep_actual_asleep AS REAL) AS asleep,
                CAST(steps AS REAL) AS steps,
                CAST(ave_resting_hr AS REAL) AS rhr,
                CAST(alcohol_std_drinks AS REAL) AS alcohol,
                fatigue_desc
         FROM daily_logs WHERE log_date >= ? AND log_date <= ?",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error daily logs: {}", e))?;
    for r in rows {
        let Some(date) = parse(&r.log_date) else { continue };
        days.insert(date, Day {
            fatigue: r.fatigue,
            headache: r.headache,
            sleep: r.sleep,
            asleep: r.asleep.filter(|v| *v > 0.0),
            steps: r.steps.filter(|v| *v > 0.0),
            rhr: r.rhr.filter(|v| *v > 0.0),
            alcohol: r.alcohol,
            // A logged day with no activity rows has a load of zero, not an unknown one.
            load: Some(0.0),
            load_hours: Some(0.0),
            desc: r.fatigue_desc.filter(|s| !s.trim().is_empty()),
            ..Default::default()
        });
    }

    let sql = format!(
        "SELECT al.log_date,
                CAST(COALESCE(SUM({load}), 0) AS REAL),
                CAST(COALESCE(SUM(al.duration_hours), 0) AS REAL)
         FROM activity_log al
         JOIN activity_types at ON al.activity_type_id = at.id
         JOIN activity_categories ac ON at.category_id = ac.id
         WHERE al.log_date >= ? AND al.log_date <= ?
         GROUP BY al.log_date",
        load = pacing::LOAD_EXPR
    );
    let loads: Vec<(String, f64, f64)> = sqlx::query_as(&sql)
        .bind(&f).bind(&t)
        .fetch_all(pool).await.map_err(|e| format!("DB error loads: {}", e))?;
    for (d, load, hours) in loads {
        if let Some(day) = parse(&d).and_then(|date| days.get_mut(&date)) {
            day.load = Some(load);
            day.load_hours = Some(hours);
        }
    }

    let bp: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT log_date, CAST(AVG(systolic) AS REAL), CAST(AVG(diastolic) AS REAL)
         FROM blood_pressure WHERE log_date >= ? AND log_date <= ? GROUP BY log_date",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error bp: {}", e))?;
    for (d, sys, dia) in bp {
        if let Some(date) = parse(&d) {
            let day = days.entry(date).or_default();
            day.sys = Some(sys);
            day.dia = Some(dia);
        }
    }
    Ok(days)
}

// ── Metric assembly ──

fn build_scorecard(days: &BTreeMap<NaiveDate, Day>, ws: NaiveDate) -> Vec<ScoreRow> {
    let mut rows = Vec::new();
    for (key, label, unit, decimals, worse, get) in SCORECARD {
        let week_mean = |w: i64, min: usize| {
            let v = week_values(days, *get, ws - Duration::days(7 * w));
            if v.len() >= min { mean(&v) } else { None }
        };
        let this_vals = week_values(days, *get, ws);
        let this_week = mean(&this_vals);
        let last_week = week_mean(1, 1);
        let baseline_means: Vec<f64> = (1..=BASELINE_WEEKS).filter_map(|w| week_mean(w, 3)).collect();
        let (mut baseline, mut flag, mut strong) = (None, None, false);
        if baseline_means.len() >= 4 {
            baseline = mean(&baseline_means);
            if let (Some(b), Some(s), Some(t)) = (baseline, sd(&baseline_means), this_week) {
                if s > 1e-9 {
                    let z = (t - b) / s;
                    if z.abs() >= 1.0 {
                        flag = Some(if z > 0.0 { "higher" } else { "lower" }.to_string());
                        strong = z.abs() >= 2.0;
                    }
                }
            }
        }
        // Skip a metric with no data this week or in the baseline (e.g. BP never logged).
        if this_week.is_none() && baseline.is_none() {
            continue;
        }
        let history = (0..=BASELINE_WEEKS).rev().map(|w| week_mean(w, if w == 0 { 1 } else { 2 })).collect();
        rows.push(ScoreRow {
            key: key.to_string(),
            label: label.to_string(),
            unit: unit.to_string(),
            decimals: *decimals,
            higher_is_worse: *worse,
            this_week,
            last_week,
            baseline,
            flag,
            strong,
            n_days: this_vals.len() as i64,
            history,
        });
    }
    rows
}

fn build_correlations(days: &BTreeMap<NaiveDate, Day>, week_end: NaiveDate) -> (Vec<Correlation>, i64) {
    let start = week_end - Duration::days(CORR_DAYS - 1);
    let measures: &[(&str, Getter, bool)] = &[
        ("sleep score", |d| d.sleep, true),
        ("hours asleep", |d| d.asleep, true),
        ("steps", |d| d.steps, true),
        ("activity load", |d| d.load, true),
        ("resting heart rate", |d| d.rhr, true),
        ("headache", |d| d.headache, false),
        ("alcohol", |d| d.alcohol, true),
    ];
    let (mut found, mut tested) = (Vec::new(), 0i64);
    for (name, get, with_lag) in measures {
        for lag in 0..=(if *with_lag { 1u8 } else { 0 }) {
            let mut pairs = Vec::new();
            let mut date = start;
            while date <= week_end {
                let fatigue = days.get(&(date + Duration::days(lag as i64))).and_then(|d| d.fatigue);
                if let (Some(x), Some(y)) = (days.get(&date).and_then(get), fatigue) {
                    pairs.push((x, y));
                }
                date += Duration::days(1);
            }
            if pairs.len() < 20 { continue; }
            if let Some(r) = pearson(&pairs) {
                tested += 1;
                if r.abs() >= 0.3 {
                    found.push(Correlation { measure: name.to_string(), lag_days: lag, r, n: pairs.len() as i64 });
                }
            }
        }
    }
    found.sort_by(|a, b| b.r.abs().partial_cmp(&a.r.abs()).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(6);
    (found, tested)
}

async fn build_meds(
    pool: &SqlitePool,
    ws: NaiveDate,
    we: NaiveDate,
    logged_days: &HashSet<NaiveDate>,
) -> Result<MedsWeek, String> {
    let (f, t) = (iso(ws), iso(we));
    let mut out = MedsWeek::default();

    let history: Vec<(String, String, String, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT event_date, medication_name, event_type, detail, old_value, new_value
             FROM medication_history WHERE event_date >= ? AND event_date <= ?
             ORDER BY event_date, id",
        )
        .bind(&f).bind(&t)
        .fetch_all(pool).await.map_err(|e| format!("DB error med history: {}", e))?;
    for (date, name, kind, detail, old, new) in history {
        let mut line = format!("{} — {}: {}", date, name, kind.replace('_', " "));
        if let (Some(o), Some(n)) = (&old, &new) {
            line.push_str(&format!(" ({} → {})", o, n));
        }
        if let Some(d) = detail.filter(|d| !d.trim().is_empty()) {
            line.push_str(&format!(" — {}", d));
        }
        out.changes.push(line);
    }

    out.notes = sqlx::query_as::<_, (String, String)>(
        "SELECT log_date, note FROM medication_notes WHERE log_date >= ? AND log_date <= ? ORDER BY log_date",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error med notes: {}", e))?
    .into_iter()
    .map(|(d, n)| format!("{}: {}", d, n))
    .collect();

    // Doses actually taken (an amount of 0 is a skipped dose), by medication and day.
    let doses: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT m.id, m.name, md.log_date,
                CAST(SUM(CASE WHEN COALESCE(md.dose_amount, 1) > 0 THEN 1 ELSE 0 END) AS INTEGER)
         FROM medication_doses md JOIN medications m ON md.medication_id = m.id
         WHERE md.log_date >= ? AND md.log_date <= ?
         GROUP BY m.id, m.name, md.log_date",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error doses: {}", e))?;

    let mut taken: HashMap<(i64, String), i64> = HashMap::new();
    let mut days_with_dose: HashSet<String> = HashSet::new();
    for (id, _, date, n) in &doses {
        if *n > 0 {
            days_with_dose.insert(date.clone());
            taken.insert((*id, date.clone()), *n);
        }
    }

    // "Missed" is inferred: the medication's *current* schedule slots for the day minus the
    // doses logged. The app has no explicit "skipped" record. Only days with at least one
    // dose logged count, so a day nobody filled in isn't reported as a day of misses.
    let regular: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT m.id, m.name, COUNT(s.id)
         FROM medications m JOIN medication_schedule s ON s.medication_id = m.id
         WHERE m.active = 1 AND COALESCE(m.med_type, 'regular') = 'regular'
         GROUP BY m.id, m.name ORDER BY m.name",
    )
    .fetch_all(pool).await.map_err(|e| format!("DB error schedule: {}", e))?;
    let tracked_days: Vec<NaiveDate> = (0..7)
        .map(|i| ws + Duration::days(i))
        .filter(|d| days_with_dose.contains(&iso(*d)))
        .collect();
    for (id, name, slots) in regular {
        let (mut logged, mut missed, mut days_missed) = (0i64, 0i64, Vec::new());
        for d in &tracked_days {
            let n = *taken.get(&(id, iso(*d))).unwrap_or(&0);
            logged += n.min(slots);
            if n < slots {
                missed += slots - n;
                days_missed.push(format!("{} ({} of {})", iso(*d), slots - n, slots));
            }
        }
        out.adherence.push(MedAdherence {
            name,
            expected_doses: slots * tracked_days.len() as i64,
            logged_doses: logged,
            missed_doses: missed,
            days_missed,
        });
    }

    let occasional: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT m.name, md.log_date,
                CAST(SUM(CASE WHEN COALESCE(md.dose_amount, 1) > 0 THEN 1 ELSE 0 END) AS INTEGER)
         FROM medication_doses md JOIN medications m ON md.medication_id = m.id
         WHERE md.log_date >= ? AND md.log_date <= ?
           AND (m.med_type = 'occasional' OR m.category = 'PRN')
         GROUP BY m.name, md.log_date ORDER BY m.name, md.log_date",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error occasional: {}", e))?;
    for (name, date, n) in occasional {
        if n == 0 { continue; }
        match out.occasional.last_mut() {
            Some(o) if o.name == name => { o.doses += n; o.days.push(date); }
            _ => out.occasional.push(OccasionalUse { name, doses: n, days: vec![date] }),
        }
    }

    // Days the user filled in the day but logged no doses at all — unknown, flagged separately.
    out.days_without_any_dose = (0..7)
        .map(|i| ws + Duration::days(i))
        .filter(|d| logged_days.contains(d) && !days_with_dose.contains(&iso(*d)))
        .map(iso)
        .collect();
    out.days_with_dose = days_with_dose.len() as i64;
    Ok(out)
}

async fn build_food(
    pool: &SqlitePool,
    ws: NaiveDate,
    we: NaiveDate,
    worst_day: Option<&str>,
) -> Result<(FoodWeek, i64), String> {
    let (f, t) = (iso(ws), iso(we));
    let top_items: Vec<(String, i64)> = sqlx::query_as(
        "SELECT fo.name, COUNT(*) AS n FROM food_log fl JOIN foods fo ON fl.food_id = fo.id
         WHERE fl.log_date >= ? AND fl.log_date <= ?
         GROUP BY fo.id ORDER BY n DESC, fo.name LIMIT 10",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error food: {}", e))?;

    let new_items: Vec<(String,)> = sqlx::query_as(
        "SELECT fo.name FROM foods fo
         WHERE (SELECT MIN(log_date) FROM food_log WHERE food_id = fo.id) BETWEEN ? AND ?
         ORDER BY fo.name LIMIT 20",
    )
    .bind(&f).bind(&t)
    .fetch_all(pool).await.map_err(|e| format!("DB error new foods: {}", e))?;

    let worst_day_items: Vec<(String,)> = match worst_day {
        Some(d) => sqlx::query_as(
            "SELECT DISTINCT fo.name FROM food_log fl JOIN foods fo ON fl.food_id = fo.id
             WHERE fl.log_date = ? ORDER BY fo.name",
        )
        .bind(d)
        .fetch_all(pool).await.map_err(|e| format!("DB error worst-day food: {}", e))?,
        None => vec![],
    };

    let food_days: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT log_date) FROM food_log WHERE log_date >= ? AND log_date <= ?",
    )
    .bind(&f).bind(&t)
    .fetch_one(pool).await.map_err(|e| format!("DB error food days: {}", e))?;

    Ok((
        FoodWeek {
            top_items,
            new_items: new_items.into_iter().map(|r| r.0).collect(),
            worst_day_items: worst_day_items.into_iter().map(|r| r.0).collect(),
        },
        food_days.0,
    ))
}

async fn exposures_between(pool: &SqlitePool, from: &str, to: &str) -> Result<Vec<(String, String)>, String> {
    sqlx::query_as(
        "SELECT log_date, description FROM exposures
         WHERE log_date >= ? AND log_date <= ? ORDER BY log_date, time_taken",
    )
    .bind(from).bind(to)
    .fetch_all(pool).await.map_err(|e| format!("DB error exposures: {}", e))
}

async fn build_labs(
    pool: &SqlitePool,
    we: NaiveDate,
    already: &HashSet<String>,
) -> Result<Vec<LabItem>, String> {
    #[derive(sqlx::FromRow)]
    struct Row {
        test_name: String,
        result_date: String,
        value_num: Option<f64>,
        value_text: Option<String>,
        unit: Option<String>,
        ref_text: Option<String>,
        flag: Option<String>,
        source_note: String,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT test_name, result_date, value_num, value_text, unit, ref_text, flag, source_note
         FROM lab_results WHERE result_date >= ? AND result_date <= ?
         ORDER BY result_date DESC, test_name",
    )
    .bind(iso(we - Duration::days(LAB_LOOKBACK_DAYS))).bind(iso(we))
    .fetch_all(pool).await.map_err(|e| format!("DB error labs: {}", e))?;

    let show = |text: Option<String>, num: Option<f64>, unit: &Option<String>| -> Option<String> {
        let v = text.filter(|t| !t.trim().is_empty()).or_else(|| num.map(|n| format!("{}", n)))?;
        Some(match unit.as_deref().filter(|u| !u.is_empty()) {
            Some(u) => format!("{} {}", v, u),
            None => v,
        })
    };

    let mut out = Vec::new();
    for r in rows {
        if already.contains(&lab_key(&r.test_name, &r.result_date, &r.source_note)) { continue; }
        let prev: Option<(Option<String>, Option<f64>, String)> = sqlx::query_as(
            "SELECT value_text, value_num, result_date FROM lab_results
             WHERE test_name = ? AND result_date < ? ORDER BY result_date DESC LIMIT 1",
        )
        .bind(&r.test_name).bind(&r.result_date)
        .fetch_optional(pool).await.map_err(|e| format!("DB error prior lab: {}", e))?;
        let previous = prev.and_then(|(t, n, d)| show(t, n, &r.unit).map(|v| format!("{} ({})", v, d)));
        out.push(LabItem {
            value: show(r.value_text, r.value_num, &r.unit).unwrap_or_default(),
            test: r.test_name,
            date: r.result_date,
            reference: r.ref_text.filter(|t| !t.trim().is_empty()),
            flag: r.flag.filter(|t| !t.trim().is_empty()),
            previous,
            source: r.source_note,
        });
        if out.len() >= 40 { break; }
    }
    Ok(out)
}

fn lab_key(test: &str, date: &str, source: &str) -> String {
    format!("{}|{}|{}", test, date, source)
}

/// Vault notes created or edited during the week, by file modified time (titles only).
fn vault_changes(ws: NaiveDate, we: NaiveDate) -> Vec<VaultChange> {
    let root = vault::vault_root();
    if !root.is_dir() { return vec![]; }
    let from = ws.and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(Local).earliest());
    let to = (we + Duration::days(1)).and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(Local).earliest());
    let (Some(from), Some(to)) = (from, to) else { return vec![] };

    let mut found: Vec<VaultChange> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') { continue; }
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let Some(modified) = entry.metadata().ok().and_then(|m| m.modified().ok()) else { continue };
                let modified: DateTime<Local> = modified.into();
                if modified < from || modified >= to { continue; }
                let Ok(rel) = path.strip_prefix(&root) else { continue };
                let folder = if rel.components().count() > 1 {
                    rel.components().next().map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default()
                } else {
                    String::new()
                };
                let title = path.file_stem().map(|s| s.to_string_lossy().replace('_', " ")).unwrap_or_default();
                found.push(VaultChange { folder, title });
            }
        }
    }
    found.sort_by(|a, b| a.folder.cmp(&b.folder).then_with(|| a.title.cmp(&b.title)));
    found.truncate(30);
    found
}

fn day_note(
    date: NaiveDate,
    days: &BTreeMap<NaiveDate, Day>,
    exposures: &[(String, String)],
) -> Option<DayNote> {
    let day = days.get(&date)?;
    let prev = days.get(&(date - Duration::days(1)));
    Some(DayNote {
        date: iso(date),
        fatigue: day.fatigue?,
        fatigue_desc: day.desc.clone(),
        prev_day_sleep_score: prev.and_then(|p| p.sleep),
        prev_day_hours_asleep: prev.and_then(|p| p.asleep),
        prev_day_steps: prev.and_then(|p| p.steps),
        prev_day_activity_hours: prev.and_then(|p| p.load_hours),
        exposures: exposures.iter().filter(|(d, _)| *d == iso(date)).map(|(_, e)| e.clone()).collect(),
    })
}

async fn build_metrics(pool: &SqlitePool, ws: NaiveDate) -> Result<WeekMetrics, String> {
    let we = ws + Duration::days(6);
    let days = load_days(pool, ws - Duration::days((7 * BASELINE_WEEKS).max(CORR_DAYS)), we).await?;

    let week_days: Vec<(&NaiveDate, &Day)> = days.range(ws..=we).collect();
    let logged_days: HashSet<NaiveDate> = week_days.iter().map(|(d, _)| **d).collect();
    let count = |get: Getter| week_days.iter().filter(|(_, d)| get(d).is_some()).count() as i64;

    let rated: Vec<(NaiveDate, f64)> = week_days
        .iter()
        .filter_map(|(d, day)| day.fatigue.map(|f| (**d, f)))
        .collect();
    let bad_days = rated.iter().filter(|(_, f)| *f >= 8.0).count() as i64;
    let good_days = rated.iter().filter(|(_, f)| *f <= 4.0).count() as i64;
    // Latest day wins a tie for worst, earliest for best — arbitrary but stable.
    let worst = rated.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0))).map(|(d, _)| *d);
    let best = rated.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0))).map(|(d, _)| *d);

    let exposures = exposures_between(pool, &iso(ws - Duration::days(1)), &iso(we)).await?;
    let week_exposures: Vec<String> = exposures
        .iter()
        .filter(|(d, _)| *d >= iso(ws))
        .map(|(d, e)| format!("{}: {}", d, e))
        .collect();

    let (food, food_days) = build_food(pool, ws, we, worst.map(iso).as_deref()).await?;
    let meds = build_meds(pool, ws, we, &logged_days).await?;

    let activity_by_category: Vec<(String, f64)> = sqlx::query_as(
        "SELECT ac.name, CAST(COALESCE(SUM(al.duration_hours), 0) AS REAL) AS hours
         FROM activity_log al
         JOIN activity_types at ON al.activity_type_id = at.id
         JOIN activity_categories ac ON at.category_id = ac.id
         WHERE al.log_date >= ? AND al.log_date <= ?
         GROUP BY ac.name ORDER BY hours DESC",
    )
    .bind(iso(ws)).bind(iso(we))
    .fetch_all(pool).await.map_err(|e| format!("DB error activity: {}", e))?;

    // Labs already reported by the previous summary stay out of this one.
    let prev_json: Option<(String,)> = sqlx::query_as(
        "SELECT metrics_json FROM weekly_summaries WHERE week_start < ? ORDER BY week_start DESC LIMIT 1",
    )
    .bind(iso(ws))
    .fetch_optional(pool).await.map_err(|e| format!("DB error previous summary: {}", e))?;
    let mut reported: Vec<String> = prev_json
        .and_then(|(j,)| serde_json::from_str::<WeekMetrics>(&j).ok())
        .map(|m| m.reported_lab_keys)
        .unwrap_or_default();
    let already: HashSet<String> = reported.iter().cloned().collect();
    let new_lab_results = build_labs(pool, we, &already).await?;
    reported.extend(new_lab_results.iter().map(|l| lab_key(&l.test, &l.date, &l.source)));

    let (correlations, correlations_tested) = build_correlations(&days, we);

    Ok(WeekMetrics {
        week_start: iso(ws),
        week_end: iso(we),
        completeness: Completeness {
            days_in_week: week_days.len() as i64,
            fatigue_days: count(|d| d.fatigue),
            sleep_days: count(|d| d.sleep),
            steps_days: count(|d| d.steps),
            medication_days: meds.days_with_dose,
            food_days,
            activity_days: week_days.iter().filter(|(_, d)| d.load_hours.unwrap_or(0.0) > 0.0).count() as i64,
        },
        scorecard: build_scorecard(&days, ws),
        bad_days,
        good_days,
        worst_day: worst.and_then(|d| day_note(d, &days, &exposures)),
        best_day: best.and_then(|d| day_note(d, &days, &exposures)),
        activity_by_category,
        meds,
        food,
        exposures: week_exposures,
        new_lab_results,
        vault_changes: vault_changes(ws, we),
        correlations,
        correlations_tested,
        reported_lab_keys: reported,
    })
}

// ── Narrative ──

fn build_prompt(m: &WeekMetrics) -> String {
    let mut for_model = m.clone();
    for_model.reported_lab_keys.clear();
    for lab in &mut for_model.new_lab_results {
        lab.source.clear();
    }
    let data = serde_json::to_string_pretty(&for_model).unwrap_or_default();
    format!(
        r#"You are writing a weekly summary of one person's ME/CFS (chronic fatigue) health log, for them to read first thing on Monday morning. Higher fatigue / headache ratings are worse; the sleep score is better when higher. Weeks run Monday to Sunday: {start} to {end}.

Everything below is computed from their own log. Use ONLY the figures given — never invent, estimate or round to a different value. If something isn't in the data, don't mention it.

How to read the data:
- scorecard: this_week vs last_week and "baseline" (the mean of their previous {bw} weekly means). flag is "higher"/"lower" when this week sits at least one standard deviation from that baseline; strong means two or more. Only call something unusual when it is flagged; otherwise it is within their normal range. `history` is the weekly means, oldest first, ending with this week.
- completeness: how many of the week's days had each kind of data. If a metric has few days, say the picture is partial instead of drawing conclusions.
- meds.adherence: "missed" is INFERRED from the current schedule minus logged doses (the app has no explicit skipped-dose record) — word it as "no dose was logged", not "you skipped". days_without_any_dose are days where nothing was logged at all; treat them as unknown.
- meds.changes are starts, stops and dose changes this week.
- correlations: Pearson r of fatigue against a measure over the last {cd} days (lag 0 = same day, lag 1 = the measure from the day before). Only pairs with |r| >= 0.3 are listed, out of correlations_tested. Describe them tentatively with n, as associations, never as causes.
- new_lab_results: results newly noticed in the records vault (from the last lab extraction); vault_changes: notes added or edited this week (titles only). If vault_changes contains a pathology note but new_lab_results is empty, say the results may not have been extracted yet (Records page → Labs).
- exposures and food are things they logged; mention them descriptively only.

Important: in this person's log, exertion (steps, activity hours, activity load) has shown no measurable correlation with the NEXT day's fatigue, and low activity tends to FOLLOW a bad day rather than precede one. Do not claim that an activity level caused or predicts a crash. Do not forecast next week. Do not diagnose or recommend treatment changes; for medication or results, suggest raising things with their clinician rather than acting.

Write supportively and plainly, in the second person. Keep each point to one or two sentences with the relevant figures. Leave out any section that has nothing worth saying.

Respond with JSON only (no markdown fences), exactly this shape. severity is one of "positive", "neutral", "warning", "critical"; icon is a single emoji.
{{
  "headline": "One sentence capturing the week",
  "overview": "Two to four sentences: how the week compared with their usual",
  "sections": [
    {{"title": "Fatigue & symptoms", "icon": "🔋", "severity": "neutral", "points": ["..."]}},
    {{"title": "Sleep & rest", "icon": "🌙", "severity": "neutral", "points": ["..."]}},
    {{"title": "Activity & pacing", "icon": "📈", "severity": "neutral", "points": ["..."]}},
    {{"title": "Food & exposures", "icon": "🍽️", "severity": "neutral", "points": ["..."]}},
    {{"title": "Medications", "icon": "💊", "severity": "neutral", "points": ["..."]}},
    {{"title": "Records & results", "icon": "🧪", "severity": "neutral", "points": ["..."]}},
    {{"title": "Trends & correlations", "icon": "🔍", "severity": "neutral", "points": ["..."]}}
  ],
  "to_raise": ["Zero to three things worth mentioning to a clinician, only if the data supports them"]
}}

Data:
{data}"#,
        start = m.week_start,
        end = m.week_end,
        bw = BASELINE_WEEKS,
        cd = CORR_DAYS,
        data = data,
    )
}

async fn call_model(api_key: &str, m: &WeekMetrics) -> Result<Narrative, String> {
    let content = call_openrouter(api_key, &build_prompt(m), 0.3, 6000).await?;
    let cleaned = strip_code_fences(&content);
    serde_json::from_str(cleaned).map_err(|e| format!("Failed to parse AI response: {} - {}", e, cleaned))
}

// ── Storage + commands ──

static GENERATING: AtomicBool = AtomicBool::new(false);

/// Holds the single-flight flag; released on drop so an early `?` can't leave it set.
struct Busy;
impl Busy {
    fn acquire() -> Option<Busy> {
        GENERATING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).ok().map(|_| Busy)
    }
}
impl Drop for Busy {
    fn drop(&mut self) {
        GENERATING.store(false, Ordering::SeqCst);
    }
}

/// Monday of the most recent *complete* week.
fn last_full_week_start() -> NaiveDate {
    let today = Local::now().date_naive();
    today - Duration::days(today.weekday().num_days_from_monday() as i64 + 7)
}

fn parse_week(week_start: &str) -> Result<NaiveDate, String> {
    let d = NaiveDate::parse_from_str(week_start, "%Y-%m-%d").map_err(|_| "Bad week date".to_string())?;
    if d.weekday().num_days_from_monday() != 0 {
        return Err("A week must start on a Monday".to_string());
    }
    Ok(d)
}

async fn require_key() -> Result<String, String> {
    settings::get_api_key()
        .await?
        .ok_or_else(|| "OpenRouter API key not configured. Please add your API key in Settings.".to_string())
}

async fn fetch_one(pool: &SqlitePool, week_start: &str) -> Result<Option<WeeklySummary>, String> {
    let row: Option<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT week_start, week_end, generated_at, metrics_json, narrative_json, seen
         FROM weekly_summaries WHERE week_start = ?",
    )
    .bind(week_start)
    .fetch_optional(pool).await.map_err(|e| format!("DB error weekly: {}", e))?;
    row.map(row_to_summary).transpose()
}

fn row_to_summary(r: (String, String, String, String, String, i64)) -> Result<WeeklySummary, String> {
    Ok(WeeklySummary {
        week_start: r.0,
        week_end: r.1,
        generated_at: r.2,
        metrics: serde_json::from_str(&r.3).map_err(|e| format!("Bad stored metrics: {}", e))?,
        narrative: serde_json::from_str(&r.4).map_err(|e| format!("Bad stored narrative: {}", e))?,
        seen: r.5 != 0,
    })
}

/// Build, narrate and store one week. Refuses a week with no log entries.
async fn generate(pool: &SqlitePool, ws: NaiveDate) -> Result<WeeklySummary, String> {
    let _busy = Busy::acquire().ok_or_else(|| "A weekly summary is already being generated.".to_string())?;
    let api_key = require_key().await?;
    let metrics = build_metrics(pool, ws).await?;
    if metrics.completeness.days_in_week == 0 {
        return Err("Nothing was logged in that week.".to_string());
    }
    let narrative = call_model(&api_key, &metrics).await?;
    let generated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let metrics_json = serde_json::to_string(&metrics).map_err(|e| e.to_string())?;
    let narrative_json = serde_json::to_string(&narrative).map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO weekly_summaries (week_start, week_end, generated_at, model, metrics_json, narrative_json)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(week_start) DO UPDATE SET
           week_end = excluded.week_end, generated_at = excluded.generated_at, model = excluded.model,
           metrics_json = excluded.metrics_json, narrative_json = excluded.narrative_json",
    )
    .bind(&metrics.week_start).bind(&metrics.week_end).bind(&generated_at)
    .bind(settings::model()).bind(&metrics_json).bind(&narrative_json)
    .execute(pool).await.map_err(|e| format!("DB error save weekly: {}", e))?;
    fetch_one(pool, &metrics.week_start).await?.ok_or_else(|| "Saved summary vanished".to_string())
}

/// Called on launch: make the last full week's summary if it doesn't exist yet.
/// Returns the banner info either way (None when there is nothing to show).
#[tauri::command]
pub async fn ensure_weekly_summary(pool: State<'_, SqlitePool>) -> Result<Option<WeeklyBanner>, String> {
    let ws = last_full_week_start();
    let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM weekly_summaries WHERE week_start = ?")
        .bind(iso(ws))
        .fetch_optional(&*pool).await.map_err(|e| format!("DB error weekly: {}", e))?;
    if exists.is_none() {
        // No key yet, or nothing logged that week: nothing to do, and not an error.
        if settings::get_api_key().await?.is_none() {
            return get_weekly_banner(pool).await;
        }
        match generate(&pool, ws).await {
            Ok(_) => {}
            Err(e) if e == "Nothing was logged in that week." => {}
            Err(e) => return Err(e),
        }
    }
    get_weekly_banner(pool).await
}

/// Make (or remake) the summary for a chosen Monday.
#[tauri::command]
pub async fn generate_weekly_summary(
    pool: State<'_, SqlitePool>,
    week_start: String,
) -> Result<WeeklySummary, String> {
    generate(&pool, parse_week(&week_start)?).await
}

/// The most recent stored summary, for the Dashboard banner and sidebar dot.
#[tauri::command]
pub async fn get_weekly_banner(pool: State<'_, SqlitePool>) -> Result<Option<WeeklyBanner>, String> {
    sqlx::query_as::<_, WeeklyBanner>(
        "SELECT week_start, week_end, seen != 0 AS seen FROM weekly_summaries ORDER BY week_start DESC LIMIT 1",
    )
    .fetch_optional(&*pool).await.map_err(|e| format!("DB error banner: {}", e))
}

/// Every stored summary, newest first.
#[tauri::command]
pub async fn list_weekly_summaries(pool: State<'_, SqlitePool>) -> Result<Vec<WeeklySummary>, String> {
    let rows: Vec<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT week_start, week_end, generated_at, metrics_json, narrative_json, seen
         FROM weekly_summaries ORDER BY week_start DESC",
    )
    .fetch_all(&*pool).await.map_err(|e| format!("DB error weekly list: {}", e))?;
    rows.into_iter().map(row_to_summary).collect()
}

#[tauri::command]
pub async fn mark_weekly_seen(pool: State<'_, SqlitePool>, week_start: String) -> Result<(), String> {
    sqlx::query("UPDATE weekly_summaries SET seen = 1 WHERE week_start = ?")
        .bind(week_start)
        .execute(&*pool).await.map_err(|e| format!("DB error mark seen: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn pearson_matches_a_known_perfect_line() {
        let r = pearson(&[(1.0, 2.0), (2.0, 4.0), (3.0, 6.0), (4.0, 8.0)]).unwrap();
        assert!((r - 1.0).abs() < 1e-9);
        assert!(pearson(&[(1.0, 5.0), (2.0, 5.0), (3.0, 5.0)]).is_none());
    }

    #[test]
    fn rejects_a_week_that_is_not_a_monday() {
        assert!(parse_week("2026-09-21").is_ok()); // Monday
        assert!(parse_week("2026-09-22").is_err());
    }

    #[tokio::test]
    async fn builds_metrics_for_a_week_with_a_flagged_metric() {
        let pool = pool().await;
        // 8 baseline weeks of steady fatigue ~4 (small wobble), then a week at 8.
        let ws = d("2026-09-21");
        for i in 0..(7 * 9) {
            let date = ws - Duration::days(7 * 8) + Duration::days(i);
            let in_week = date >= ws;
            let fatigue = if in_week { 8.0 } else { 4.0 + (i % 3) as f64 * 0.3 };
            sqlx::query("INSERT INTO daily_logs (log_date, fatigue_rating, sleep_avg, steps) VALUES (?, ?, 6, 5000)")
                .bind(iso(date)).bind(fatigue).execute(&pool).await.unwrap();
        }
        // Meds: Dexamphetamine has 3 slots; log 2 doses on Monday of the week, none Tuesday
        // (a day with no doses at all is unknown, not missed).
        let med: (i64,) = sqlx::query_as("SELECT id FROM medications WHERE name = 'Dexamphetamine'")
            .fetch_one(&pool).await.unwrap();
        for t in ["07:00", "11:00"] {
            sqlx::query("INSERT INTO medication_doses (medication_id, log_date, time_taken, dose_amount) VALUES (?, ?, ?, 10)")
                .bind(med.0).bind(iso(ws)).bind(t).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO exposures (log_date, description) VALUES (?, 'Paint fumes')")
            .bind(iso(ws + Duration::days(2))).execute(&pool).await.unwrap();

        let m = build_metrics(&pool, ws).await.unwrap();
        assert_eq!(m.week_end, "2026-09-27");
        assert_eq!(m.completeness.days_in_week, 7);
        assert_eq!(m.bad_days, 7);

        let fatigue = m.scorecard.iter().find(|r| r.key == "fatigue").unwrap();
        assert_eq!(fatigue.this_week, Some(8.0));
        assert_eq!(fatigue.flag.as_deref(), Some("higher"));
        assert!(fatigue.strong);
        assert_eq!(fatigue.history.len(), 9);
        // Steps are constant, so there is no spread to measure against — never flagged.
        assert!(m.scorecard.iter().find(|r| r.key == "steps").unwrap().flag.is_none());

        let dex = m.meds.adherence.iter().find(|a| a.name == "Dexamphetamine").unwrap();
        assert_eq!((dex.expected_doses, dex.logged_doses, dex.missed_doses), (3, 2, 1));
        assert_eq!(m.meds.days_without_any_dose.len(), 6);
        assert_eq!(m.completeness.medication_days, 1);
        assert_eq!(m.exposures, vec!["2026-09-23: Paint fumes".to_string()]);
    }

    #[tokio::test]
    async fn only_new_lab_results_are_reported_and_a_week_with_no_log_is_refused() {
        let pool = pool().await;
        sqlx::query("INSERT INTO lab_results (test_name, result_date, value_num, value_text, unit, source_note, extracted_at)
                     VALUES ('Ferritin', '2026-09-24', 80, '80', 'ug/L', 'Pathology/a.md', '2026-09-28')")
            .execute(&pool).await.unwrap();
        let ws = d("2026-09-21");
        let first = build_metrics(&pool, ws).await.unwrap();
        assert_eq!(first.new_lab_results.len(), 1);
        assert_eq!(first.completeness.days_in_week, 0);

        // Store it as the previous summary: the next week must not repeat the result.
        sqlx::query("INSERT INTO weekly_summaries (week_start, week_end, generated_at, metrics_json, narrative_json)
                     VALUES ('2026-09-21', '2026-09-27', 'x', ?, '{}')")
            .bind(serde_json::to_string(&first).unwrap()).execute(&pool).await.unwrap();
        let next = build_metrics(&pool, ws + Duration::days(7)).await.unwrap();
        assert!(next.new_lab_results.is_empty());
    }
}
