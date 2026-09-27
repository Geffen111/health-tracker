-- Food & drink log. Not for calorie counting: the point is to see whether anything
-- eaten lines up with fatigue, so an item is just a name and a kind.
CREATE TABLE IF NOT EXISTS foods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    kind TEXT NOT NULL DEFAULT 'food',          -- 'food' | 'drink'
    regular INTEGER NOT NULL DEFAULT 1,         -- 1 = on the quick-add list, 0 = one-off
    active INTEGER NOT NULL DEFAULT 1,          -- 0 = hidden from the lists, history kept
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- A group logs several items at once (e.g. "Usual breakfast" = oats + banana + coffee).
CREATE TABLE IF NOT EXISTS food_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    default_time TEXT,                          -- "07:30", pre-fills the log form
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS food_group_items (
    group_id INTEGER NOT NULL REFERENCES food_groups(id) ON DELETE CASCADE,
    food_id INTEGER NOT NULL REFERENCES foods(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, food_id)
);

-- One row per item eaten. A group expands into one row per member, so every question
-- ("days I had dairy") is asked of food_log alone; group_id only records where it came from.
CREATE TABLE IF NOT EXISTS food_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL,                     -- YYYY-MM-DD
    time_taken TEXT,                            -- "12:30"
    food_id INTEGER NOT NULL REFERENCES foods(id),
    amount TEXT,                                -- free text, optional: "large", "2 cups"
    group_id INTEGER REFERENCES food_groups(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_food_log_date ON food_log(log_date);
CREATE INDEX IF NOT EXISTS idx_food_log_food ON food_log(food_id);
