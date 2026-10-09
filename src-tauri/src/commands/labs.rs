// Pathology/lab results: AI-assisted extraction from the Health Records vault
// into the `lab_results` table, plus queries that drive the Labs chart view.
//
// The vault's pathology notes come in many table shapes (rows-as-dates,
// rows-as-analytes with date columns, qualitative serology, mixed date formats,
// `<3`/flag values), so a regex parser would be brittle. Instead each note is
// sent to the model with a strict JSON-extraction prompt; the structured rows
// are stored with their raw value/reference text and source note for audit. The
// table is a re-buildable cache — re-running an extraction rebuilds each note's
// rows from scratch, so edits in Obsidian flow through on the next run.
//
// Reading back, rows are cleaned up (`load_consolidated`) without touching the table:
// one analyte reported under several names ("Complement C3", "Serum Complement (C3)",
// "C3") becomes one test; the same result repeated by a later report's history table
// becomes one point, keeping the most precise date ("Mar 2026" vs "3 Mar 2026"); and
// each test gets one category. So fixes apply to existing data without re-extracting.
//
// NOTE: extraction sends raw note content to OpenRouter. This is an explicit,
// user-chosen exception to the app's "only aggregates leave the device" rule.

use crate::commands::ai::{call_openrouter, strip_code_fences};
use crate::commands::{settings, vault};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use tauri::State;

#[derive(Serialize)]
pub struct LabExtractResult {
    pub notes_processed: i64,
    pub rows_extracted: i64,
    pub notes_failed: i64,
    pub errors: Vec<String>,
    pub extracted_at: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct LabTestSummary {
    pub test_name: String,
    pub category: Option<String>,
    pub n: i64,
    pub latest_date: String,
    pub latest_value_text: Option<String>,
    pub latest_value_num: Option<f64>,
    pub unit: Option<String>,
    pub flag: Option<String>,
}

/// One result as shown: possibly several stored rows that report the same thing.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LabPoint {
    pub test_name: String,
    pub category: Option<String>,
    pub result_date: String,
    pub value_num: Option<f64>,
    pub value_text: Option<String>,
    pub unit: Option<String>,
    pub ref_low: Option<f64>,
    pub ref_high: Option<f64>,
    pub ref_text: Option<String>,
    pub flag: Option<String>,
    pub source_note: String,
    /// Every note this result appears in (source_note first).
    #[sqlx(skip)]
    pub sources: Vec<String>,
}

/// Row shape the model is asked to emit. Numeric fields are accepted as either
/// JSON numbers or strings (models are inconsistent) and coerced.
#[derive(Deserialize)]
struct ExtractedRow {
    test_name: String,
    #[serde(default)]
    category: Option<String>,
    date: String,
    #[serde(default)]
    value_num: Option<serde_json::Value>,
    #[serde(default)]
    value_text: Option<String>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    ref_low: Option<serde_json::Value>,
    #[serde(default)]
    ref_high: Option<serde_json::Value>,
    #[serde(default)]
    ref_text: Option<String>,
    #[serde(default)]
    flag: Option<String>,
}

/// Re-extract every pathology note in the vault into `lab_results`.
#[tauri::command]
pub async fn extract_lab_results(pool: State<'_, SqlitePool>) -> Result<LabExtractResult, String> {
    let api_key = settings::get_api_key()
        .await?
        .ok_or_else(|| "OpenRouter API key not configured. Add your key in Settings.".to_string())?;

    // Pathology notes only (by frontmatter type or folder); skip the index note.
    let notes: Vec<vault::RawNote> = vault::walk_notes()
        .into_iter()
        .filter(|n| {
            n.note_type.as_deref() == Some("pathology_result")
                || (n.folder == "Pathology Results"
                    && n.note_type.as_deref() != Some("pathology_results_index"))
        })
        .filter(|n| !n.rel_path.to_lowercase().contains("index"))
        .collect();

    let extracted_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut rows_extracted = 0i64;
    let mut notes_failed = 0i64;
    let mut errors: Vec<String> = Vec::new();

    for note in &notes {
        // One retry: a failure is usually a malformed or cut-off reply, not the note.
        let first = extract_one(&api_key, &note.title, &note.body).await;
        let result = match first {
            Ok(rows) => Ok(rows),
            Err(_) => extract_one(&api_key, &note.title, &note.body).await,
        };
        match result {
            Ok(rows) => {
                let n = store_note_rows(&pool, &note.rel_path, &extracted_at, &rows).await?;
                rows_extracted += n;
            }
            Err(e) => {
                notes_failed += 1;
                errors.push(format!("{}: {}", note.title, e));
            }
        }
    }

    settings::put_setting("labs_last_extract", serde_json::json!(extracted_at))?;

    Ok(LabExtractResult {
        notes_processed: notes.len() as i64,
        rows_extracted,
        notes_failed,
        errors,
        extracted_at,
    })
}

