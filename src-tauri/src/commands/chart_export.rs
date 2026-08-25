use crate::db::get_data_dir;
use std::path::PathBuf;

/// Chart images live beside the CSV/JSON exports so everything the app writes out
/// is in one place.
fn charts_root() -> Result<PathBuf, String> {
    let root = get_data_dir().join("exports").join("charts");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

/// Keep a caller-supplied stem to characters that are safe in a Windows filename.
fn sanitise(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() { "chart".to_string() } else { trimmed.to_string() }
}

/// Write a PNG rendered in the frontend and open it in the default image viewer.
/// Returns the saved path so the page can show where it went.
#[tauri::command]
pub async fn save_chart_png(bytes: Vec<u8>, file_stem: String) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("The rendered image was empty".into());
    }
    let name = format!(
        "{}-{}.png",
        sanitise(&file_stem),
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let path = charts_root()?.join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;

    // Opened from Rust rather than the JS opener API so no extra capability scope
    // is needed for the exports directory.
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}
