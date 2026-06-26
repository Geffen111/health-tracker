-- Lifecycle log for medications: started / ceased / dose changes over time.
-- medication_name is denormalised so a history entry survives the med being deleted.
CREATE TABLE IF NOT EXISTS medication_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    medication_id INTEGER REFERENCES medications(id) ON DELETE SET NULL,
    medication_name TEXT NOT NULL,
    event_type TEXT NOT NULL,                   -- started, ceased, reactivated, dose_changed, note
    event_date TEXT NOT NULL,                   -- YYYY-MM-DD
    detail TEXT,                                -- editable free text, e.g. "Ceased — side effects"
    old_value TEXT,                             -- prior value for dose_changed (e.g. "15")
    new_value TEXT,                             -- new value for dose_changed (e.g. "10")
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_medication_history_med ON medication_history(medication_id);
CREATE INDEX IF NOT EXISTS idx_medication_history_date ON medication_history(event_date);
