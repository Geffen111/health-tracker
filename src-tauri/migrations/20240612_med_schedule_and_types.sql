-- Split medications into regular vs occasional, give each a default time, and add
-- a per-medication dose schedule (so e.g. Dexamphetamine can offer 3 quick-add
-- buttons with its typical dose+time pre-filled).

ALTER TABLE medications ADD COLUMN default_time TEXT;        -- typical time, "07:00"
ALTER TABLE medications ADD COLUMN med_type TEXT;            -- 'regular' | 'occasional'

-- Backfill: anything previously flagged PRN is occasional; everything else regular.
UPDATE medications SET med_type = CASE WHEN category = 'PRN' THEN 'occasional' ELSE 'regular' END
 WHERE med_type IS NULL;

CREATE TABLE IF NOT EXISTS medication_schedule (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    medication_id INTEGER NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 1,      -- 1,2,3 — drives "Add dose 1/2/3"
    label TEXT,                                 -- e.g. "Morning", "Midday"
    dose_amount REAL,                           -- typical dose for this slot
    time_of_day TEXT,                           -- typical time, "07:00"
    UNIQUE(medication_id, sort_order)
);

-- Seed the regular medications (idempotent on the unique name).
INSERT OR IGNORE INTO medications (name, default_dose, dose_unit, med_type, default_time, active) VALUES
    ('Dexamphetamine', 15, 'mg', 'regular', '07:00', 1),
    ('Escitalopram',    5, 'mg', 'regular', '07:00', 1),
    ('Candesartan',     8, 'mg', 'regular', '07:00', 1),
    ('Melatonin',       2, 'mg', 'regular', '20:30', 1);

-- Seed each regular's default dose schedule (idempotent on medication_id+sort_order).
INSERT OR IGNORE INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
    SELECT id, 1, 'Morning', 15, '07:00' FROM medications WHERE name = 'Dexamphetamine';
INSERT OR IGNORE INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
    SELECT id, 2, 'Midday', 10, '11:00' FROM medications WHERE name = 'Dexamphetamine';
INSERT OR IGNORE INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
    SELECT id, 3, 'Afternoon', 10, '14:00' FROM medications WHERE name = 'Dexamphetamine';
INSERT OR IGNORE INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
    SELECT id, 1, 'Morning', 5, '07:00' FROM medications WHERE name = 'Escitalopram';
INSERT OR IGNORE INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
    SELECT id, 1, 'Morning', 8, '07:00' FROM medications WHERE name = 'Candesartan';
INSERT OR IGNORE INTO medication_schedule (medication_id, sort_order, label, dose_amount, time_of_day)
    SELECT id, 1, 'Evening', 2, '20:30' FROM medications WHERE name = 'Melatonin';
