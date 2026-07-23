// Samsung Health → Health Sync → Google Drive CSV auto-import.
//
// Health Sync writes one CSV per day per metric into four folders under a Drive
// root. Each metric is intraday samples; we aggregate per day and COALESCE-upsert
// into daily_logs so manually-entered fields are never clobbered. A sleep session
// crosses midnight, so it's attributed to the WAKE day (the date of its last row).
//
// "On each sync a file is (re)created for that day", so files are reprocessed when
// their modification time is newer than the last successful sync (or when `full`).
//
// The per-metric aggregation is pure (agg_steps/agg_hr/agg_energy/agg_sleep) and
// unit-tested at the bottom; the command just does file IO, merging and upsert.

use crate::commands::settings;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::State;

const DEFAULT_ROOT: &str = "G:\\My Drive";
const STEPS_DIR: &str = "Health Sync Steps";
const HR_DIR: &str = "Health Sync Heart rate";
const ENERGY_DIR: &str = "Health Sync Energy burned";
const SLEEP_DIR: &str = "Health Sync Sleep";

#[derive(Serialize)]
pub struct CsvImportResult {
    pub files_processed: i64,
    pub files_skipped: i64,
    pub days_updated: i64,
    pub steps_days: i64,
    pub hr_days: i64,
    pub sleep_days: i64,
    pub energy_days: i64,
    pub errors: Vec<String>,
    pub last_sync: String,
}

#[derive(Default)]
struct DayAgg {
    steps: Option<i64>,
    activity_calories: Option<f64>,
    ave_hr: Option<i64>,
    hr_min: Option<i64>,
    hr_max: Option<i64>,
    sleep_actual_asleep: Option<f64>,
    sleep_rem: Option<f64>,
    sleep_deep: Option<f64>,
    sleep_awake: Option<f64>,
    sleep_time_head_on_pillow: Option<f64>,
}

/// Per-day HR, bucketed by minute. A workout is sampled every second, so a single
/// exercise hour can hold thousands of high-bpm rows; averaging per sample lets that
/// burst dominate and pushes the "daily average" up to workout levels. Bucketing by
/// minute first (one vote per minute) makes the daily average time-weighted instead.
struct DayHr {
    minute: HashMap<String, (i64, i64)>, // "HH:MM" -> (bpm sum, sample count)
    min: i64,
    max: i64,
}

impl DayHr {
    fn new() -> Self {
        DayHr { minute: HashMap::new(), min: i64::MAX, max: i64::MIN }
    }
}

