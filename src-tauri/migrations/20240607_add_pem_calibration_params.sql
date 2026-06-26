-- Additional calibration params from spreadsheet v5
INSERT OR IGNORE INTO pem_calibration (param_name, param_value, description) VALUES
    ('activity_log_start_date', 46150.0, 'Excel serial date when ActivityLog data begins; before this, model falls back to v5 formula (no activity-based load)'),
    ('high_energy_fatigued_multiplier', 0.1, 'Extra recovery debt per High Energy Hour when fatigue >= 6');