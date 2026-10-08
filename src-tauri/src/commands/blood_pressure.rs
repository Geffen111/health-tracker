use crate::models::BloodPressure;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use tauri::State;

#[derive(Serialize, sqlx::FromRow)]
pub struct BpDailyAvg {
    pub log_date: String,
    pub avg_systolic: Option<f64>,
    pub avg_diastolic: Option<f64>,
}

/// Daily-averaged blood pressure over the last `days` days, oldest first —
/// feeds the Cardio history chart.
#[tauri::command]
pub async fn get_bp_history(
    pool: State<'_, SqlitePool>,
    days: i64,
) -> Result<Vec<BpDailyAvg>, String> {
    sqlx::query_as::<_, BpDailyAvg>(
        "SELECT log_date, \
                AVG(systolic) AS avg_systolic, \
                AVG(diastolic) AS avg_diastolic \
         FROM blood_pressure \
         WHERE systolic IS NOT NULL AND diastolic IS NOT NULL \
           AND log_date >= date('now', ?) \
         GROUP BY log_date \
         ORDER BY log_date",
    )
    .bind(format!("-{} days", days))
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

// ── Calibration adjustment ──
//
// The watch doesn't measure BP absolutely: it is calibrated against a cuff, and the cuff
// is itself only accurate to a few mmHg. So each calibration moves every watch reading
// after it up or down by a roughly constant amount — a step that is the cuff's error on
// that day, not a change in BP.
//
// This removes those steps (a level-shift / "homogenisation" adjustment):
//  1. Readings are split into periods at each calibration (date + time).
//  2. At each calibration, the step is the median of the first STEP_WINDOW days after it
//     minus the median of the last STEP_WINDOW days before it (already adjusted). Medians,
//     so one odd day doesn't set the step. With fewer than MIN_DAYS either side there's no
//     estimate and the period keeps the previous offset.
//  3. Each period's offset is the running total of the steps. All offsets are then shifted
//     so the adjusted readings keep the same overall mean as the raw ones: no single
//     calibration is treated as the true one — the level is the average of all of them.
//
// Trends *within* a period are untouched, so a gradual real change survives; a genuine
// change exactly at a calibration would be removed with the step. Only `source = 'watch'`
// readings move: a cuff reading is a measurement in its own right. Display/analysis only —
// the stored readings are never changed. Used by the Cardio chart and the weekly summary.

const STEP_WINDOW: usize = 7;
const MIN_DAYS: usize = 3;
const WATCH: &str = "watch";

/// One reading. `systolic`/`diastolic` are REAL so an adjusted copy can carry fractions.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BpPoint {
    pub log_date: String,
    pub time_taken: Option<String>,
    pub systolic: f64,
    pub diastolic: f64,
    pub source: Option<String>,
}

/// One calibration period and what was added to its watch readings (mmHg).
#[derive(Debug, Serialize)]
pub struct BpPeriod {
    /// Date of the calibration that opened the period; None before the first one.
    pub from: Option<String>,
    pub sys: f64,
    pub dia: f64,
    /// Days with watch readings in this period.
    pub days: i64,
}

#[derive(Debug, Serialize)]
pub struct BpSeries {
    /// Every reading as measured, oldest first.
    pub raw: Vec<BpPoint>,
    /// The same readings with the watch ones adjusted for calibration.
    pub adjusted: Vec<BpPoint>,
    pub periods: Vec<BpPeriod>,
}

fn stamp(date: &str, time: Option<&str>) -> String {
    format!("{} {}", date, time.unwrap_or("00:00"))
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.total_cmp(b));
    let m = xs.len() / 2;
    if xs.len() % 2 == 1 { xs[m] } else { (xs[m - 1] + xs[m]) / 2.0 }
}

