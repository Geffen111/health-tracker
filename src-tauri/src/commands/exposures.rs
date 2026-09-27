//! "Exposures of note" on the Activity page — dust, mould, paint fumes and the like — with
//! optional photo attachments. The photos live in the database (BLOB) so they sync with
//! it; they cross the IPC boundary as base64 because a JSON number array would be ~4x
//! the size of the image.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use tauri::State;

/// Hard ceiling on one attachment after the frontend has shrunk it. Anything past this
/// is almost certainly not a photo (or the resize failed), and every byte of it would be
/// re-uploaded by OneDrive with the database.
const MAX_ATTACHMENT_BYTES: usize = 15 * 1024 * 1024;

#[derive(Debug, Serialize, FromRow)]
pub struct AttachmentMeta {
    pub id: i64,
    pub exposure_id: i64,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct Exposure {
    pub id: i64,
    pub log_date: String,
    pub time_taken: Option<String>,
    pub description: String,
    pub attachments: Vec<AttachmentMeta>,
}

#[derive(Debug, Serialize)]
pub struct AttachmentData {
    pub file_name: String,
    pub mime_type: String,
    pub data_base64: String,
}

#[tauri::command]
pub async fn get_exposures_for_date(
    pool: State<'_, SqlitePool>,
    date: String,
) -> Result<Vec<Exposure>, String> {
    let rows: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, log_date, time_taken, description FROM exposures WHERE log_date = ? \
         ORDER BY time_taken IS NULL, time_taken, id",
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let atts = sqlx::query_as::<_, AttachmentMeta>(
        "SELECT a.id, a.exposure_id, a.file_name, a.mime_type, \
                CAST(LENGTH(a.data) AS INTEGER) AS size_bytes \
         FROM exposure_attachments a JOIN exposures e ON e.id = a.exposure_id \
         WHERE e.log_date = ? ORDER BY a.id",
    )
    .bind(&date)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out: Vec<Exposure> = rows
        .into_iter()
        .map(|(id, log_date, time_taken, description)| Exposure {
            id, log_date, time_taken, description, attachments: Vec::new(),
        })
        .collect();
    for a in atts {
        if let Some(e) = out.iter_mut().find(|e| e.id == a.exposure_id) {
            e.attachments.push(a);
        }
    }
    Ok(out)
}

/// Descriptions used before, most-used first — the suggestions under the text field.
#[tauri::command]
pub async fn list_exposure_descriptions(pool: State<'_, SqlitePool>) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT description FROM exposures GROUP BY description COLLATE NOCASE \
         ORDER BY COUNT(*) DESC, MAX(log_date) DESC",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

#[tauri::command]
pub async fn add_exposure(
    pool: State<'_, SqlitePool>,
    log_date: String,
    time_taken: Option<String>,
    description: String,
) -> Result<i64, String> {
    let description = description.trim();
    if description.is_empty() {
        return Err("Describe the exposure first.".into());
    }
    let time_taken = time_taken.filter(|t| !t.trim().is_empty());
    sqlx::query("INSERT INTO exposures (log_date, time_taken, description) VALUES (?, ?, ?)")
        .bind(&log_date).bind(&time_taken).bind(description)
        .execute(&*pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_exposure(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM exposure_attachments WHERE exposure_id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM exposures WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn add_exposure_attachment(
    pool: State<'_, SqlitePool>,
    exposure_id: i64,
    file_name: String,
    mime_type: String,
    data_base64: String,
) -> Result<i64, String> {
    let bytes = B64.decode(data_base64.as_bytes()).map_err(|e| format!("Unreadable image data: {}", e))?;
    if bytes.is_empty() {
        return Err("The image was empty.".into());
    }
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "That file is {:.1} MB — too large to store (limit {} MB).",
            bytes.len() as f64 / 1_048_576.0,
            MAX_ATTACHMENT_BYTES / 1_048_576
        ));
    }
    sqlx::query(
        "INSERT INTO exposure_attachments (exposure_id, file_name, mime_type, data) VALUES (?, ?, ?, ?)",
    )
    .bind(exposure_id).bind(&file_name).bind(&mime_type).bind(&bytes)
    .execute(&*pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

async fn load_attachment(pool: &SqlitePool, id: i64) -> Result<(String, String, Vec<u8>), String> {
    sqlx::query_as("SELECT file_name, mime_type, data FROM exposure_attachments WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "That attachment no longer exists.".to_string())
}

#[tauri::command]
pub async fn get_exposure_attachment(pool: State<'_, SqlitePool>, id: i64) -> Result<AttachmentData, String> {
    let (file_name, mime_type, data) = load_attachment(&pool, id).await?;
    Ok(AttachmentData { file_name, mime_type, data_base64: B64.encode(data) })
}

/// Write the attachment to a temp file and hand it to the default app — for formats the
/// webview can't show (HEIC, say), or to zoom in properly.
#[tauri::command]
pub async fn open_exposure_attachment(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    let (file_name, _, data) = load_attachment(&pool, id).await?;
    let safe: String = file_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let dir = std::env::temp_dir().join("health-tracker-attachments");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}-{}", id, safe));
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_exposure_attachment(pool: State<'_, SqlitePool>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM exposure_attachments WHERE id = ?")
        .bind(id).execute(&*pool).await.map_err(|e| e.to_string())?;
    Ok(())
}
