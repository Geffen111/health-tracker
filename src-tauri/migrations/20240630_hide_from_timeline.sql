-- "Hide from timeline" on everything that puts a marker on the dashboard Timeline, for
-- entries worth keeping but not worth showing there.
ALTER TABLE exposures ADD COLUMN hide_from_timeline INTEGER NOT NULL DEFAULT 0;
ALTER TABLE health_notes ADD COLUMN hide_from_timeline INTEGER NOT NULL DEFAULT 0;
ALTER TABLE medication_history ADD COLUMN hide_from_timeline INTEGER NOT NULL DEFAULT 0;
