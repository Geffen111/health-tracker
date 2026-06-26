-- Per-day heart-rate range (synced from the watch later, alongside ave_resting_hr / ave_hr).
ALTER TABLE daily_logs ADD COLUMN hr_min INTEGER;
ALTER TABLE daily_logs ADD COLUMN hr_max INTEGER;