/// `cuts` are calibration stamps ("YYYY-MM-DD HH:MM"), sorted.
fn adjust_for_calibration(readings: &[BpPoint], cuts: &[String]) -> (Vec<BpPoint>, Vec<BpPeriod>) {
    let period_of = |r: &BpPoint| {
        let t = stamp(&r.log_date, r.time_taken.as_deref());
        cuts.iter().filter(|c| t >= **c).count()
    };
    let is_watch = |r: &BpPoint| r.source.as_deref() == Some(WATCH);

    // Daily means of the watch readings, per period (a calibration day can sit in two).
    // Keyed (period, date), so iteration is already in time order.
    let mut by_day: BTreeMap<(usize, String), (f64, f64, f64)> = BTreeMap::new();
    let mut counts = vec![0usize; cuts.len() + 1];
    for r in readings.iter().filter(|r| is_watch(r)) {
        let p = period_of(r);
        counts[p] += 1;
        let e = by_day.entry((p, r.log_date.clone())).or_insert((0.0, 0.0, 0.0));
        e.0 += r.systolic;
        e.1 += r.diastolic;
        e.2 += 1.0;
    }
    let days: Vec<(usize, f64, f64)> = by_day.iter().map(|((p, _), (s, d, n))| (*p, s / n, d / n)).collect();

    // Offsets are what a period reads above the first one; adjusted = raw - offset.
    let mut off_s = vec![0.0; cuts.len() + 1];
    let mut off_d = vec![0.0; cuts.len() + 1];
    for p in 1..=cuts.len() {
        let before: Vec<&(usize, f64, f64)> = days.iter().filter(|d| d.0 < p).collect();
        let before = &before[before.len().saturating_sub(STEP_WINDOW)..];
        let after: Vec<&(usize, f64, f64)> = days.iter().filter(|d| d.0 == p).take(STEP_WINDOW).collect();
        if before.len() >= MIN_DAYS && after.len() >= MIN_DAYS {
            off_s[p] = median(after.iter().map(|d| d.1).collect())
                - median(before.iter().map(|d| d.1 - off_s[d.0]).collect());
            off_d[p] = median(after.iter().map(|d| d.2).collect())
                - median(before.iter().map(|d| d.2 - off_d[d.0]).collect());
        } else {
            off_s[p] = off_s[p - 1];
            off_d[p] = off_d[p - 1];
        }
    }

    // Keep the overall mean: centre the offsets on their reading-weighted average.
    let n = counts.iter().sum::<usize>().max(1) as f64;
    let centre_s = off_s.iter().zip(&counts).map(|(o, c)| o * *c as f64).sum::<f64>() / n;
    let centre_d = off_d.iter().zip(&counts).map(|(o, c)| o * *c as f64).sum::<f64>() / n;
    let add_s: Vec<f64> = off_s.iter().map(|o| centre_s - o).collect();
    let add_d: Vec<f64> = off_d.iter().map(|o| centre_d - o).collect();

    let periods = (0..=cuts.len())
        .map(|p| BpPeriod {
            from: if p == 0 { None } else { Some(cuts[p - 1][..10].to_string()) },
            sys: add_s[p],
            dia: add_d[p],
            days: days.iter().filter(|d| d.0 == p).count() as i64,
        })
        .collect();
    let adjusted = readings
        .iter()
        .map(|r| {
            if !is_watch(r) {
                return r.clone();
            }
            let p = period_of(r);
            BpPoint { systolic: r.systolic + add_s[p], diastolic: r.diastolic + add_d[p], ..r.clone() }
        })
        .collect();
    (adjusted, periods)
}

/// Every reading, raw and calibration-adjusted. The whole history is needed: each step
/// is measured against the readings either side of a calibration, however far back.
pub async fn load_bp_series(pool: &SqlitePool) -> Result<BpSeries, String> {
    let raw: Vec<BpPoint> = sqlx::query_as(
        "SELECT log_date, time_taken, CAST(systolic AS REAL) AS systolic, \
                CAST(diastolic AS REAL) AS diastolic, source FROM blood_pressure \
         WHERE systolic IS NOT NULL AND diastolic IS NOT NULL \
         ORDER BY log_date, COALESCE(time_taken, '00:00'), reading_num",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error bp: {}", e))?;
    let cals: Vec<(String, Option<String>)> = sqlx::query_as("SELECT cal_date, cal_time FROM watch_calibration")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("DB error calibrations: {}", e))?;
    let mut cuts: Vec<String> = cals.iter().map(|(d, t)| stamp(d, t.as_deref())).collect();
    cuts.sort();
    let (adjusted, periods) = adjust_for_calibration(&raw, &cuts);
    Ok(BpSeries { raw, adjusted, periods })
}

