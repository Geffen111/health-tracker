-- Fix miscaled early-2026 step counts.
--
-- When daily logging began (2026-02-21) steps were entered in THOUSANDS — e.g. a day
-- of ~5,000 steps was stored as "5". This ran until late May 2026, when entry switched
-- to raw counts (the watch sync). May is a transition month holding BOTH conventions
-- (values like 4 alongside 13433), so a date cut can't separate them — but a value cut
-- can: every miscaled value is <= 16, while every genuine raw count is >= 1790. A
-- threshold of 1000 sits safely in that gap.
--
-- Multiply the miscaled rows by 1000 to bring them onto the same raw-count scale as the
-- rest of the data. Runs once (sqlx tracks applied migrations), so it cannot double-apply.
--
-- Note: this does not affect the dashboard's monthly-activity card for these months —
-- that uses seeded spreadsheet averages — but it corrects the daily_logs the rolling
-- averages and the PEM model read. Re-run the PEM backfill afterwards if you want the
-- affected days' predictions recomputed from the corrected steps.
UPDATE daily_logs
SET steps = steps * 1000
WHERE steps IS NOT NULL
  AND steps > 0
  AND steps < 1000;