/// Import (or re-import) the Health Sync CSVs. `full = true` ignores the
/// last-sync modification-time filter and reprocesses every file.
#[tauri::command]
pub async fn import_health_csv(
    pool: State<'_, SqlitePool>,
    root: Option<String>,
    full: bool,
) -> Result<CsvImportResult, String> {
    let root = root
        .filter(|s| !s.trim().is_empty())
        .or_else(|| settings::setting_str("csv_root"))
        .unwrap_or_else(|| DEFAULT_ROOT.to_string());
    let root = PathBuf::from(root);
    if !root.exists() {
        return Err(format!("CSV root folder not found: {}", root.display()));
    }

    let last_sync_unix = if full { None } else { settings::setting_i64("last_sync_unix") };
    let mut errors: Vec<String> = Vec::new();
    let mut files_processed = 0i64;
    let mut files_skipped = 0i64;

    // Cross-file per-date accumulators.
    let mut steps: HashMap<String, i64> = HashMap::new();
    let mut energy: HashMap<String, f64> = HashMap::new();
    let mut hr: HashMap<String, DayHr> = HashMap::new(); // per-minute buckets + min/max
    let mut sleep: HashMap<String, (f64, f64, f64, f64)> = HashMap::new(); // asleep, rem, deep, awake (secs)

    // ── Steps ──
    for path in &collect_files(&root.join(STEPS_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((h, recs)) => match agg_steps(&h, &recs) {
                Ok(m) => {
                    files_processed += 1;
                    // Daily totals, not increments — overlapping files (e.g. a
                    // monthly range plus a single-day export) report the same
                    // day, so take the max rather than summing.
                    for (d, n) in m {
                        let e = steps.entry(d).or_insert(0);
                        if n > *e {
                            *e = n;
                        }
                    }
                }
                Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
            },
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // ── Energy burned (active calories) ──
    for path in &collect_files(&root.join(ENERGY_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((h, recs)) => match agg_energy(&h, &recs) {
                Ok(m) => {
                    files_processed += 1;
                    for (d, c) in m {
                        *energy.entry(d).or_insert(0.0) += c;
                    }
                }
                Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
            },
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // ── Heart rate ──
    for path in &collect_files(&root.join(HR_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((h, recs)) => match agg_hr(&h, &recs) {
                Ok(m) => {
                    files_processed += 1;
                    // The HR folder holds overlapping 30-day exports, so the same
                    // minute recurs across files; merging per-minute sums keeps the
                    // per-minute mean (and the daily average) stable regardless.
                    for (d, dh) in m {
                        let e = hr.entry(d).or_insert_with(DayHr::new);
                        for (minute, (s, c)) in dh.minute {
                            let me = e.minute.entry(minute).or_insert((0, 0));
                            me.0 += s;
                            me.1 += c;
                        }
                        e.min = e.min.min(dh.min);
                        e.max = e.max.max(dh.max);
                    }
                }
                Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
            },
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // ── Sleep (attributed to wake day) ──
    for path in &collect_files(&root.join(SLEEP_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((h, recs)) => match agg_sleep(&h, &recs) {
                Ok(m) => {
                    files_processed += 1;
                    // A single night recurs across overlapping exports — a 30-day range
                    // file and the per-day file for that morning carry the same session
                    // (or one is a partial fragment). Take the fullest reading per stage
                    // rather than summing, which would double-count the overlap.
                    for (d, (a, r, dp, aw)) in m {
                        let e = sleep.entry(d).or_insert((0.0, 0.0, 0.0, 0.0));
                        e.0 = e.0.max(a);
                        e.1 = e.1.max(r);
                        e.2 = e.2.max(dp);
                        e.3 = e.3.max(aw);
                    }
                }
                Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
            },
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // A 0.0 active-calorie total means Health Sync exported the day with no real
    // data (the source app wasn't recording active energy). Drop those so we don't
    // write a misleading zero — or, via the import's COALESCE, clobber a value that
    // came from another source — for a day that simply has no calorie reading.
    energy.retain(|_, c| *c > 0.0);

    // ── Merge into per-day aggregates ──
    let mut days: HashMap<String, DayAgg> = HashMap::new();
    let steps_days = steps.len() as i64;
    let energy_days = energy.len() as i64;
    let hr_days = hr.len() as i64;
    let sleep_days = sleep.len() as i64;

    for (date, n) in steps {
        days.entry(date).or_default().steps = Some(n);
    }
    for (date, c) in energy {
        days.entry(date).or_default().activity_calories = Some(round1(c));
    }
    for (date, dh) in hr {
        if let Some(ave) = day_ave_hr(&dh) {
            let a = days.entry(date).or_default();
            a.ave_hr = Some(ave);
            a.hr_min = Some(dh.min);
            a.hr_max = Some(dh.max);
        }
    }
    for (date, (asleep, rem, deep, awake)) in sleep {
        let a = days.entry(date).or_default();
        a.sleep_actual_asleep = Some(hours(asleep));
        a.sleep_rem = Some(hours(rem));
        a.sleep_deep = Some(hours(deep));
        a.sleep_awake = Some(hours(awake));
        a.sleep_time_head_on_pillow = Some(hours(asleep + awake));
    }

    // ── Upsert ──
    let days_updated = days.len() as i64;
    for (date, a) in &days {
        if let Err(e) = upsert_day(&pool, date, a).await {
            errors.push(format!("upsert {}: {}", date, e));
        }
    }

    // ── Record sync time ──
    let now = chrono::Local::now();
    let last_sync = now.format("%Y-%m-%d %H:%M").to_string();
    let _ = settings::put_setting("last_sync", serde_json::json!(last_sync));
    let _ = settings::put_setting("last_sync_unix", serde_json::json!(now.timestamp()));

    Ok(CsvImportResult {
        files_processed,
        files_skipped,
        days_updated,
        steps_days,
        hr_days,
        sleep_days,
        energy_days,
        errors,
        last_sync,
    })
}

// ── Pure per-metric aggregation (unit-tested) ──

/// Steps per day. Samsung Health writes one daily-total row per day at 00:00:00
/// (its official count, source `com.sec.android.app.shealth`); Health Connect
/// adds per-interval rows. Summing both double-counts, so prefer the daily total
/// and fall back to summing the granular rows on days with no midnight total.
fn agg_steps(headers: &csv::StringRecord, records: &[csv::StringRecord]) -> Result<HashMap<String, i64>, &'static str> {
    let di = col(headers, "Date").ok_or("missing Date column")?;
    let si = col(headers, "Steps").ok_or("missing Steps column")?;
    let mut snap: HashMap<String, i64> = HashMap::new();
    let mut gran: HashMap<String, i64> = HashMap::new();
    for r in records {
        let cell = match r.get(di) { Some(c) => c, None => continue };
        let date = match date_part(cell) { Some(d) => d, None => continue };
        let n = match r.get(si).and_then(parse_i64) { Some(v) => v, None => continue };
        if cell.split_whitespace().nth(1) == Some("00:00:00") {
            *snap.entry(date).or_insert(0) += n;
        } else {
            *gran.entry(date).or_insert(0) += n;
        }
    }
    let mut out: HashMap<String, i64> = gran;
    for (d, v) in snap {
        if v > 0 {
            out.insert(d, v); // the daily total supersedes the granular sum
        }
    }
    Ok(out)
}

/// SUM active calories per day.
fn agg_energy(headers: &csv::StringRecord, records: &[csv::StringRecord]) -> Result<HashMap<String, f64>, &'static str> {
    let di = col(headers, "Date").ok_or("missing Date column")?;
    let ai = col(headers, "Active calories").ok_or("missing Active calories column")?;
    let mut out: HashMap<String, f64> = HashMap::new();
    for r in records {
        if let (Some(date), Some(c)) = (r.get(di).and_then(date_part), r.get(ai).and_then(parse_f64)) {
            *out.entry(date).or_insert(0.0) += c;
        }
    }
    Ok(out)
}

/// Per day: HR bucketed by minute, plus the day's true min/max bpm.
fn agg_hr(headers: &csv::StringRecord, records: &[csv::StringRecord]) -> Result<HashMap<String, DayHr>, &'static str> {
    let di = col(headers, "Date").ok_or("missing Date column")?;
    let hi = col(headers, "Heart rate").ok_or("missing Heart rate column")?;
    let mut out: HashMap<String, DayHr> = HashMap::new();
    for r in records {
        if let (Some(date), Some(minute), Some(bpm)) = (
            r.get(di).and_then(date_part),
            r.get(di).and_then(minute_part),
            r.get(hi).and_then(parse_i64),
        ) {
            let e = out.entry(date).or_insert_with(DayHr::new);
            let m = e.minute.entry(minute).or_insert((0, 0));
            m.0 += bpm;
            m.1 += 1;
            e.min = e.min.min(bpm);
            e.max = e.max.max(bpm);
        }
    }
    Ok(out)
}

/// Time-weighted daily average HR: average each minute's samples, then average those
/// per-minute means (one vote per minute) so a second-by-second workout burst counts
/// as a single minute rather than thousands of samples. `None` if no samples.
fn day_ave_hr(dh: &DayHr) -> Option<i64> {
    if dh.minute.is_empty() {
        return None;
    }
    let n = dh.minute.len() as f64;
    let sum: f64 = dh
        .minute
        .values()
        .map(|(s, c)| if *c > 0 { *s as f64 / *c as f64 } else { 0.0 })
        .sum();
    Some((sum / n).round() as i64)
}

/// A gap this long (seconds) between consecutive rows starts a new sleep session.
/// Within a night, rows are back-to-back (each row's start follows the previous by
/// its own duration — minutes, not hours); the daytime gap between nights is many
/// hours. 3h cleanly separates nights without splitting on in-night wake periods.
const SLEEP_SESSION_GAP_SECS: i64 = 3 * 3600;

/// Sum sleep-stage seconds per session, each attributed to its own wake day (the
/// date of the session's latest row). Returns a map keyed by wake day →
/// (asleep, rem, deep, awake) seconds.
///
/// A single file may hold one night (the usual per-day Health Connect export) or a
/// whole month (a range export). We must NOT collapse every row onto one date, or a
/// month's sleep lands on a single day and the other nights read blank; instead we
/// split the rows into sessions on large time gaps and attribute each to its own day.
fn agg_sleep(headers: &csv::StringRecord, records: &[csv::StringRecord]) -> Result<HashMap<String, (f64, f64, f64, f64)>, &'static str> {
    let di = col(headers, "Date").ok_or("missing Date column")?;
    let dur = col(headers, "Duration in seconds").ok_or("missing Duration in seconds column")?;
    let st = col(headers, "Sleep stage").ok_or("missing Sleep stage column")?;
    let mut out: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();

    // Rows that carry a parseable timestamp, sorted chronologically. (The "YYYY.MM.DD
    // HH:MM:SS" format sorts lexically = chronologically, so we sort on the raw string
    // and only parse to seconds when measuring the gap between rows.)
    let mut rows: Vec<(&str, f64, Option<String>)> = records
        .iter()
        .filter_map(|r| {
            let ts = r.get(di)?;
            let secs = r.get(dur).and_then(parse_f64).unwrap_or(0.0);
            let stage = r.get(st).map(|s| s.trim().to_lowercase());
            Some((ts, secs, stage))
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));

    // Walk the rows, breaking into sessions whenever the gap since the previous row
    // exceeds the threshold. Flush each finished session onto its wake day.
    let mut session: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    let mut session_wake: Option<String> = None;
    let mut prev_ts: Option<i64> = None;

    let flush = |out: &mut HashMap<String, (f64, f64, f64, f64)>,
                 wake: &mut Option<String>,
                 s: &mut (f64, f64, f64, f64)| {
        if let Some(day) = wake.take() {
            let e = out.entry(day).or_insert((0.0, 0.0, 0.0, 0.0));
            e.0 += s.0;
            e.1 += s.1;
            e.2 += s.2;
            e.3 += s.3;
        }
        *s = (0.0, 0.0, 0.0, 0.0);
    };

    for (ts, secs, stage) in rows {
        let epoch = parse_ts_secs(ts);
        if let (Some(cur), Some(prev)) = (epoch, prev_ts) {
            if cur - prev > SLEEP_SESSION_GAP_SECS {
                flush(&mut out, &mut session_wake, &mut session);
            }
        }
        prev_ts = epoch.or(prev_ts);

        match stage.as_deref() {
            Some("rem") => { session.0 += secs; session.1 += secs; }
            Some("deep") => { session.0 += secs; session.2 += secs; }
            Some("light") | Some("sleeping") => { session.0 += secs; }
            Some("awake") | Some("awake_in_bed") => { session.3 += secs; }
            _ => {}
        }
        // The wake day is the date of the session's latest row; rows are sorted, so
        // each successive row's date is the running wake day for this session.
        if let Some(d) = date_part(ts) {
            session_wake = Some(d);
        }
    }
    flush(&mut out, &mut session_wake, &mut session);

    Ok(out)
}

async fn upsert_day(pool: &SqlitePool, date: &str, a: &DayAgg) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO daily_logs (log_date, steps, activity_calories, ave_hr, hr_min, hr_max,
            sleep_actual_asleep, sleep_rem, sleep_deep, sleep_awake, sleep_time_head_on_pillow, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(log_date) DO UPDATE SET
            steps=COALESCE(excluded.steps, daily_logs.steps),
            activity_calories=COALESCE(excluded.activity_calories, daily_logs.activity_calories),
            ave_hr=COALESCE(excluded.ave_hr, daily_logs.ave_hr),
            hr_min=COALESCE(excluded.hr_min, daily_logs.hr_min),
            hr_max=COALESCE(excluded.hr_max, daily_logs.hr_max),
            sleep_actual_asleep=COALESCE(excluded.sleep_actual_asleep, daily_logs.sleep_actual_asleep),
            sleep_rem=COALESCE(excluded.sleep_rem, daily_logs.sleep_rem),
            sleep_deep=COALESCE(excluded.sleep_deep, daily_logs.sleep_deep),
            sleep_awake=COALESCE(excluded.sleep_awake, daily_logs.sleep_awake),
            sleep_time_head_on_pillow=COALESCE(excluded.sleep_time_head_on_pillow, daily_logs.sleep_time_head_on_pillow),
            updated_at=datetime('now')",
    )
    .bind(date)
    .bind(a.steps)
    .bind(a.activity_calories)
    .bind(a.ave_hr)
    .bind(a.hr_min)
    .bind(a.hr_max)
    .bind(a.sleep_actual_asleep)
    .bind(a.sleep_rem)
    .bind(a.sleep_deep)
    .bind(a.sleep_awake)
    .bind(a.sleep_time_head_on_pillow)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Checks if a file path belongs to a Samsung Health export (e.g. contains "Samsung Health").
fn is_samsung_health_file(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(s) => s.to_lowercase(),
        None => return false,
    };
    name.contains("samsung health") || name.contains("samsung-health") || name.contains("samsunghealth")
}

/// CSV files in `dir` (only Samsung Health files, optionally filtered by `last_sync_unix`).
fn collect_files(dir: &Path, last_sync_unix: Option<i64>, skipped: &mut i64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out, // folder missing → nothing to import from it
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_csv = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("csv"))
            .unwrap_or(false);
        if !is_csv {
            continue;
        }

        // Only process files exported from Samsung Health, skipping Health Connect and other files.
        if !is_samsung_health_file(&path) {
            *skipped += 1;
            continue;
        }

        if let Some(ls) = last_sync_unix {
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            if let Some(mt) = mtime {
                if mt <= ls {
                    *skipped += 1;
                    continue;
                }
            }
        }
        out.push(path);
    }
    out
}

fn read_csv(path: &Path) -> Result<(csv::StringRecord, Vec<csv::StringRecord>), String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|e| e.to_string())?;
    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
    let records: Vec<csv::StringRecord> = rdr.records().filter_map(|r| r.ok()).collect();
    Ok((headers, records))
}

