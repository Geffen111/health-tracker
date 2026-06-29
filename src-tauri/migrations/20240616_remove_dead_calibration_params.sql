-- Remove dead, never-read PEM calibration params left behind by the spreadsheet import.
--
-- The spreadsheet's Calibration sheet uses human-readable param names ("Steps weight
-- (Physical Load)", "Fatigue-map slope", "Debt persistence", ...). The PEM model in
-- commands/pem.rs only ever reads canonical snake_case names (steps_weight,
-- fatigue_from_debt_slope, debt_persistence, ...), so every Title-Case row the import
-- created was inert duplicate weight (58 rows: 31 functional + 27 dead).
--
-- The import is now UPDATE-only (no INSERT), so these will not be recreated. This clears
-- the ones already present. Canonical params are lower-case with underscores and contain
-- no spaces; every dead row contains a space and/or an uppercase letter, so this
-- predicate targets only the non-functional rows and leaves the 31 real params untouched.
DELETE FROM pem_calibration
WHERE param_name LIKE '% %'
   OR param_name <> lower(param_name);
