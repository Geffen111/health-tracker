// Natural-language questions about the user's health log. Hybrid approach:
//   1. LLM turns the question into a single read-only SQL query (schema only — no
//      health data leaves the machine at this step).
//   2. We validate it (SELECT-only, single statement) and run it locally for exact figures.
//   3. LLM turns the (small) result rows into a plain-English answer.
// The generated SQL and rows are returned too, so the user can check the working.

use crate::commands::ai::{call_openrouter, strip_code_fences};
use crate::models::AskResponse;
use sqlx::{Column, Row, SqlitePool};
use tauri::State;

const MAX_DISPLAY_ROWS: usize = 200;
const MAX_ANSWER_ROWS: usize = 50;

// Tokens that would mutate the database or schema. Matched whole-word against
// the tokenised query, so identifiers like `created_at` are not affected.
const FORBIDDEN_TOKENS: &[&str] = &[
    "insert", "update", "delete", "drop", "alter", "create", "attach", "detach",
    "pragma", "replace", "vacuum", "reindex", "truncate", "grant", "commit",
    "begin", "rollback", "savepoint",
];

fn schema_doc(today: &str) -> String {
    format!(
        r#"SQLite database for one person's ME/CFS (chronic fatigue) health log. Today is {today}.
Dates are TEXT 'YYYY-MM-DD'. Ratings are 0-10 (higher = worse for fatigue/headache).
Boolean-ish columns are stored as 0/1 INTEGER. Tables:

daily_logs(log_date, day_name, fatigue_rating REAL, fatigue_desc, headache_rating REAL,
  headache_duration_hours REAL, other_symptoms, my_sleep_rating REAL, phone_sleep_rating REAL,
  sleep_avg REAL, sleep_time_head_on_pillow REAL, sleep_actual_asleep REAL, sleep_rem REAL,
  sleep_deep REAL, sleep_awake REAL, steps INTEGER, activity_calories REAL, ave_resting_hr INTEGER,
  ave_hr INTEGER, hr_min INTEGER, hr_max INTEGER, rostered_hours REAL, sick_leave_hours REAL,
  office_hours REAL, wfh_hours REAL, alcohol_std_drinks REAL, multivitamin INTEGER, vitamin_c INTEGER,
  add_meds, compression_socks INTEGER, notes)
  - One row per day. sleep_* are hours. fatigue_rating/headache_rating may be NULL on un-logged days.

pem_predictions(log_date, physical_load REAL, cognitive_load REAL, sensory_social_load REAL,
  three_day_weighted_load REAL, recovery_debt REAL, threshold_penalty REAL, predicted_pem_risk REAL,
  risk_band TEXT 'Low'|'Medium'|'High', crash_flag INTEGER, predicted_next_day_fatigue REAL,
  predicted_low REAL, predicted_high REAL)
  - PEM = post-exertional malaise (a crash). crash_flag = 1 means a crash was flagged that day.

medications(id, name, short_code, default_dose REAL, dose_unit, category, active INTEGER, notes)
medication_doses(id, medication_id, log_date, time_taken, dose_amount REAL, notes)
  - join medication_doses.medication_id = medications.id for the med name.

blood_pressure(id, log_date, reading_num, time_taken, systolic INTEGER, diastolic INTEGER, notes)
  - multiple readings per day (reading_num). A "deleted" reading has notes='DELETED' and null systolic/diastolic.

activity_categories(id, name, energy_weight REAL)   -- e.g. 'Physical / Active', 'Screen / Sedentary'
activity_types(id, name, category_id, default_energy_cost TEXT 'Low'|'Medium'|'High')
activity_log(id, log_date, activity_type_id, duration_hours REAL, energy_cost, notes)
  - join activity_log.activity_type_id = activity_types.id = activity_categories.id for names/categories.

watch_calibration(id, cal_date, cal_time, notes)   -- blood-pressure-monitor calibration events.

Date helpers: date('now'), date('now','-30 days'), strftime('%Y-%m', log_date), etc.
Match free text with "LIKE ''%term%'' COLLATE NOCASE"."#,
        today = today
    )
}