async fn extract_one(api_key: &str, title: &str, body: &str) -> Result<Vec<ExtractedRow>, String> {
    let prompt = format!(
        r#"Extract structured lab/pathology measurements from ONE patient note.
Return ONLY a JSON array (no markdown, no commentary). Each element is one analyte measured at one date:
{{"test_name","category","date","value_num","value_text","unit","ref_low","ref_high","ref_text","flag"}}

Rules:
- One object per analyte PER date. If a table has several date columns (e.g. "Jul 2017","Mar 2026"), emit one object for each analyte in each date column.
- test_name: short canonical analyte name; strip a leading "S "/"Serum". E.g. "C-Reactive Protein (CRP)"->"CRP", "S Ferritin"->"Ferritin", "S HDL-Cholesterol"->"HDL Cholesterol", "Haemoglobin"->"Haemoglobin". Always use these names: "C3" and "C4" (complement), "Haematocrit" (not PCV/HCT), "Red Cell Count" (not RCC), "White Cell Count" (not WCC), "Vitamin B12", "Anti-TPO", "Vitamin D (25-OH)". For a urine test prefix "Urine " (e.g. "Urine Albumin", "Urine Creatinine").
- category: the panel this note represents, e.g. "FBE","Lipids","Iron Studies","Serology","CRP","Thyroid","Renal".
- date: ISO "YYYY-MM-DD". Dates in the note are Australian. "3 Mar 2026"->"2026-03-03"; "03/03/26" (DD/MM/YY)->"2026-03-03"; "Mar 2026"->"2026-03-01" (first-of-month when no day). If the note has a single header date, use it for every row.
- value_num: the numeric value as a JSON number, or null if non-numeric. For "<3" use value_num null, value_text "<3". For "84" use value_num 84.
- value_text: the value exactly as written ("84","<3","Not detected","161").
- unit: e.g. "mg/L","ug/L","x10^9/L"; null if none.
- ref_low/ref_high: numeric bounds when known. "30-500"->30/500. "<4"->ref_high 4, ref_low null. ">50"->ref_low 50, ref_high null. null when qualitative/unknown.
- ref_text: the reference exactly as written.
- flag: "HIGH" or "LOW" if the value is flagged abnormal (look for ⚠, *, H, L, "HIGH"); else null.
- If a table repeats earlier results (history columns), emit those too.
- Ignore narrative interpretation, methodology and "Source:" footers. Only emit measured analyte values. If there are none, return [].

Note title: {title}

Note markdown:
{body}"#,
        title = title,
        body = body,
    );

    let content = call_openrouter(api_key, &prompt, 0.0, 8192).await?;
    parse_rows(&content)
}

/// Parse the model's reply into rows, tolerating stray text around the array.
fn parse_rows(content: &str) -> Result<Vec<ExtractedRow>, String> {
    let cleaned = strip_code_fences(content).trim();
    if let Ok(rows) = serde_json::from_str::<Vec<ExtractedRow>>(cleaned) {
        return Ok(rows);
    }
    // Fallback: slice from the first '[' to the last ']'.
    if let (Some(start), Some(end)) = (cleaned.find('['), cleaned.rfind(']')) {
        if end > start {
            return serde_json::from_str::<Vec<ExtractedRow>>(&cleaned[start..=end])
                .map_err(|e| format!("could not parse extracted JSON: {}", e));
        }
    }
    Err("model did not return a JSON array".to_string())
}

