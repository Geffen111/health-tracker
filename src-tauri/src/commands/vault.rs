// Read-only browser for the Obsidian "Health Records" vault.
//
// The vault is the user's own curated markdown (pathology results, health
// topics, medications, timeline, practitioners…), kept canonical in Obsidian and
// synced via OneDrive. The app only ever READS it — there is no write-back, so
// nothing here can disturb the vault or race with Obsidian/OneDrive sync.
//
// All file access lives in Rust (same as csv_import) so we never have to widen
// the Tauri fs-plugin scope. `get_vault_index` returns lightweight metadata for
// the whole tree (parsed from each note's YAML frontmatter); `read_vault_note`
// returns one note's markdown body for the frontend to render.

use crate::commands::settings;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const VAULT_ROOT_SETTING: &str = "vault_root";
const DEFAULT_VAULT: &str = "C:\\Users\\gavin\\OneDrive\\Obsidian\\Health-Records";

/// Configured vault root, or the default OneDrive location.
fn vault_root() -> PathBuf {
    PathBuf::from(settings::setting_str(VAULT_ROOT_SETTING).unwrap_or_else(|| DEFAULT_VAULT.to_string()))
}

#[derive(Serialize)]
pub struct VaultNote {
    /// Forward-slash path relative to the vault root (the id used by read_vault_note).
    pub rel_path: String,
    /// Display title: first markdown heading, else the prettified filename.
    pub title: String,
    /// Top-level folder, e.g. "Pathology Results"; empty string for root notes.
    pub folder: String,
    /// Filename stem, used to resolve `[[wikilinks]]`.
    pub stem: String,
    /// Frontmatter `type`, if present.
    pub note_type: Option<String>,
    /// Best date for the note: a YYYY-MM-DD in the filename, else frontmatter `date`.
    pub date: Option<String>,
    /// Frontmatter `tags`.
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct VaultIndex {
    pub root: String,
    pub exists: bool,
    pub notes: Vec<VaultNote>,
}

#[derive(Serialize)]
pub struct VaultNoteContent {
    pub rel_path: String,
    pub title: String,
    /// Markdown body with the YAML frontmatter stripped.
    pub body: String,
}

/// Index every `.md` note in the vault (metadata only — bodies are not loaded).
#[tauri::command]
pub async fn get_vault_index() -> Result<VaultIndex, String> {
    let root = vault_root();
    if !root.is_dir() {
        return Ok(VaultIndex { root: root.to_string_lossy().into_owned(), exists: false, notes: vec![] });
    }

    let mut notes: Vec<VaultNote> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            // Skip hidden / Obsidian-internal directories (.obsidian, .trash, .makemd, .space…).
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(note) = index_note(&root, &path) {
                    notes.push(note);
                }
            }
        }
    }

    // Newest first where a date is known, then alphabetical by title.
    notes.sort_by(|a, b| match (&b.date, &a.date) {
        (Some(bd), Some(ad)) => bd.cmp(ad).then_with(|| a.title.cmp(&b.title)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });

    Ok(VaultIndex { root: root.to_string_lossy().into_owned(), exists: true, notes })
}

fn index_note(root: &Path, path: &Path) -> Option<VaultNote> {
    let content = fs::read_to_string(path).ok()?;
    let (frontmatter, body) = split_frontmatter(&content);

    let rel = path.strip_prefix(root).ok()?;
    let rel_path = rel.to_string_lossy().replace('\\', "/");
    let folder = rel
        .components()
        .next()
        .filter(|_| rel.components().count() > 1)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = path.file_stem()?.to_string_lossy().into_owned();

    let note_type = frontmatter_value(frontmatter, "type");
    let fm_date = frontmatter_value(frontmatter, "date");
    let date = date_from_stem(&stem).or(fm_date);
    let tags = frontmatter_tags(frontmatter);
    let title = first_heading(body).unwrap_or_else(|| prettify_stem(&stem));

    Some(VaultNote { rel_path, title, folder, stem, note_type, date, tags })
}

/// Read one note's markdown body. The path is validated to stay inside the vault.
#[tauri::command]
pub async fn read_vault_note(rel_path: String) -> Result<VaultNoteContent, String> {
    let root = vault_root();
    // Reject absolute paths and `..` traversal before touching the filesystem.
    let rel = PathBuf::from(&rel_path);
    if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("Invalid note path".into());
    }
    let full = root.join(&rel);
    if full.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("Not a markdown note".into());
    }
    // Defence in depth: canonicalize and confirm the file is under the vault root.
    let canon_root = fs::canonicalize(&root).map_err(|e| format!("Vault not found: {}", e))?;
    let canon_full = fs::canonicalize(&full).map_err(|_| "Note not found".to_string())?;
    if !canon_full.starts_with(&canon_root) {
        return Err("Note is outside the vault".into());
    }

    let content = fs::read_to_string(&canon_full).map_err(|e| format!("Could not read note: {}", e))?;
    let (_, body) = split_frontmatter(&content);
    let stem = full.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let title = first_heading(body).unwrap_or_else(|| prettify_stem(&stem));
    Ok(VaultNoteContent {
        rel_path: rel_path.replace('\\', "/"),
        title,
        body: body.to_string(),
    })
}

