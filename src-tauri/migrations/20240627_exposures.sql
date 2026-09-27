-- "Exposures of note" on the Activity page: dust, mould, paint fumes, cut grass... Free text,
-- with earlier descriptions offered back as suggestions so the same thing is spelt the
-- same way each time.
CREATE TABLE IF NOT EXISTS exposures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL,                     -- YYYY-MM-DD
    time_taken TEXT,                            -- "14:00", optional
    description TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_exposures_date ON exposures(log_date);

-- Photos attached to an exposure, stored in the database itself so they travel with it
-- (OneDrive sync). The frontend shrinks large photos before they get here.
CREATE TABLE IF NOT EXISTS exposure_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    exposure_id INTEGER NOT NULL REFERENCES exposures(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    data BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_exposure_attachments_exp ON exposure_attachments(exposure_id);