/// Rebuild one note's rows: clear its previous rows, then insert the new set.
async fn store_note_rows(
    pool: &SqlitePool,
    source_note: &str,
    extracted_at: &str,
    rows: &[ExtractedRow],
) -> Result<i64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM lab_results WHERE source_note = ?")
        .bind(source_note)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    let mut inserted = 0i64;
    for r in rows {
        let test_name = r.test_name.trim();
        let date = match normalize_date(&r.date) {
            Some(d) => d,
            None => continue, // skip rows without a usable date
        };
        if test_name.is_empty() {
            continue;
        }
        sqlx::query(
            r#"INSERT INTO lab_results
                (test_name, category, result_date, value_num, value_text, unit,
                 ref_low, ref_high, ref_text, flag, source_note, extracted_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(test_name, result_date, source_note) DO UPDATE SET
                 category=excluded.category, value_num=excluded.value_num,
                 value_text=excluded.value_text, unit=excluded.unit,
                 ref_low=excluded.ref_low, ref_high=excluded.ref_high,
                 ref_text=excluded.ref_text, flag=excluded.flag,
                 extracted_at=excluded.extracted_at"#,
        )
        .bind(test_name)
        .bind(opt_str(&r.category))
        .bind(&date)
        .bind(as_f64(&r.value_num))
        .bind(opt_str(&r.value_text))
        .bind(opt_str(&r.unit))
        .bind(as_f64(&r.ref_low))
        .bind(as_f64(&r.ref_high))
        .bind(opt_str(&r.ref_text))
        .bind(opt_str(&r.flag))
        .bind(source_note)
        .bind(extracted_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        inserted += 1;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(inserted)
}

// ── Reading back: one name, one category, one point per result ──

/// Analytes that turn up under several names. Keyed by `name_key` of the raw name.
const NAME_ALIASES: &[(&str, &str)] = &[
    ("complement c3", "C3"), ("c3 complement", "C3"), ("c3", "C3"),
    ("complement c4", "C4"), ("c4 complement", "C4"), ("c4", "C4"),
    ("pcv", "Haematocrit"), ("hct", "Haematocrit"), ("haematocrit pcv", "Haematocrit"),
    ("hematocrit", "Haematocrit"), ("packed cell volume", "Haematocrit"),
    ("rcc", "Red Cell Count"), ("red cell count", "Red Cell Count"), ("red blood cell count", "Red Cell Count"),
    ("wcc", "White Cell Count"), ("white cell count", "White Cell Count"), ("white blood cell count", "White Cell Count"),
    ("hb", "Haemoglobin"), ("hemoglobin", "Haemoglobin"),
    ("plt", "Platelets"), ("platelet count", "Platelets"),
    ("total vitamin b12", "Vitamin B12"), ("b12", "Vitamin B12"), ("vitamin b12", "Vitamin B12"),
    ("tpoab", "Anti-TPO"), ("anti tpo", "Anti-TPO"), ("thyroid peroxidase antibodies", "Anti-TPO"),
    ("anti thyroid peroxidase", "Anti-TPO"), ("tpo antibodies", "Anti-TPO"),
    ("25 hydroxy vitamin d", "Vitamin D (25-OH)"), ("vitamin d", "Vitamin D (25-OH)"),
    ("25 oh vitamin d", "Vitamin D (25-OH)"), ("vitamin d 25 oh", "Vitamin D (25-OH)"),
    ("c reactive protein", "CRP"), ("crp", "CRP"),
    ("thyroid stimulating hormone", "TSH"),
    ("fasting glucose", "Glucose"), ("fasting blood glucose", "Glucose"),
];

/// Panels named differently by different labs and notes.
const CATEGORY_ALIASES: &[(&str, &str)] = &[
    ("serum biochemistry", "Biochemistry"), ("biochemistry", "Biochemistry"), ("lfts", "Biochemistry"),
    ("renal", "Biochemistry"), ("euc", "Biochemistry"), ("eucs", "Biochemistry"), ("renal function", "Biochemistry"),
    ("crp", "Inflammation"), ("esr", "Inflammation"),
    ("thyroid antibodies", "Thyroid"),
    ("fasting blood glucose", "Glucose"),
    ("b12 folate", "B12 & Folate"), ("vitamin b12 homocysteine", "B12 & Folate"),
    ("rheumatoid factor", "Serology"), ("ena", "Serology"), ("hiv serology", "Serology"),
    ("ige", "Allergy"), ("allergen panel", "Allergy"), ("tryptase", "Allergy"),
    ("complement", "Complement"),
];

/// Lower-case, punctuation to spaces, a leading "serum"/"s " dropped.
fn name_key(raw: &str) -> String {
    let cleaned: String = raw
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut words: Vec<&str> = cleaned.split_whitespace().collect();
    while matches!(words.first(), Some(&"serum") | Some(&"s")) && words.len() > 1 {
        words.remove(0);
    }
    words.join(" ")
}

fn canonical_name(raw: &str, category: Option<&str>) -> String {
    let key = name_key(raw);
    let mut name = NAME_ALIASES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| raw.trim().to_string());
    // An albumin:creatinine ratio note measures urine, not the serum analytes of the same name.
    let urine = category.map(|c| name_key(c)).is_some_and(|c| c == "acr" || c.contains("urine"));
    if urine && matches!(key.as_str(), "albumin" | "creatinine") {
        name = format!("Urine {}", name);
    }
    name
}

fn canonical_category(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let key = name_key(raw);
    Some(CATEGORY_ALIASES.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string()).unwrap_or_else(|| raw.to_string()))
}

