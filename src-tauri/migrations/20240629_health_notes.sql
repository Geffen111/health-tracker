-- Health notes on the Activity page: appointments, tests and anything else worth placing
-- on the dashboard Timeline. A dated title with an optional short note.
CREATE TABLE IF NOT EXISTS health_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL,                     -- YYYY-MM-DD
    note_type TEXT NOT NULL DEFAULT 'other',    -- appointment, test, other
    title TEXT NOT NULL,
    body TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_health_notes_date ON health_notes(log_date);
