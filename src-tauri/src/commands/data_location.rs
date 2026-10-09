//! Choosing the data folder and opening the database (see db/mod.rs and db/lock.rs).
//!
//! The pool used to be opened unconditionally in `setup`. Now it is opened only once a
//! folder is known and no other computer has it open; until then the frontend shows the
//! setup screen (`+layout.svelte` renders no page before `get_startup_state` says ready),
//! and no command that needs the database is called.

use crate::db::{self, lock, DB_FILE};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Default)]
pub struct Startup {
    pub dir: Option<PathBuf>,
    pub ready: bool,
    pub opened: u64,
    pub error: Option<String>,
    pub lock: Option<lock::LockInfo>,
}

pub type StartupState = Mutex<Startup>;

#[derive(Debug, Serialize)]
pub struct StartupInfo {
    /// "ready", "needs_location" (first run), "locked" (open on another computer) or
    /// "error" (the folder couldn't be opened).
    pub status: &'static str,
    pub data_dir: Option<String>,
    pub lock: Option<lock::LockInfo>,
    pub error: Option<String>,
    pub machine: String,
}

fn info(s: &Startup) -> StartupInfo {
    let status = if s.ready {
        "ready"
    } else if s.error.is_some() {
        "error"
    } else if s.lock.is_some() {
        "locked"
    } else if s.dir.is_none() {
        "needs_location"
    } else {
        "error"
    };
    StartupInfo {
        status,
        data_dir: s.dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
        lock: s.lock.clone(),
        error: s.error.clone(),
        machine: lock::machine_name(),
    }
}

/// Open the database in `dir` unless another computer holds it (`force` opens anyway),
/// take the lock and start its heartbeat. Records the outcome in the startup state.
pub async fn open(app: &AppHandle, dir: PathBuf, force: bool) -> StartupInfo {
    let state = app.state::<StartupState>();
    {
        let mut s = state.lock().unwrap();
        if s.ready {
            return info(&s);
        }
        s.dir = Some(dir.clone());
        s.error = None;
        s.lock = if force { None } else { lock::held_elsewhere(&dir) };
        if s.lock.is_some() {
            return info(&s);
        }
    }
    let result = db::open_db(&dir).await;
    let mut s = state.lock().unwrap();
    match result {
        Ok(pool) => {
            let opened = lock::now();
            if let Err(e) = lock::write(&dir, opened) {
                eprintln!("{}", e); // advisory only: a read-only folder still works
            }
            app.manage(pool);
            s.ready = true;
            s.opened = opened;
            start_heartbeat(app.clone(), dir, opened);
        }
        Err(e) => s.error = Some(e),
    }
    info(&s)
}

/// Keep the lock fresh. If another computer has taken the folder over (it found our lock
/// stale — e.g. this laptop slept with the app open — or chose "open anyway"), stop and
/// tell the window, which asks to be closed rather than write underneath them.
fn start_heartbeat(app: AppHandle, dir: PathBuf, opened: u64) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(lock::HEARTBEAT_SECS));
        match lock::read(&dir) {
            Some(l) if !l.machine.eq_ignore_ascii_case(&lock::machine_name()) => {
                let _ = app.emit("data-lock-lost", l.machine);
                return;
            }
            _ => {
                let _ = lock::write(&dir, opened);
            }
        }
    });
}

/// Called on exit: give the folder up for the next computer.
pub fn release(app: &AppHandle) {
    if let Some(state) = app.try_state::<StartupState>() {
        let s = state.lock().unwrap();
        if let (true, Some(dir)) = (s.ready, s.dir.as_ref()) {
            lock::release(dir);
        }
    }
}

#[tauri::command]
pub fn get_startup_state(state: State<'_, StartupState>) -> StartupInfo {
    info(&state.lock().unwrap())
}

/// Try the current folder again (after the other computer has closed it).
#[tauri::command]
pub async fn retry_startup(app: AppHandle) -> Result<StartupInfo, String> {
    let dir = app.state::<StartupState>().lock().unwrap().dir.clone();
    match dir {
        Some(dir) => Ok(open(&app, dir, false).await),
        None => Ok(info(&app.state::<StartupState>().lock().unwrap())),
    }
}

#[tauri::command]
pub async fn open_despite_lock(app: AppHandle) -> Result<StartupInfo, String> {
    let dir = app.state::<StartupState>().lock().unwrap().dir.clone().ok_or("No data folder chosen.")?;
    Ok(open(&app, dir, true).await)
}

/// First run: use `path` for data. `mode` "new" starts a fresh log there (or carries on
/// with one already there); "existing" requires the health.db from another computer.
#[tauri::command]
pub async fn choose_data_location(app: AppHandle, path: String, mode: String) -> Result<StartupInfo, String> {
    let dir = PathBuf::from(path.trim());
    if dir.as_os_str().is_empty() {
        return Err("Choose a folder.".into());
    }
    if mode == "existing" && !dir.join(DB_FILE).exists() {
        return Err(format!(
            "There's no {} in {}. Pick the folder your other computer uses — it holds health.db and settings.json.",
            DB_FILE,
            dir.display()
        ));
    }
    db::set_configured_dir(&dir)?;
    Ok(open(&app, dir, false).await)
}

