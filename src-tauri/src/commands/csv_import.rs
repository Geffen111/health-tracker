// Samsung Health → Health Sync → Google Drive CSV auto-import.
//
// Health Sync writes one CSV per day per metric into four folders under a Drive
// root. Each metric is intraday samples; we aggregate per day and COALESCE-upsert
// into daily_logs so manually-entered fields are never clobbered. A sleep session
// crosses midnight, so it's attributed to the WAKE day (the date of its last row).
//
// "On each sync a file is (re)created for that day", so files are reprocessed when
// their modification time is newer than the last successful sync (or when `full`).

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

    // Intermediate per-date accumulators.
    let mut steps: HashMap<String, i64> = HashMap::new();
    let mut energy: HashMap<String, f64> = HashMap::new();
    // (sum, count, min, max)
    let mut hr: HashMap<String, (i64, i64, i64, i64)> = HashMap::new();
    // (asleep, rem, deep, awake) seconds
    let mut sleep: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();

    // ── Steps ──
    for path in &collect_files(&root.join(STEPS_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((headers, records)) => {
                files_processed += 1;
                let (di, si) = (col(&headers, "Date"), col(&headers, "Steps"));
                if let (Some(di), Some(si)) = (di, si) {
                    for r in &records {
                        if let (Some(date), Some(n)) = (r.get(di).and_then(date_part), r.get(si).and_then(parse_i64)) {
                            *steps.entry(date).or_insert(0) += n;
                        }
                    }
                } else {
                    errors.push(format!("{}: missing Date/Steps columns", file_label(path)));
                }
            }
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // ── Energy burned (active calories) ──
    for path in &collect_files(&root.join(ENERGY_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((headers, records)) => {
                files_processed += 1;
                let (di, ai) = (col(&headers, "Date"), col(&headers, "Active calories"));
                if let (Some(di), Some(ai)) = (di, ai) {
                    for r in &records {
                        if let (Some(date), Some(c)) = (r.get(di).and_then(date_part), r.get(ai).and_then(parse_f64)) {
                            *energy.entry(date).or_insert(0.0) += c;
                        }
                    }
                } else {
                    errors.push(format!("{}: missing Date/Active calories columns", file_label(path)));
                }
            }
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // ── Heart rate ──
    for path in &collect_files(&root.join(HR_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((headers, records)) => {
                files_processed += 1;
                let (di, hi) = (col(&headers, "Date"), col(&headers, "Heart rate"));
                if let (Some(di), Some(hi)) = (di, hi) {
                    for r in &records {
                        if let (Some(date), Some(bpm)) = (r.get(di).and_then(date_part), r.get(hi).and_then(parse_i64)) {
                            let e = hr.entry(date).or_insert((0, 0, i64::MAX, i64::MIN));
                            e.0 += bpm;
                            e.1 += 1;
                            e.2 = e.2.min(bpm);
                            e.3 = e.3.max(bpm);
                        }
                    }
                } else {
                    errors.push(format!("{}: missing Date/Heart rate columns", file_label(path)));
                }
            }
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

    // ── Sleep (attribute the whole session to its wake day) ──
    for path in &collect_files(&root.join(SLEEP_DIR), last_sync_unix, &mut files_skipped) {
        match read_csv(path) {
            Ok((headers, records)) => {
                files_processed += 1;
                let (di, dur, st) = (
                    col(&headers, "Date"),
                    col(&headers, "Duration in seconds"),
                    col(&headers, "Sleep stage"),
                );
                if let (Some(di), Some(dur), Some(st)) = (di, dur, st) {
                    // Wake day = date of the latest row (timestamps sort lexically).
                    let wake_date = records
                        .iter()
                        .filter_map(|r| r.get(di).map(|s| s.to_string()))
                        .max()
                        .and_then(|s| date_part(&s));
                    if let Some(date) = wake_date {
                        let e = sleep.entry(date).or_insert((0.0, 0.0, 0.0, 0.0));
                        for r in &records {
                            let secs = r.get(dur).and_then(parse_f64).unwrap_or(0.0);
                            match r.get(st).map(|s| s.trim().to_lowercase()).as_deref() {
                                Some("rem") => { e.0 += secs; e.1 += secs; }
                                Some("deep") => { e.0 += secs; e.2 += secs; }
                                Some("light") | Some("sleeping") => { e.0 += secs; }
                                Some("awake") | Some("awake_in_bed") => { e.3 += secs; }
                                _ => {}
                            }
                        }
                    }
                } else {
                    errors.push(format!("{}: missing Date/Duration/Sleep stage columns", file_label(path)));
                }
            }
            Err(e) => errors.push(format!("{}: {}", file_label(path), e)),
        }
    }

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
    for (date, (sum, count, min, max)) in hr {
        if count > 0 {
            let a = days.entry(date).or_default();
            a.ave_hr = Some((sum as f64 / count as f64).round() as i64);
            a.hr_min = Some(min);
            a.hr_max = Some(max);
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

/// CSV files in `dir` (optionally only those modified after `last_sync_unix`).
fn collect_files(dir: &Path, last_sync_unix: Option<i64>, skipped: &mut i64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out, // folder missing → just nothing to import from it
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

fn parse_i64(s: &str) -> Option<i64> {
    let t = s.trim().trim_matches('"');
    // Tolerate decimals like "6705.0".
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
