CREATE TABLE IF NOT EXISTS activity_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,                  -- Physical/Active, Screen/Sedentary, etc.
    energy_weight REAL                          -- used in PEM model
);

CREATE TABLE IF NOT EXISTS activity_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,                  -- Walking, Gaming, Cooking, etc.
    category_id INTEGER NOT NULL REFERENCES activity_categories(id),
    default_energy_cost TEXT DEFAULT 'Medium'   -- Low, Medium, High
);

CREATE TABLE IF NOT EXISTS activity_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_date TEXT NOT NULL,                     -- YYYY-MM-DD
    activity_type_id INTEGER NOT NULL REFERENCES activity_types(id),
    duration_hours REAL NOT NULL,
    energy_cost TEXT,                           -- override: Low, Medium, High
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_activity_log_date ON activity_log(log_date);

-- Seed default activity categories
INSERT OR IGNORE INTO activity_categories (name, energy_weight) VALUES
    ('Physical / Active', 1.0),
    ('Domestic', 0.5),
    ('Cognitive / Active', 1.4),
    ('Hobby / Creative', 0.5),
    ('Social', 0.6),
    ('Screen / Sedentary', 0.3);

-- Seed default activity types (matching your current spreadsheet)
INSERT OR IGNORE INTO activity_types (name, category_id, default_energy_cost) VALUES
    ('Walking', (SELECT id FROM activity_categories WHERE name='Physical / Active'), 'High'),
    ('Yard Work', (SELECT id FROM activity_categories WHERE name='Physical / Active'), 'High'),
    ('Shopping', (SELECT id FROM activity_categories WHERE name='Domestic'), 'High'),
    ('Cleaning', (SELECT id FROM activity_categories WHERE name='Domestic'), 'High'),
    ('Cooking', (SELECT id FROM activity_categories WHERE name='Domestic'), 'Medium'),
    ('Ironing', (SELECT id FROM activity_categories WHERE name='Domestic'), 'Medium'),
    ('Admin - Emails/Bills', (SELECT id FROM activity_categories WHERE name='Domestic'), 'Low'),
    ('Appointments', (SELECT id FROM activity_categories WHERE name='Domestic'), 'Medium'),
    ('Driving', (SELECT id FROM activity_categories WHERE name='Domestic'), 'Medium'),
    ('Work at Office', (SELECT id FROM activity_categories WHERE name='Cognitive / Active'), 'High'),
    ('Work from Home', (SELECT id FROM activity_categories WHERE name='Cognitive / Active'), 'Medium'),
    ('Work Stress', (SELECT id FROM activity_categories WHERE name='Cognitive / Active'), 'High'),
    ('Anxiety', (SELECT id FROM activity_categories WHERE name='Cognitive / Active'), 'High'),
    ('Family Stress', (SELECT id FROM activity_categories WHERE name='Cognitive / Active'), 'High'),
    ('Guitar / Music', (SELECT id FROM activity_categories WHERE name='Hobby / Creative'), 'Low'),
    ('Model Ship', (SELECT id FROM activity_categories WHERE name='Hobby / Creative'), 'Low'),
    ('Recording', (SELECT id FROM activity_categories WHERE name='Hobby / Creative'), 'Medium'),
    ('Reading', (SELECT id FROM activity_categories WHERE name='Hobby / Creative'), 'Low'),
    ('Gaming', (SELECT id FROM activity_categories WHERE name='Screen / Sedentary'), 'Low'),
    ('Phone', (SELECT id FROM activity_categories WHERE name='Screen / Sedentary'), 'Low'),
    ('Computer Use', (SELECT id FROM activity_categories WHERE name='Screen / Sedentary'), 'Low'),
    ('Watching TV', (SELECT id FROM activity_categories WHERE name='Screen / Sedentary'), 'Low'),
    ('Smart Home', (SELECT id FROM activity_categories WHERE name='Screen / Sedentary'), 'Medium'),
    ('Visiting Neighbours', (SELECT id FROM activity_categories WHERE name='Social'), 'High'),
    ('Visitors Over', (SELECT id FROM activity_categories WHERE name='Social'), 'High'),
    ('Cafe', (SELECT id FROM activity_categories WHERE name='Social'), 'High'),
    ('Playing with Augie', (SELECT id FROM activity_categories WHERE name='Social'), 'Medium'),
    ('Augie''s Friends Over', (SELECT id FROM activity_categories WHERE name='Social'), 'Medium'),
    ('Socialising out of the house', (SELECT id FROM activity_categories WHERE name='Social'), 'High');