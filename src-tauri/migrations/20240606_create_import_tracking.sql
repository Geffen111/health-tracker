CREATE TABLE IF NOT EXISTS import_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,                      -- 'xlsx_import', 'google_drive_csv'
    filename TEXT,
    rows_imported INTEGER DEFAULT 0,
    rows_skipped INTEGER DEFAULT 0,
    imported_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS samsung_imports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    import_date TEXT NOT NULL,
    source_file TEXT,
    data_type TEXT,                            -- steps, calories, heart_rate, sleep
    record_count INTEGER,
    imported_at TEXT NOT NULL DEFAULT (datetime('now'))
);