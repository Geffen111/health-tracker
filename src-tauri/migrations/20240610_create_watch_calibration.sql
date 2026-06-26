-- Log of watch/HR-sensor calibrations, used to flag the ~30-day recalibration.
CREATE TABLE IF NOT EXISTS watch_calibration (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cal_date TEXT NOT NULL,                     -- YYYY-MM-DD
    cal_time TEXT,                              -- HH:MM
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_watch_calibration_date ON watch_calibration(cal_date);
