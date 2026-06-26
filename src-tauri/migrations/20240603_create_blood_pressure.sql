CREATE TABLE IF NOT EXISTS blood_pressure (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL,                     -- YYYY-MM-DD
    reading_num INTEGER NOT NULL,              -- 1, 2, or 3
    time_taken TEXT,                            -- 24h format
    systolic INTEGER NOT NULL,
    diastolic INTEGER NOT NULL,
    notes TEXT,
    UNIQUE(log_date, reading_num)
);

CREATE INDEX IF NOT EXISTS idx_bp_date ON blood_pressure(log_date);