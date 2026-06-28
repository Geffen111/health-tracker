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
// NOTE: extraction sends raw note content to OpenRouter. This is an explicit,
// user-chosen exception to the app's "only aggregates leave the device" rule.

use crate::commands::ai::{call_openrouter, strip_code_fences};
use crate::commands::{settings, vault};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
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

#[derive(Serialize, sqlx::FromRow)]
pub struct LabPoint {
    pub result_date: String,
    pub value_num: Option<f64>,
    pub value_text: Option<String>,
    pub unit: Option<String>,
    pub ref_low: Option<f64>,
    pub ref_high: Option<f64>,
    pub ref_text: Option<String>,
    pub flag: Option<String>,
    pub source_note: String,
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
        match extract_one(&api_key, &note.title, &note.body).await {
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
- test_name: short canonical analyte name; strip a leading "S "/"Serum". E.g. "C-Reactive Protein (CRP)"->"CRP", "S Ferritin"->"Ferritin", "S HDL-Cholesterol"->"HDL Cholesterol", "Haemoglobin"->"Haemoglobin".
- category: the panel this note represents, e.g. "FBE","Lipids","Iron Studies","Serology","CRP","Thyroid","Renal".
- date: ISO "YYYY-MM-DD". Dates in the note are Australian. "3 Mar 2026"->"2026-03-03"; "03/03/26" (DD/MM/YY)->"2026-03-03"; "Mar 2026"->"2026-03-01" (first-of-month when no day). If the note has a single header date, use it for every row.
- value_num: the numeric value as a JSON number, or null if non-numeric. For "<3" use value_num null, value_text "<3". For "84" use value_num 84.
- value_text: the value exactly as written ("84","<3","Not detected","161").
- unit: e.g. "mg/L","ug/L","x10^9/L"; null if none.
- ref_low/ref_high: numeric bounds when known. "30-500"->30/500. "<4"->ref_high 4, ref_low null. ">50"->ref_low 50, ref_high null. null when qualitative/unknown.
- ref_text: the reference exactly as written.
- flag: "HIGH" or "LOW" if the value is flagged abnormal (look for ⚠, *, H, L, "HIGH"); else null.
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

/// One row per distinct test, carrying its latest value (for the analyte picker).
#[tauri::command]
pub async fn get_lab_tests(pool: State<'_, SqlitePool>) -> Result<Vec<LabTestSummary>, String> {
    sqlx::query_as::<_, LabTestSummary>(
        r#"SELECT l.test_name AS test_name, l.category AS category,
             (SELECT COUNT(*) FROM lab_results x WHERE x.test_name = l.test_name) AS n,
             l.result_date AS latest_date, l.value_text AS latest_value_text,
             l.value_num AS latest_value_num, l.unit AS unit, l.flag AS flag
           FROM lab_results l
           JOIN (SELECT test_name, MAX(result_date) AS md FROM lab_results GROUP BY test_name) m
             ON m.test_name = l.test_name AND m.md = l.result_date
           GROUP BY l.test_name
           ORDER BY (l.category IS NULL), l.category, l.test_name"#,
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

/// Every result for one analyte, oldest first (for the chart + table).
#[tauri::command]
pub async fn get_lab_series(
    pool: State<'_, SqlitePool>,
    test_name: String,
) -> Result<Vec<LabPoint>, String> {
    sqlx::query_as::<_, LabPoint>(
        r#"SELECT result_date, value_num, value_text, unit, ref_low, ref_high, ref_text, flag, source_note
           FROM lab_results WHERE test_name = ? ORDER BY result_date ASC"#,
    )
    .bind(test_name)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
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

/// Accept ISO YYYY-MM-DD, or pad YYYY-MM to first-of-month. Otherwise None.
fn normalize_date(d: &str) -> Option<String> {
    let d = d.trim();
    let b = d.as_bytes();
    if b.len() == 10 && is_iso_ymd(d) {
        return Some(d.to_string());
    }
    if b.len() == 7
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
    {
        return Some(format!("{}-01", d));
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
        assert_eq!(normalize_date("Mar 2026"), None);
    }
}
