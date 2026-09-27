-- A free-text note per day on the Medication page ("took the Dex late — slept in", say).
-- One row per day; clearing the note deletes the row.
CREATE TABLE IF NOT EXISTS medication_notes (
    log_date TEXT PRIMARY KEY,                  -- YYYY-MM-DD
    note TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- An occasional (as-needed) medication isn't "started" like a daily one — it's added to
-- the list. New ones are logged as 'added'; relabel the ones already recorded, but only
-- where the entry still carries the generated wording (never a note typed over it).
UPDATE medication_history
   SET event_type = 'added', detail = 'Occasional medication added'
 WHERE event_type = 'started'
   AND detail = 'Started ' || medication_name
   AND medication_id IN (SELECT id FROM medications WHERE med_type = 'occasional' OR category = 'PRN');