#[tauri::command]
pub async fn get_bp_series(pool: State<'_, SqlitePool>) -> Result<BpSeries, String> {
    load_bp_series(&pool).await
}

#[tauri::command]
pub async fn get_bp_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<BloodPressure>, String> {
    sqlx::query_as::<_, BloodPressure>(
        // Chronological, not by reading_num: the sync appends watch readings as it
        // finds them, so an early-morning synced reading can carry a higher
        // reading_num than a manual one taken that afternoon. Undated readings last.
        "SELECT * FROM blood_pressure WHERE log_date = ?
         ORDER BY COALESCE(time_taken, '99:99'), reading_num"
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_bp(
    pool: State<'_, SqlitePool>,
    bp: BloodPressure,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO blood_pressure (log_date, reading_num, time_taken, systolic, diastolic, pulse, notes, source)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(log_date, reading_num) DO UPDATE SET
         time_taken=excluded.time_taken, systolic=excluded.systolic,
         diastolic=excluded.diastolic, pulse=excluded.pulse,
         notes=excluded.notes, source=excluded.source"
    )
    .bind(&bp.log_date).bind(bp.reading_num).bind(&bp.time_taken)
    .bind(bp.systolic).bind(bp.diastolic).bind(bp.pulse)
    .bind(&bp.notes).bind(&bp.source)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Permanently remove a single BP reading (systolic/diastolic are NOT NULL, so a
/// soft-delete by nulling them isn't possible — this deletes the row outright).
#[tauri::command]
pub async fn delete_bp(
    pool: State<'_, SqlitePool>,
    log_date: String,
    reading_num: i64,
) -> Result<(), String> {
    sqlx::query("DELETE FROM blood_pressure WHERE log_date = ? AND reading_num = ?")
        .bind(&log_date).bind(reading_num)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn reading(date: &str, sys: f64, dia: f64, source: Option<&str>) -> BpPoint {
        BpPoint {
            log_date: date.into(),
            time_taken: Some("08:00".into()),
            systolic: sys,
            diastolic: dia,
            source: source.map(Into::into),
        }
    }

    fn days(from: u32, to: u32, sys: f64, dia: f64) -> Vec<BpPoint> {
        (from..=to).map(|d| reading(&format!("2026-09-{:02}", d), sys, dia, Some("watch"))).collect()
    }

    #[test]
    fn removes_a_calibration_step_and_keeps_the_mean() {
        // 10 days at 130/80, calibration on the 11th, 10 days at 110/70.
        let mut rs = days(1, 10, 130.0, 80.0);
        rs.extend(days(11, 20, 110.0, 70.0));
        rs.push(reading("2026-09-15", 140.0, 90.0, None)); // a cuff reading: left alone
        let (adj, periods) = adjust_for_calibration(&rs, &["2026-09-11 07:00".to_string()]);
        for r in adj.iter().filter(|r| r.source.is_some()) {
            assert!((r.systolic - 120.0).abs() < 1e-9 && (r.diastolic - 75.0).abs() < 1e-9, "{:?}", r);
        }
        assert_eq!(adj.last().unwrap().systolic, 140.0);
        assert_eq!(periods.len(), 2);
        assert!((periods[0].sys + 10.0).abs() < 1e-9 && (periods[1].sys - 10.0).abs() < 1e-9);
    }

    #[test]
    fn too_few_days_after_a_calibration_leaves_it_unshifted() {
        let mut rs = days(1, 10, 130.0, 80.0);
        rs.extend(days(11, 12, 110.0, 70.0));
        let (adj, _) = adjust_for_calibration(&rs, &["2026-09-11 07:00".to_string()]);
        let got: Vec<f64> = adj.iter().map(|r| r.systolic).collect();
        let want: Vec<f64> = rs.iter().map(|r| r.systolic).collect();
        assert_eq!(got, want);
    }
}