/// "< 3", "<3" and "3.0" all read as the same value.
fn value_key(p: &LabPoint) -> String {
    match (&p.value_text, p.value_num) {
        (Some(t), _) if !t.trim().is_empty() => {
            let t: String = t.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
            t.parse::<f64>().map(|n| format!("{}", n)).unwrap_or(t)
        }
        (_, Some(n)) => format!("{}", n),
        _ => String::new(),
    }
}

/// Days between two ISO dates (None if either won't parse).
fn days_apart(a: &str, b: &str) -> Option<i64> {
    let pa = chrono::NaiveDate::parse_from_str(a, "%Y-%m-%d").ok()?;
    let pb = chrono::NaiveDate::parse_from_str(b, "%Y-%m-%d").ok()?;
    Some((pa - pb).num_days().abs())
}

/// A first-of-month date is how a month-only date ("Mar 2026") is stored.
fn month_only(d: &str) -> bool {
    d.ends_with("-01")
}

/// Merge stored rows into results as shown. Expects rows already renamed, sorted by test
/// then date. Two rows are one result when they have the same value and either the same
/// date, or one has only a month and the other falls within 45 days of it.
fn consolidate(rows: Vec<LabPoint>) -> Vec<LabPoint> {
    let mut out: Vec<LabPoint> = Vec::new();
    for mut r in rows {
        r.sources = vec![r.source_note.clone()];
        let key = value_key(&r);
        let same = out.iter_mut().rev().take_while(|o| o.test_name == r.test_name).find(|o| {
            value_key(o) == key
                && (o.result_date == r.result_date
                    || ((month_only(&o.result_date) || month_only(&r.result_date))
                        && days_apart(&o.result_date, &r.result_date).is_some_and(|d| d <= 45)))
        });
        match same {
            Some(o) => {
                if !o.sources.contains(&r.source_note) {
                    o.sources.push(r.source_note.clone());
                }
                // The precise date wins; fill in anything the first copy lacked.
                if month_only(&o.result_date) && !month_only(&r.result_date) {
                    o.result_date = r.result_date.clone();
                }
                o.unit = o.unit.take().or(r.unit);
                o.ref_low = o.ref_low.or(r.ref_low);
                o.ref_high = o.ref_high.or(r.ref_high);
                o.ref_text = o.ref_text.take().or(r.ref_text);
                o.flag = o.flag.take().or(r.flag);
            }
            None => out.push(r),
        }
    }
    // Each test takes the category most of its results were filed under.
    let mut votes: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for p in &out {
        if let Some(c) = &p.category {
            *votes.entry(p.test_name.clone()).or_default().entry(c.clone()).or_default() += 1;
        }
    }
    let pick: HashMap<String, String> = votes
        .into_iter()
        .map(|(t, v)| {
            let best = v.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(c, _)| c).unwrap();
            (t, best)
        })
        .collect();
    for p in &mut out {
        p.category = pick.get(&p.test_name).cloned();
    }
    out.sort_by(|a, b| a.test_name.cmp(&b.test_name).then(a.result_date.cmp(&b.result_date)));
    out
}