#[derive(Debug, Serialize)]
pub struct LocationOption {
    /// "local" or "cloud".
    pub kind: &'static str,
    pub label: String,
    pub path: String,
    /// A health.db is already there — offered as "existing data".
    pub has_data: bool,
    pub modified: Option<String>,
}

fn option(kind: &'static str, label: &str, dir: PathBuf) -> LocationOption {
    let db = dir.join(DB_FILE);
    let modified = std::fs::metadata(&db).and_then(|m| m.modified()).ok().map(|t| {
        chrono::DateTime::<chrono::Local>::from(t).format("%-d %b %Y, %H:%M").to_string()
    });
    LocationOption { kind, label: label.into(), path: dir.to_string_lossy().into_owned(), has_data: db.exists(), modified }
}

/// Folders worth offering: this computer, plus any cloud-synced folder that exists here.
#[tauri::command]
pub fn suggest_data_locations() -> Vec<LocationOption> {
    let mut out = vec![option("local", "This computer only", db::default_local_data_dir())];
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut cloud = |label: &str, root: PathBuf, sub: &[&str]| {
        if root.is_dir() && !seen.contains(&root) {
            seen.push(root.clone());
            out.push(option("cloud", label, sub.iter().fold(root, |p, s| p.join(s))));
        }
    };
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Ok(root) = std::env::var(var) {
            cloud("OneDrive", PathBuf::from(root), &["Apps", "HealthTracker"]);
        }
    }
    if let Some(home) = dirs::home_dir() {
        cloud("Dropbox", home.join("Dropbox"), &["HealthTracker"]);
        cloud("Google Drive", home.join("Google Drive").join("My Drive"), &["HealthTracker"]);
        cloud("Google Drive", home.join("My Drive"), &["HealthTracker"]);
        cloud("iCloud Drive", home.join("Library/Mobile Documents/com~apple~CloudDocs"), &["HealthTracker"]);
    }
    if cfg!(windows) {
        for letter in ['G', 'H', 'I'] {
            cloud("Google Drive", PathBuf::from(format!("{}:\\My Drive", letter)), &["HealthTracker"]);
        }
    }
    out
}

#[derive(Debug, Serialize)]
pub struct DataLocation {
    pub path: String,
    pub machine: String,
    /// Other `health*.db` files in the folder — usually conflicting copies from a sync service.
    pub stray_copies: Vec<String>,
}

#[tauri::command]
pub fn get_data_location() -> DataLocation {
    let dir = db::get_data_dir();
    let mut stray_copies: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != DB_FILE && n.to_lowercase().starts_with("health") && n.to_lowercase().ends_with(".db"))
                .collect()
        })
        .unwrap_or_default();
    stray_copies.sort();
    DataLocation { path: dir.to_string_lossy().into_owned(), machine: lock::machine_name(), stray_copies }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Settings: move to another folder and restart. "copy" takes a consistent copy of the
/// current database (VACUUM INTO, safe while open) plus settings.json to a folder without
/// one; "existing" switches to a folder that already has data. The old folder is left as it
/// was, as a backup.
#[tauri::command]
pub async fn change_data_location(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    path: String,
    mode: String,
) -> Result<(), String> {
    let target = PathBuf::from(path.trim());
    let current = db::get_data_dir();
    if target.as_os_str().is_empty() || same_dir(&target, &current) {
        return Err("That's the folder already in use.".into());
    }
    let target_db = target.join(DB_FILE);
    match mode.as_str() {
        "copy" => {
            if target_db.exists() {
                return Err("That folder already has Health Tracker data. Use it as it is, or pick an empty folder.".into());
            }
            std::fs::create_dir_all(&target).map_err(|e| format!("Couldn't create {}: {}", target.display(), e))?;
            sqlx::query("VACUUM INTO ?")
                .bind(target_db.to_string_lossy().into_owned())
                .execute(&*pool)
                .await
                .map_err(|e| format!("Couldn't copy the database: {}", e))?;
            let settings = current.join("settings.json");
            if settings.exists() {
                std::fs::copy(&settings, target.join("settings.json"))
                    .map_err(|e| format!("Copied the database, but not settings.json: {}", e))?;
            }
        }
        "existing" => {
            if !target_db.exists() {
                return Err(format!("There's no {} in {}.", DB_FILE, target.display()));
            }
        }
        _ => return Err("Unknown mode.".into()),
    }
    db::set_configured_dir(&target)?;
    release(&app);
    app.restart();
}

/// Whether a folder already holds Health Tracker data (Settings words the move by it).
#[tauri::command]
pub fn folder_has_data(path: String) -> bool {
    PathBuf::from(path.trim()).join(DB_FILE).exists()
}
