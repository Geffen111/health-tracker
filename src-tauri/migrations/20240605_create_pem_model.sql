CREATE TABLE IF NOT EXISTS pem_calibration (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    param_name TEXT NOT NULL UNIQUE,
    param_value REAL NOT NULL,
    description TEXT
);

-- Seed default calibration values from v6 model
INSERT OR IGNORE INTO pem_calibration (param_name, param_value, description) VALUES
    ('debt_persistence', 0.55, 'Higher = recovery debt decays more slowly'),
    ('recovery_credit', 0.14, 'Lower = less recovery credit on low-fatigue days'),
    ('crash_threshold', 4.0, 'Lower = threshold penalty activates sooner'),
    ('threshold_exponent', 1.3, 'Higher = more nonlinear crash penalty'),
    ('fatigue_sensitivity_divisor', 9.0, 'Lower = fatigue amplifies exertion more strongly'),
    ('fatigue_load_penalty', 0.2, 'Extra debt added when fatigue is already elevated'),
    ('active_while_fatigued_penalty', 0.2, 'Extra debt from activity when fatigue >= 6'),
    ('risk_divisor', 2.5, 'Lower = higher final risk score'),
    ('low_risk_band_cutoff', 2.0, 'Upper bound of Low risk band'),
    ('medium_risk_band_cutoff', 4.5, 'Upper bound of Medium risk band'),
    ('energy_factor_low', 0.7, 'Numeric weight for Low energy-cost activities'),
    ('energy_factor_medium', 1.0, 'Numeric weight for Medium energy-cost activities'),
    ('energy_factor_high', 2.0, 'Numeric weight for High energy-cost activities'),
    ('weight_physical_active', 1.0, 'Category weight for Physical/Active hours'),
    ('weight_domestic', 0.5, 'Category weight for Domestic hours'),
    ('weight_cognitive_active', 1.4, 'Category weight for Cognitive/Active hours'),
    ('weight_hobby_creative', 0.5, 'Category weight for Hobby/Creative hours'),
    ('weight_social', 0.6, 'Category weight for Social hours (sensory/social load)'),
    ('weight_screen_sedentary', 0.3, 'Category weight for Screen/Sedentary hours (sensory load)'),
    ('sleep_weight', 0.2, 'Weight applied to poor-sleep deviation in PEM risk'),
    ('sleep_baseline', 8.0, 'Reference good-sleep hours'),
    ('fatigue_map_slope', 0.466, 'Converts Risk to next-day fatigue'),
    ('fatigue_map_intercept', 4.004, 'Intercept of Risk->fatigue mapping'),
    ('prediction_range', 1.6, 'Typical error band (residual SD)'),
    ('steps_normaliser', 2000.0, 'Steps that map to 1.0 of step-load'),
    ('steps_weight', 0.4, 'Share of physical load from steps'),
    ('calories_normaliser', 500.0, 'Calories that map to 1.0 of calorie-load'),
    ('calories_weight', 0.6, 'Share of physical load from calories');

CREATE TABLE IF NOT EXISTS pem_predictions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL UNIQUE,             -- YYYY-MM-DD
    physical_load REAL,
    cognitive_load REAL,
    sensory_social_load REAL,
    three_day_weighted_load REAL,
    recovery_debt REAL,
    threshold_penalty REAL,
    predicted_pem_risk REAL,
    risk_band TEXT,                            -- Low, Medium, High
    crash_flag INTEGER DEFAULT 0,
    predicted_next_day_fatigue REAL,
    predicted_low REAL,
    predicted_high REAL,
    computed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_pem_predictions_date ON pem_predictions(log_date);