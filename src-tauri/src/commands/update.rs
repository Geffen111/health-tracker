// Update check. The build-info.json marker lives as a GitHub *release asset*,
// which 302-redirects to release-assets.githubusercontent.com — and that host
// sends no CORS header, so a fetch() from the webview is silently blocked. We
// therefore fetch it from Rust (no CORS) and just hand back the commit string.

/// Returns the commit the latest published build was built from, or None if the
/// marker can't be reached (offline, no release yet, etc.). The frontend compares
/// it against the commit baked into the running build.
#[tauri::command]
pub async fn latest_build_commit(owner: String, repo: String) -> Result<Option<String>, String> {
    let url = format!(
        "https://github.com/{}/{}/releases/download/latest/build-info.json",
        owner, repo
    );
    let client = reqwest::Client::new();
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !resp.status().is_success() {
        return Ok(None);
    }
    let text = match resp.text().await {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    // Tolerate the UTF-8 BOM that PowerShell's Set-Content writes.
    let text = text.trim_start_matches('\u{feff}');
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    Ok(value
        .get("commit")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string()))
}