#[tauri::command]
pub async fn ask_question(
    pool: State<'_, SqlitePool>,
    question: String,
) -> Result<AskResponse, String> {
    let api_key = crate::commands::settings::get_api_key()
        .await?
        .ok_or_else(|| "OpenRouter API key not configured. Please add your API key in Settings.".to_string())?;

    let q = question.trim();
    if q.is_empty() {
        return Err("Please enter a question.".to_string());
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let schema = schema_doc(&today);

    // Step 1: generate SQL (with one retry if the query fails to execute).
    let mut plan = generate_sql(&api_key, &schema, q, None).await?;
    let (columns, rows, truncated) = match run_query(&pool, &plan.sql).await {
        Ok(r) => r,
        Err(first_err) => {
            plan = generate_sql(&api_key, &schema, q, Some(&first_err)).await?;
            run_query(&pool, &plan.sql)
                .await
                .map_err(|e| format!("Could not run the generated query: {}", e))?
        }
    };

    // Step 3: turn the rows into a plain-English answer.
    let answer = answer_from_rows(&api_key, q, &plan.explanation, &columns, &rows).await?;

    Ok(AskResponse {
        answer,
        sql: plan.sql,
        explanation: plan.explanation,
        columns,
        rows,
        truncated,
    })
}

struct QueryPlan {
    sql: String,
    explanation: String,
}

async fn generate_sql(
    api_key: &str,
    schema: &str,
    question: &str,
    retry_error: Option<&str>,
) -> Result<QueryPlan, String> {
    let retry_note = match retry_error {
        Some(e) => format!("\n\nYour previous query failed with this error, fix it:\n{}", e),
        None => String::new(),
    };

    let prompt = format!(
        r#"{schema}

Write ONE read-only SQL query (a single SELECT, optionally with a leading WITH/CTE) that
retrieves the data needed to answer this question:

"{question}"

Rules:
- Read-only: SELECT only. No INSERT/UPDATE/DELETE/PRAGMA/etc. One statement, no semicolons.
- Prefer aggregates (AVG, SUM, COUNT, MIN, MAX) so the result is small and directly answers the question.
- Cast averages/sums with CAST(... AS REAL) so they come back as decimals.
- Ignore NULL ratings in averages. Add a LIMIT (<= 200) if the query could return many rows.

Respond with JSON only, no markdown:
{{"sql": "SELECT ...", "explanation": "one short line on what this computes"}}{retry_note}"#,
        schema = schema,
        question = question,
        retry_note = retry_note,
    );

    let content = call_openrouter(api_key, &prompt, 0.1, 1024).await?;
    let cleaned = strip_code_fences(&content);

    #[derive(serde::Deserialize)]
    struct Plan {
        sql: String,
        #[serde(default)]
        explanation: String,
    }
    let plan: Plan = serde_json::from_str(cleaned)
        .map_err(|e| format!("Could not parse the model's query: {} - {}", e, cleaned))?;

    let sql = sanitize_sql(&plan.sql)?;
    Ok(QueryPlan { sql, explanation: plan.explanation })
}

/// Validates that `sql` is a single read-only statement and returns it trimmed.
fn sanitize_sql(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return Err("The model returned an empty query.".to_string());
    }
    if trimmed.contains(';') {
        return Err("Only a single statement is allowed.".to_string());
    }
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("select") || lower.starts_with("with")) {
        return Err("Only read-only SELECT queries are allowed.".to_string());
    }
    // Whole-word check against mutating keywords (so `created_at` is fine).
    for token in lower.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if FORBIDDEN_TOKENS.contains(&token) {
            return Err(format!("Query rejected: contains disallowed keyword '{}'.", token));
        }
    }
    Ok(trimmed.to_string())
}

#[allow(clippy::type_complexity)]
async fn run_query(
    pool: &SqlitePool,
    sql: &str,
) -> Result<(Vec<String>, Vec<Vec<String>>, bool), String> {
    let fetched = sqlx::query(sql).fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut columns: Vec<String> = Vec::new();
    if let Some(first) = fetched.first() {
        columns = first.columns().iter().map(|c| c.name().to_string()).collect();
    }

    let truncated = fetched.len() > MAX_DISPLAY_ROWS;
    let rows: Vec<Vec<String>> = fetched
        .iter()
        .take(MAX_DISPLAY_ROWS)
        .map(|row| (0..columns.len()).map(|i| value_to_string(row, i)).collect())
        .collect();

    Ok((columns, rows, truncated))
}

/// Best-effort stringify of a dynamically-typed SQLite cell.
fn value_to_string(row: &sqlx::sqlite::SqliteRow, i: usize) -> String {
    if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
        return v.map(|x| x.to_string()).unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
        return v.map(|x| format!("{}", x)).unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(i) {
        return v.unwrap_or_default();
    }
    String::new()
}

async fn answer_from_rows(
    api_key: &str,
    question: &str,
    explanation: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Result<String, String> {
    // Compact text table for the model.
    let mut table = columns.join(" | ");
    table.push('\n');
    for row in rows.iter().take(MAX_ANSWER_ROWS) {
        table.push_str(&row.join(" | "));
        table.push('\n');
    }
    if rows.is_empty() {
        table.push_str("(no rows)\n");
    }

    let prompt = format!(
        r#"You are a health-tracking assistant for someone managing ME/CFS (chronic fatigue) and
post-exertional malaise (PEM). The user asked:

"{question}"

To answer, this query was run ({explanation}). Its result:

{table}

Write a concise, direct answer in plain English using these figures. Round sensibly and include
units (hours, bpm, steps, /10). If the question is comparative or hypothetical, reason over the
figures and state any assumption. If the result is empty, say no matching data was found. Answer in
1-3 sentences. Do not give medical advice or diagnoses; describe what the data shows. Do not mention SQL."#,
        question = question,
        explanation = explanation,
        table = table,
    );

    call_openrouter(api_key, &prompt, 0.2, 1024).await
}