fn col(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers.iter().position(|h| h.trim().eq_ignore_ascii_case(name))
}

/// "2026.06.25 00:00:00" → "2026-06-25".
fn date_part(s: &str) -> Option<String> {
    let tok = s.split_whitespace().next()?;
    let p: Vec<&str> = tok.split('.').collect();
    if p.len() == 3 && p[0].len() == 4 {
        Some(format!("{}-{}-{}", p[0], p[1], p[2]))
    } else {
        None
    }
}

/// "2026.06.25 08:41:09" → "08:41" (the sample's hour:minute), for per-minute bucketing.
fn minute_part(s: &str) -> Option<String> {
    let time = s.split_whitespace().nth(1)?;
    let hhmm = time.get(0..5)?;
    if hhmm.len() == 5 && hhmm.as_bytes()[2] == b':' {
        Some(hhmm.to_string())
    } else {
        None
    }
}

/// "2026.06.25 08:41:09" → seconds since the Unix epoch, for measuring the gap
/// between consecutive sleep rows. Returns None if the timestamp doesn't parse.
fn parse_ts_secs(s: &str) -> Option<i64> {
    use chrono::NaiveDateTime;
    NaiveDateTime::parse_from_str(s.trim(), "%Y.%m.%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp())
}

fn parse_i64(s: &str) -> Option<i64> {
    let t = s.trim().trim_matches('"');
    t.parse::<i64>().ok().or_else(|| t.parse::<f64>().ok().map(|f| f.round() as i64))
}

fn parse_f64(s: &str) -> Option<f64> {
    s.trim().trim_matches('"').parse::<f64>().ok()
}

fn hours(seconds: f64) -> f64 {
    round1(seconds / 3600.0)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn file_label(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(items: &[&str]) -> csv::StringRecord {
        csv::StringRecord::from(items.to_vec())
    }

    #[test]
    fn date_part_converts_dotted_to_iso() {
        assert_eq!(date_part("2026.06.25 00:00:00").as_deref(), Some("2026-06-25"));
        assert_eq!(date_part("2026.06.25").as_deref(), Some("2026-06-25"));
        assert_eq!(date_part("garbage"), None);
        assert_eq!(date_part(""), None);
    }

    #[test]
    fn parse_numbers_tolerates_quotes_and_decimals() {
        assert_eq!(parse_i64("6705"), Some(6705));
        assert_eq!(parse_i64("\"6705.0\""), Some(6705));
        assert_eq!(parse_i64("bad"), None);
        assert_eq!(parse_f64("\"0.0\""), Some(0.0));
        assert_eq!(parse_f64("12.5"), Some(12.5));
    }

    #[test]
    fn hours_converts_and_rounds() {
        assert_eq!(hours(3600.0), 1.0);
        assert_eq!(hours(900.0), 0.3); // 0.25h rounds to 0.3 at 1dp (half away from zero)
        assert_eq!(hours(1800.0), 0.5);
        assert_eq!(hours(0.0), 0.0);
    }

    #[test]
    fn steps_use_daily_total_not_sum() {
        let h = rec(&["Date", "Time", "Steps", "Source"]);
        let recs = vec![
            rec(&["2026.06.26 00:00:00", "00:00:00", "6705", "com.sec.android.app.shealth"]),
            rec(&["2026.06.26 06:41:09", "06:41:09", "6", "android"]),
            rec(&["2026.06.26 06:41:21", "06:41:21", "8", "android"]),
        ];
        let m = agg_steps(&h, &recs).unwrap();
        // The 00:00:00 daily total wins; the granular rows are not added on top.
        assert_eq!(m.get("2026-06-26"), Some(&6705));
    }

    #[test]
    fn steps_fall_back_to_granular_without_midnight_total() {
        let h = rec(&["Date", "Time", "Steps"]);
        let recs = vec![
            rec(&["2026.06.26 06:41:09", "06:41:09", "6"]),
            rec(&["2026.06.26 06:41:21", "06:41:21", "8"]),
        ];
        let m = agg_steps(&h, &recs).unwrap();
        assert_eq!(m.get("2026-06-26"), Some(&14));
    }

    #[test]
    fn hr_buckets_per_minute_and_tracks_min_max() {
        let h = rec(&["Date", "Time", "Heart rate", "Source"]);
        let recs = vec![
            rec(&["2026.06.26 00:00:00", "00:00:00", "70", "x"]),
            rec(&["2026.06.26 00:01:00", "00:01:00", "66", "x"]),
            rec(&["2026.06.26 00:02:00", "00:02:00", "80", "x"]),
        ];
        let m = agg_hr(&h, &recs).unwrap();
        let dh = &m["2026-06-26"];
        assert_eq!(dh.minute.len(), 3); // three distinct minutes
        assert_eq!(dh.min, 66);
        assert_eq!(dh.max, 80);
        assert_eq!(day_ave_hr(dh), Some(72)); // mean of 70, 66, 80
    }

    #[test]
    fn hr_daily_average_is_time_weighted_not_sample_weighted() {
        // One resting minute at 60 bpm, then a workout minute sampled 100x at 150 bpm.
        // Per-sample mean would be ~149; time-weighted (per-minute) mean is (60+150)/2.
        let h = rec(&["Date", "Time", "Heart rate"]);
        let mut recs = vec![rec(&["2026.06.26 08:00:00", "08:00:00", "60"])];
        for i in 0..100 {
            recs.push(rec(&[&format!("2026.06.26 09:00:{:02}", i % 60), "09:00", "150"]));
        }
        let m = agg_hr(&h, &recs).unwrap();
        let dh = &m["2026-06-26"];
        assert_eq!(dh.minute.len(), 2);
        assert_eq!(day_ave_hr(dh), Some(105));
        assert_eq!(dh.min, 60);
        assert_eq!(dh.max, 150);
    }

    #[test]
    fn energy_sums_active_calories_only() {
        let h = rec(&["Date", "Time", "Active calories", "Resting calories", "Total calories"]);
        let recs = vec![
            rec(&["2026.06.25 00:00:00", "00:00:00", "0.0", "0.0", "0.0"]),
            rec(&["2026.06.25 10:00:00", "10:00:00", "12.5", "60", "72.5"]),
        ];
        let m = agg_energy(&h, &recs).unwrap();
        assert!((m["2026-06-25"] - 12.5).abs() < 1e-9);
    }

    #[test]
    fn sleep_attributed_to_wake_day_and_stages_summed() {
        let h = rec(&["Date", "Time", "Duration in seconds", "Sleep stage"]);
        let recs = vec![
            rec(&["2026.06.25 22:44:00", "22:44:00", "900", "light"]),
            rec(&["2026.06.25 23:25:00", "23:25:00", "2460", "deep"]),
            rec(&["2026.06.26 00:06:00", "00:06:00", "60", "awake"]),
            rec(&["2026.06.26 01:59:30", "01:59:30", "930", "rem"]),
        ];
        let m = agg_sleep(&h, &recs).unwrap();
        // The whole session lands on the wake day, not the bed day.
        assert!(!m.contains_key("2026-06-25"));
        let (asleep, rem, deep, awake) = m["2026-06-26"];
        assert_eq!(rem, 930.0);
        assert_eq!(deep, 2460.0);
        assert_eq!(asleep, 900.0 + 2460.0 + 930.0); // light + deep + rem
        assert_eq!(awake, 60.0);
    }

    #[test]
    fn sleep_range_file_splits_into_per_night_sessions() {
        // A range export holding two separate nights must land on two wake days, not
        // collapse the whole file onto the latest date.
        let h = rec(&["Date", "Time", "Duration in seconds", "Sleep stage"]);
        let recs = vec![
            // Night one (contiguous rows) → wakes 06-25.
            rec(&["2026.06.24 23:00:00", "23:00:00", "3600", "light"]),
            rec(&["2026.06.25 00:00:00", "00:00:00", "1800", "deep"]),
            rec(&["2026.06.25 00:30:00", "00:30:00", "600", "awake"]),
            // ~22h daytime gap (no rows), then night two (contiguous) → wakes 06-26.
            rec(&["2026.06.25 22:30:00", "22:30:00", "3600", "light"]),
            rec(&["2026.06.25 23:30:00", "23:30:00", "1800", "deep"]),
            rec(&["2026.06.26 00:00:00", "00:00:00", "1200", "rem"]),
        ];
        let m = agg_sleep(&h, &recs).unwrap();
        assert_eq!(m.len(), 2);
        // Night one: the 22h gap keeps it off the later wake day.
        let n1 = m["2026-06-25"];
        assert_eq!(n1, (3600.0 + 1800.0, 0.0, 1800.0, 600.0));
        let n2 = m["2026-06-26"];
        assert_eq!(n2, (3600.0 + 1800.0 + 1200.0, 1200.0, 1800.0, 0.0));
    }

    #[test]
    fn missing_columns_returns_error() {
        let h = rec(&["Date", "Time"]);
        assert!(agg_steps(&h, &[]).is_err());
        assert!(agg_hr(&h, &[]).is_err());
        assert!(agg_energy(&h, &[]).is_err());
        assert!(agg_sleep(&h, &[]).is_err());
    }

    #[test]
    fn matches_only_samsung_health_files() {
        assert!(is_samsung_health_file(Path::new("Steps 2026.07.22 Samsung Health.csv")));
        assert!(is_samsung_health_file(Path::new("Steps 29-2026 samsung health.csv")));
        assert!(is_samsung_health_file(Path::new("Steps June 2026 Samsung-Health.csv")));
        assert!(is_samsung_health_file(Path::new("Heart rate 2026.07.21 SamsungHealth.csv")));

        assert!(!is_samsung_health_file(Path::new("Steps 2026.07.22 Health Connect.csv")));
        assert!(!is_samsung_health_file(Path::new("Heart rate 2026.06.01-2026.07.01 Health Connect.csv")));
        assert!(!is_samsung_health_file(Path::new("random_file.csv")));
    }
}
