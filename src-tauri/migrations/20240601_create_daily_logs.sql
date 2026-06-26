CREATE TABLE IF NOT EXISTS daily_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL UNIQUE,              -- YYYY-MM-DD
    day_name TEXT,
    fatigue_desc TEXT,                          -- e.g. "Severe evening + next 1½ days"
    fatigue_rating REAL,                        -- 0-10 scale
    headache_desc TEXT,
    headache_rating REAL,                       -- 0-10 scale
    headache_duration_hours REAL,
    other_symptoms TEXT,                        -- brain fog, malaise, etc.
    my_sleep_rating REAL,                       -- 0-10 self assessment
    phone_sleep_rating REAL,                    -- Samsung/watch sleep rating
    sleep_avg REAL,                             -- average of my + phone
    sleep_time_head_on_pillow REAL,             -- head on pillow hours
    sleep_actual_asleep REAL,                   -- actual sleep time
    sleep_rem REAL,
    sleep_deep REAL,
    sleep_awake REAL,
    steps INTEGER,
    activity_calories REAL,
    ave_resting_hr INTEGER,
    ave_hr INTEGER,
    rostered_hours REAL,
    sick_leave_hours REAL,
    office_hours REAL,
    wfh_hours REAL,
    alcohol_std_drinks REAL,
    multivitamin INTEGER DEFAULT 0,
    vitamin_c INTEGER DEFAULT 0,
    add_meds TEXT,                              -- additional meds notes
    compression_socks INTEGER DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_daily_logs_date ON daily_logs(log_date);