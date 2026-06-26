CREATE TABLE IF NOT EXISTS medications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,                  -- e.g. "Dexamphetamine", "Ozempic"
    short_code TEXT,                            -- e.g. "Dex", "Esc", "Cand"
    default_dose REAL,                          -- default dose amount (e.g. 15)
    dose_unit TEXT DEFAULT 'mg',                -- mg, mcg, tablets, mL
    category TEXT,                              -- stimulant, GLP-1, antihistamine, BP, etc.
    active INTEGER DEFAULT 1,                   -- 1=in use, 0=archived
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS medication_doses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    medication_id INTEGER NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    log_date TEXT NOT NULL,                     -- YYYY-MM-DD
    time_taken TEXT,                            -- 24h format e.g. "07:00"
    dose_amount REAL,
    notes TEXT,
    UNIQUE(medication_id, log_date, time_taken)
);

CREATE INDEX IF NOT EXISTS idx_medication_doses_date ON medication_doses(log_date);
CREATE INDEX IF NOT EXISTS idx_medication_doses_med ON medication_doses(medication_id);