// ── small markdown / frontmatter helpers ──

/// Split a `---`-delimited YAML frontmatter block off the front of a note.
/// Returns (frontmatter_lines, body). If there is no frontmatter, the first
/// element is empty and the body is the whole content.
fn split_frontmatter(content: &str) -> (&str, &str) {
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    if let Some(rest) = trimmed.strip_prefix("---\n").or_else(|| trimmed.strip_prefix("---\r\n")) {
        // Find the closing delimiter line.
        if let Some(end) = find_closing_delim(rest) {
            let fm = &rest[..end.0];
            let body = &rest[end.1..];
            return (fm, body.trim_start_matches(['\n', '\r']));
        }
    }
    ("", trimmed)
}

/// Returns (offset_of_delim_start, offset_after_delim_line) for a line that is
/// exactly `---` within `s`.
fn find_closing_delim(s: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            return Some((offset, offset + line.len()));
        }
        offset += line.len();
    }
    None
}

/// Read a scalar `key: value` from frontmatter lines.
fn frontmatter_value(fm: &str, key: &str) -> Option<String> {
    for line in fm.lines() {
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(val) = rest.strip_prefix(':') {
                let v = val.trim().trim_matches(['"', '\'']).to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Collect a frontmatter `tags:` value, whether it's an inline `[a, b]` list or
/// a YAML block of `  - tag` lines.
fn frontmatter_tags(fm: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_block = false;
    for line in fm.lines() {
        let trimmed = line.trim();
        if in_block {
            if let Some(item) = trimmed.strip_prefix("- ") {
                tags.push(clean_tag(item));
                continue;
            }
            // A non-indented, non-list line ends the tags block.
            if !line.starts_with(char::is_whitespace) {
                in_block = false;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("tags:") {
            let rest = rest.trim();
            if rest.is_empty() {
                in_block = true;
            } else if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                for part in inner.split(',') {
                    let t = clean_tag(part);
                    if !t.is_empty() {
                        tags.push(t);
                    }
                }
            } else {
                tags.push(clean_tag(rest));
            }
        }
    }
    tags.retain(|t| !t.is_empty());
    tags
}

fn clean_tag(s: &str) -> String {
    s.trim().trim_matches(['"', '\'']).trim_start_matches('#').to_string()
}

/// First markdown ATX heading (`# …`) in the body, trimmed of leading `#`s.
fn first_heading(body: &str) -> Option<String> {
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            let title = t.trim_start_matches('#').trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// "B12_Folate_Melbourne_2026-01-15" → "B12 Folate Melbourne 2026-01-15".
fn prettify_stem(stem: &str) -> String {
    stem.replace(['_'], " ")
}

/// Extract a trailing/embedded `YYYY-MM-DD` from a filename stem.
fn date_from_stem(stem: &str) -> Option<String> {
    let bytes = stem.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    for start in 0..=bytes.len() - 10 {
        let slice = &stem[start..start + 10];
        if is_iso_date(slice) {
            return Some(slice.to_string());
        }
    }
    None
}

fn is_iso_date(s: &str) -> bool {
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
    fn strips_frontmatter() {
        let c = "---\ntype: pathology_result\ndate: 2026-05-31\n---\n\n# Title\nbody";
        let (fm, body) = split_frontmatter(c);
        assert!(fm.contains("type: pathology_result"));
        assert_eq!(body, "# Title\nbody");
        assert_eq!(frontmatter_value(fm, "type").as_deref(), Some("pathology_result"));
    }

    #[test]
    fn no_frontmatter_passthrough() {
        let c = "# Just a note\ncontent";
        let (fm, body) = split_frontmatter(c);
        assert_eq!(fm, "");
        assert_eq!(body, c);
    }

    #[test]
    fn parses_block_and_inline_tags() {
        let block = "tags:\n  - pathology\n  - blood-work\nother: x";
        assert_eq!(frontmatter_tags(block), vec!["pathology", "blood-work"]);
        let inline = "tags: [pathology, inflammation]";
        assert_eq!(frontmatter_tags(inline), vec!["pathology", "inflammation"]);
    }

    #[test]
    fn date_from_filename() {
        assert_eq!(date_from_stem("CRP_2026-03-03").as_deref(), Some("2026-03-03"));
        assert_eq!(date_from_stem("Medications"), None);
    }

    #[test]
    fn heading_then_fallback() {
        assert_eq!(first_heading("\n# Hello\nx").as_deref(), Some("Hello"));
        assert_eq!(prettify_stem("A_B_C"), "A B C");
    }
}
