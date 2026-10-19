-- Supply tracking: an opt-in "x days left" counter per medication.
-- On hand = supply_count as of supply_date, less supply_per_day for each day since
-- (medications::on_hand_at in Rust, $lib/medSupply.ts on the page — keep them in step).
-- A script fill adds boxes x box size and re-dates the count.
ALTER TABLE medications ADD COLUMN supply_tracking INTEGER NOT NULL DEFAULT 0;
ALTER TABLE medications ADD COLUMN supply_count REAL;           -- tablets on hand at supply_date
ALTER TABLE medications ADD COLUMN supply_date TEXT;            -- YYYY-MM-DD
ALTER TABLE medications ADD COLUMN supply_per_day REAL;         -- tablets used a day
ALTER TABLE medications ADD COLUMN supply_box_size REAL;        -- tablets per box
ALTER TABLE medications ADD COLUMN supply_boxes_per_fill REAL NOT NULL DEFAULT 1;
ALTER TABLE medications ADD COLUMN supply_warn_days INTEGER NOT NULL DEFAULT 7;
ALTER TABLE medications ADD COLUMN script_repeats_left INTEGER; -- NULL = not tracked
ALTER TABLE medications ADD COLUMN needs_script INTEGER NOT NULL DEFAULT 0;

-- Each fill, so a mistaken one can be undone and the last fill date shown.
CREATE TABLE IF NOT EXISTS medication_fills (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    medication_id INTEGER NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    fill_date TEXT NOT NULL,                    -- YYYY-MM-DD
    quantity REAL NOT NULL,                     -- tablets added
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_medication_fills_med ON medication_fills(medication_id);
