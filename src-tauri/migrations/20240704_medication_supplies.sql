-- Supply moves off `medications` onto one row per tablet strength / script, so a 12 mg
-- dose taken as an 8 mg + a 4 mg tablet can count both (each with its own box, repeats and
-- expiry). Usage now comes from the doses actually logged, not a fixed daily figure:
-- on hand = base_count, plus fills entered after base_at, less the tablets in each dose
-- logged after base_local (commands/medications.rs, `supply_views`).
CREATE TABLE IF NOT EXISTS medication_supplies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    medication_id INTEGER NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    strength REAL,                              -- per tablet, in the med's dose unit (8 = 8 mg)
    label TEXT,                                 -- optional, e.g. "Brand X"
    base_count REAL NOT NULL DEFAULT 0,         -- tablets counted at base_at
    base_at TEXT NOT NULL,                      -- UTC, compared with medication_fills.created_at
    base_local TEXT NOT NULL,                   -- the same moment, local "YYYY-MM-DD HH:MM", compared with doses
    in_use_since TEXT NOT NULL,                 -- YYYY-MM-DD; usage is averaged from here on
    box_size REAL,
    boxes_per_fill REAL NOT NULL DEFAULT 1,
    warn_days INTEGER NOT NULL DEFAULT 7,
    repeats_left INTEGER,
    needs_script INTEGER NOT NULL DEFAULT 0,
    script_expiry TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_medication_supplies_med ON medication_supplies(medication_id);

-- Carry over the counters set up on the old columns: what the old sum says is on hand
-- today becomes the counted base, now. The strength isn't known — the page asks for it.
INSERT INTO medication_supplies
    (medication_id, base_count, base_at, base_local, in_use_since, box_size, boxes_per_fill,
     warn_days, repeats_left, needs_script, script_expiry)
SELECT id,
       MAX(0, COALESCE(supply_count, 0) - COALESCE(supply_per_day, 0) *
              MAX(0, CAST(julianday(date('now', 'localtime')) - julianday(COALESCE(supply_date, date('now', 'localtime'))) AS INTEGER))),
       datetime('now'), strftime('%Y-%m-%d %H:%M', 'now', 'localtime'), '2000-01-01',
       supply_box_size, supply_boxes_per_fill, supply_warn_days, script_repeats_left, needs_script, script_expiry
  FROM medications
 WHERE supply_tracking = 1;

ALTER TABLE medication_fills ADD COLUMN supply_id INTEGER;
UPDATE medication_fills
   SET supply_id = (SELECT s.id FROM medication_supplies s WHERE s.medication_id = medication_fills.medication_id);

ALTER TABLE medications DROP COLUMN supply_tracking;
ALTER TABLE medications DROP COLUMN supply_count;
ALTER TABLE medications DROP COLUMN supply_date;
ALTER TABLE medications DROP COLUMN supply_per_day;
ALTER TABLE medications DROP COLUMN supply_box_size;
ALTER TABLE medications DROP COLUMN supply_boxes_per_fill;
ALTER TABLE medications DROP COLUMN supply_warn_days;
ALTER TABLE medications DROP COLUMN script_repeats_left;
ALTER TABLE medications DROP COLUMN needs_script;
ALTER TABLE medications DROP COLUMN script_expiry;
