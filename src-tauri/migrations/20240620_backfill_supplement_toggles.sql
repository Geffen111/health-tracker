-- Retire the Supplements toggle by moving its data into the medication log.
--
-- Multivitamin and Vitamin C were tracked as two booleans on daily_logs, flipped
-- from a small "Supplements" card on the Medication page. Both are now proper
-- medications — 'Vitamin C Supp' (active) and 'Multivitamin Supp' (ceased
-- 2026-07-16) — so every toggled-on day is backfilled here as a dose row, and the
-- card is removed from the page. Nothing else read those two columns.
--
-- Dose amount and time come from each medication's own defaults, so a backfilled
-- day is indistinguishable from one logged by hand (Vitamin C 1000mg @ 20:30,
-- Multivitamin 1 tablet @ 20:30).
--
-- Days that already have a dose for that medication are skipped: Vitamin C's real
-- dose logging began 2026-07-16 while the toggle was still in use until 07-20, so
-- 07-16 and 07-17 have both. The NOT EXISTS test is on the DATE, not on
-- (date, time) — a dose logged at an unusual hour must still count as "already
-- logged" rather than gaining a second entry at 20:30.
--
-- The daily_logs.multivitamin / vitamin_c columns are deliberately left in place:
-- they cost nothing and are the only remaining record of what this migration read.
-- Runs once (sqlx tracks applied migrations), so it cannot double-apply. If either
-- medication has been renamed the JOIN matches nothing and this is a no-op.

INSERT INTO medication_doses (medication_id, log_date, time_taken, dose_amount, notes)
SELECT m.id, d.log_date, COALESCE(m.default_time, '20:30'), m.default_dose,
       'Backfilled from the Supplements toggle'
  FROM daily_logs d
  JOIN medications m ON m.name = 'Vitamin C Supp'
 WHERE d.vitamin_c = 1
   AND NOT EXISTS (
       SELECT 1 FROM medication_doses x
        WHERE x.medication_id = m.id AND x.log_date = d.log_date);

INSERT INTO medication_doses (medication_id, log_date, time_taken, dose_amount, notes)
SELECT m.id, d.log_date, COALESCE(m.default_time, '20:30'), m.default_dose,
       'Backfilled from the Supplements toggle'
  FROM daily_logs d
  JOIN medications m ON m.name = 'Multivitamin Supp'
 WHERE d.multivitamin = 1
   AND NOT EXISTS (
       SELECT 1 FROM medication_doses x
        WHERE x.medication_id = m.id AND x.log_date = d.log_date);
