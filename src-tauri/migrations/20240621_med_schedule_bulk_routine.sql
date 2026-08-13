ALTER TABLE medication_schedule ADD COLUMN bulk_routine TEXT;

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO app_settings (key, value) VALUES ('morning_bulk_time', '07:00');
INSERT INTO app_settings (key, value) VALUES ('evening_bulk_time', '20:00');