/// Every result, renamed and de-duplicated, sorted by test then date.
pub async fn load_consolidated(pool: &SqlitePool) -> Result<Vec<LabPoint>, String> {
    let mut rows: Vec<LabPoint> = sqlx::query_as(
        r#"SELECT test_name, category, result_date, value_num, value_text, unit, ref_low, ref_high,
                  ref_text, flag, source_note
           FROM lab_results ORDER BY source_note"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    for r in &mut rows {
        r.test_name = canonical_name(&r.test_name, r.category.as_deref());
        r.category = canonical_category(r.category.as_deref());
    }
    rows.sort_by(|a, b| a.test_name.cmp(&b.test_name).then(a.result_date.cmp(&b.result_date)));
    Ok(consolidate(rows))
}

/// One row per distinct test, carrying its latest value (for the analyte picker).
#[tauri::command]
pub async fn get_lab_tests(pool: State<'_, SqlitePool>) -> Result<Vec<LabTestSummary>, String> {
    let all = load_consolidated(&pool).await?;
    let mut out: Vec<LabTestSummary> = Vec::new();
    for p in &all {
        match out.last_mut() {
            Some(t) if t.test_name == p.test_name => {
                t.n += 1;
                // Sorted by date, so the last one seen is the latest.
                t.latest_date = p.result_date.clone();
                t.latest_value_text = p.value_text.clone();
                t.latest_value_num = p.value_num;
                t.unit = p.unit.clone().or(t.unit.take());
                t.flag = p.flag.clone();
            }
            _ => out.push(LabTestSummary {
                test_name: p.test_name.clone(),
                category: p.category.clone(),
                n: 1,
                latest_date: p.result_date.clone(),
                latest_value_text: p.value_text.clone(),
                latest_value_num: p.value_num,
                unit: p.unit.clone(),
                flag: p.flag.clone(),
            }),
        }
    }
    out.sort_by(|a, b| {
        (a.category.is_none(), &a.category, &a.test_name).cmp(&(b.category.is_none(), &b.category, &b.test_name))
    });
    Ok(out)
}

/// Every result for one analyte, oldest first (for the chart + table).
#[tauri::command]
pub async fn get_lab_series(
    pool: State<'_, SqlitePool>,
    test_name: String,
) -> Result<Vec<LabPoint>, String> {
    Ok(load_consolidated(&pool).await?.into_iter().filter(|p| p.test_name == test_name).collect())
}

#[tauri::command]
pub async fn get_labs_last_extract() -> Result<Option<String>, String> {
    Ok(settings::setting_str("labs_last_extract"))
}

// ── helpers ──

