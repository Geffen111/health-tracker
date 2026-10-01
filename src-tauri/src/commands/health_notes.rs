//! Health notes on the Activity page — appointments, tests and other dated events — and
//! the event list behind the dashboard Timeline, which merges these with medication
//! changes and exposures.

use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use tauri::State;

const NOTE_TYPES: [&str; 3] = ["appointment", "test", "other"];

#[derive(Debug, Serialize, FromRow)]
pub struct HealthNote {
    pub id: i64,
    pub log_date: String,
    pub note_type: String,
    pub title: String,
    pub body: Option<String>,
}

/// One marker on the Timeline. `kind` is medication / exposure / note; `subtype` is the
/// medication event type or the note type.
#[derive(Debug, Serialize, FromRow)]
pub struct TimelineEvent {
    pub date: String,
    pub kind: String,
    pub subtype: Option<String>,
    pub title: String,
    pub detail: Option<String>,
}

fn clean(note_type: &str, title: &str, body: Option<String>) -> Result<(String, String, Option<String>), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Give the note a title first.".into());
    }
    let note_type = if NOTE_TYPES.contains(&note_type) { note_type } else { "other" };
    let body = body.map(|b| b.trim().to_string()).filter(|b| !b.is_empty());
    Ok((note_type.to_string(), title.to_string(), body))
}

#[tauri::command]
pub async fn get_health_notes_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<HealthNote>, String> {
    sqlx::query_as::<_, HealthNote>(
        "SELECT id, log_date, note_type, title, body FROM health_notes WHERE log_date = ? ORDER BY id",
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_health_note(
    pool: State<'_, SqlitePool>,
    log_date: String,
    note_type: String,
    title: String,
    body: Option<String>,
) -> Result<i64, String> {
    let (note_type, title, body) = clean(&note_type, &title, body)?;
    sqlx::query("INSERT INTO health_notes (log_date, note_type, title, body) VALUES (?, ?, ?, ?)")
        .bind(&log_date).bind(&note_type).bind(&title).bind(&body)
        .execute(&*pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_health_note(
    pool: State<'_, SqlitePool>,
    id: i64,
    note_type: String,
    title: String,
    body: Option<String>,
) -> Result<(), String> {
    let (note_type, title, body) = clean(&note_type, &title, body)?;
    sqlx::query("UPDATE health_notes SET note_type = ?, title = ?, body = ? WHERE id = ?")
        .bind(&note_type).bind(&title).bind(&body).bind(id)
        .execute(&*pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_health_note(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM health_notes WHERE id = ?")
        .bind(id).execute(&*pool).await.map(|_| ()).map_err(|e| e.to_string())
}

/// Every dated event for the Timeline, oldest first: medication starts/stops/dose
/// changes, exposures, and health notes.
#[tauri::command]
pub async fn get_timeline_events(pool: State<'_, SqlitePool>) -> Result<Vec<TimelineEvent>, String> {
    sqlx::query_as::<_, TimelineEvent>(
        "SELECT event_date AS date, 'medication' AS kind, event_type AS subtype, \
                medication_name AS title, \
                COALESCE(detail, CASE WHEN event_type = 'dose_changed' \
                     THEN old_value || ' → ' || new_value END) AS detail, \
                0 AS ord, id \
           FROM medication_history \
         UNION ALL \
         SELECT log_date, 'exposure', NULL, description, time_taken, 1, id FROM exposures \
         UNION ALL \
         SELECT log_date, 'note', note_type, title, body, 2, id FROM health_notes \
         ORDER BY 1, 6, 7",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}
