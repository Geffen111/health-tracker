-- Blood pressure readings now carry their provenance and the pulse the monitor
-- reported alongside them.
--
-- source: 'watch' on every row the Health Sync CSV import creates. On a hand-typed
-- row it holds whatever device name was entered (an arm cuff, say), so two readings
-- taken minutes apart can be told apart when one monitor is being checked against
-- another. NULL = a manual row with no device recorded, which is every row that
-- existed before this migration.
--
-- The sync only ever matches against rows it owns (source = 'watch'); a manual row
-- is never updated or deleted by it. See commands/csv_import.rs::import_bp.
ALTER TABLE blood_pressure ADD COLUMN pulse INTEGER;
ALTER TABLE blood_pressure ADD COLUMN source TEXT;
