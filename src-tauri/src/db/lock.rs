//! "Open on another computer" lock for a synced data folder.
//!
//! `health.lock` holds this computer's name and a heartbeat refreshed every
//! HEARTBEAT_SECS while the app runs, and is removed on exit. Another computer whose
//! lock is fresher than STALE_SECS is taken to still have the database open: launching
//! asks first rather than opening it underneath them. A crash or a sleeping laptop leaves
//! a lock that goes stale on its own, and a lock left by *this* computer never blocks it.
//!
//! It is advisory: the sync service may deliver the lock a little late, so it narrows the
//! window for conflicting copies rather than closing it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const HEARTBEAT_SECS: u64 = 120;
/// Long enough to outlast sync delays and a missed heartbeat or two.
const STALE_SECS: u64 = 10 * 60;
const LOCK_FILE: &str = "health.lock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub machine: String,
    /// Unix seconds when that computer opened the data.
    pub opened: u64,
    /// Unix seconds of its last heartbeat.
    pub heartbeat: u64,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn machine_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "another computer".to_string())
}

fn path(dir: &Path) -> PathBuf {
    dir.join(LOCK_FILE)
}

pub fn read(dir: &Path) -> Option<LockInfo> {
    serde_json::from_str(&std::fs::read_to_string(path(dir)).ok()?).ok()
}

/// A fresh lock held by a different computer, if there is one.
pub fn held_elsewhere(dir: &Path) -> Option<LockInfo> {
    read(dir).filter(|l| {
        !l.machine.eq_ignore_ascii_case(&machine_name()) && now().saturating_sub(l.heartbeat) < STALE_SECS
    })
}

/// Take (or refresh) the lock for this computer, keeping `opened` from `since`.
pub fn write(dir: &Path, since: u64) -> Result<(), String> {
    let info = LockInfo { machine: machine_name(), opened: since, heartbeat: now() };
    std::fs::write(path(dir), serde_json::to_string(&info).unwrap())
        .map_err(|e| format!("Couldn't write the lock file: {}", e))
}

/// Remove the lock, but only if it is still ours.
pub fn release(dir: &Path) {
    if read(dir).is_some_and(|l| l.machine.eq_ignore_ascii_case(&machine_name())) {
        let _ = std::fs::remove_file(path(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ht-lock-test-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(d: &Path, machine: &str, age: u64) {
        let info = LockInfo { machine: machine.into(), opened: 0, heartbeat: now() - age };
        std::fs::write(path(d), serde_json::to_string(&info).unwrap()).unwrap();
    }

    #[test]
    fn another_computer_blocks_only_while_fresh() {
        let d = dir("fresh");
        assert!(held_elsewhere(&d).is_none());
        put(&d, "OTHER-PC-XYZ", 60);
        assert_eq!(held_elsewhere(&d).unwrap().machine, "OTHER-PC-XYZ");
        put(&d, "OTHER-PC-XYZ", STALE_SECS + 5);
        assert!(held_elsewhere(&d).is_none());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn own_lock_never_blocks_and_release_leaves_others_alone() {
        let d = dir("own");
        write(&d, now()).unwrap();
        assert!(held_elsewhere(&d).is_none());
        release(&d);
        assert!(read(&d).is_none());
        put(&d, "OTHER-PC-XYZ", 10);
        release(&d);
        assert!(read(&d).is_some());
        std::fs::remove_dir_all(&d).ok();
    }
}
