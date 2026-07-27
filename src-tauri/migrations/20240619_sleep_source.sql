-- Marks who owns a day's sleep breakdown. 'manual' means it was typed in on the
-- Sleep page, and the Health Sync CSV import must leave those five columns alone
-- (see commands/csv_import.rs::upsert_day). NULL = sync-owned, the default for
-- every row that existed before this migration.
ALTER TABLE daily_logs ADD COLUMN sleep_source TEXT;