fn opt_str(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn as_f64(v: &Option<serde_json::Value>) -> Option<f64> {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// The model is asked for ISO dates but doesn't always comply, so accept the forms the
/// notes use: ISO, YYYY-MM (first of month), Australian D/M/Y (2- or 4-digit year),
/// "3 Mar 2026", "Mar 2026" / "March 2026" (first of month). Otherwise None.
fn normalize_date(d: &str) -> Option<String> {
    use chrono::NaiveDate;
    let d = d.trim().trim_end_matches('.');
    let b = d.as_bytes();
    if b.len() == 10 && is_iso_ymd(d) {
        return NaiveDate::parse_from_str(d, "%Y-%m-%d").ok().map(|_| d.to_string());
    }
    if b.len() == 7
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
    {
        return Some(format!("{}-01", d));
    }
    use chrono::Datelike;
    let iso = |x: NaiveDate| x.format("%Y-%m-%d").to_string();
    // %Y happily reads "26" as the year 26, so a pre-1900 result means "try the next format".
    let plausible = |x: &NaiveDate| x.year() >= 1900;
    for f in ["%d/%m/%Y", "%d/%m/%y", "%d-%m-%Y", "%d.%m.%Y", "%Y-%m-%d", "%d %b %Y", "%d %B %Y", "%e %b %Y"] {
        if let Some(x) = NaiveDate::parse_from_str(d, f).ok().filter(plausible) {
            return Some(iso(x));
        }
    }
    for f in ["%d %b %Y", "%d %B %Y"] {
        if let Some(x) = NaiveDate::parse_from_str(&format!("1 {}", d), f).ok().filter(plausible) {
            return Some(iso(x));
        }
    }
    None
}

fn is_iso_ymd(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_with_noise() {
        let c = "Here you go:\n```json\n[{\"test_name\":\"CRP\",\"date\":\"2026-03-03\",\"value_num\":3}]\n```";
        let rows = parse_rows(c).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].test_name, "CRP");
    }

    #[test]
    fn coerces_numbers() {
        assert_eq!(as_f64(&Some(serde_json::json!(84))), Some(84.0));
        assert_eq!(as_f64(&Some(serde_json::json!("5.5"))), Some(5.5));
        assert_eq!(as_f64(&Some(serde_json::json!("<3"))), None);
        assert_eq!(as_f64(&None), None);
    }

    #[test]
    fn normalizes_dates() {
        assert_eq!(normalize_date("2026-03-03").as_deref(), Some("2026-03-03"));
        assert_eq!(normalize_date("2017-07").as_deref(), Some("2017-07-01"));
        assert_eq!(normalize_date("Mar 2026").as_deref(), Some("2026-03-01"));
        assert_eq!(normalize_date("March 2026").as_deref(), Some("2026-03-01"));
        assert_eq!(normalize_date("03/03/2026").as_deref(), Some("2026-03-03"));
        assert_eq!(normalize_date("3/3/26").as_deref(), Some("2026-03-03"));
        assert_eq!(normalize_date("3 Mar 2026").as_deref(), Some("2026-03-03"));
        assert_eq!(normalize_date("2026-02-30"), None);
        assert_eq!(normalize_date("soon"), None);
    }

    fn pt(name: &str, cat: &str, date: &str, text: &str, note: &str) -> LabPoint {
        LabPoint {
            test_name: canonical_name(name, Some(cat)),
            category: canonical_category(Some(cat)),
            result_date: date.into(),
            value_num: text.trim().parse().ok(),
            value_text: Some(text.into()),
            unit: None,
            ref_low: None,
            ref_high: None,
            ref_text: None,
            flag: None,
            source_note: note.into(),
            sources: vec![],
        }
    }

    #[test]
    fn one_name_per_analyte() {
        assert_eq!(canonical_name("Complement C3", None), "C3");
        assert_eq!(canonical_name("Serum Complement (C3)", None), "C3");
        assert_eq!(canonical_name("S Complement C4", None), "C4");
        assert_eq!(canonical_name("Haematocrit / PCV", None), "Haematocrit");
        assert_eq!(canonical_name("WCC", None), "White Cell Count");
        assert_eq!(canonical_name("Albumin", Some("ACR")), "Urine Albumin");
        assert_eq!(canonical_name("Albumin", Some("LFTs")), "Albumin");
        assert_eq!(canonical_name("Ferritin", None), "Ferritin");
        assert_eq!(canonical_category(Some("Serum Biochemistry")).as_deref(), Some("Biochemistry"));
    }

    #[test]
    fn repeated_results_merge_keeping_the_precise_date() {
        let mut rows = vec![
            pt("ALP", "LFTs", "2023-12-01", "94", "a.md"),          // "Dec 2023" history column
            pt("ALP", "Serum Biochemistry", "2023-12-16", "94", "b.md"),
            pt("ALP", "Biochemistry", "2023-12-16", "94", "c.md"),  // same report, another note
            pt("ALP", "Biochemistry", "2024-07-28", "90", "c.md"),
            pt("CRP", "CRP", "2026-03-03", "<3", "d.md"),
            pt("CRP", "CRP", "2026-03-03", "< 3", "e.md"),
            pt("CRP", "CRP", "2026-03-20", "<3", "f.md"),           // a later, separate test
        ];
        rows.sort_by(|a, b| a.test_name.cmp(&b.test_name).then(a.result_date.cmp(&b.result_date)));
        let out = consolidate(rows);
        let got: Vec<(String, String, usize)> = out.iter().map(|p| (p.test_name.clone(), p.result_date.clone(), p.sources.len())).collect();
        assert_eq!(got, vec![
            ("ALP".into(), "2023-12-16".into(), 3),
            ("ALP".into(), "2024-07-28".into(), 1),
            ("CRP".into(), "2026-03-03".into(), 2),
            ("CRP".into(), "2026-03-20".into(), 1),
        ]);
        assert!(out.iter().filter(|p| p.test_name == "ALP").all(|p| p.category.as_deref() == Some("Biochemistry")));
    }
}
