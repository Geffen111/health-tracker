-- Categories (one per item) and flags (any number per item) for the food list, so the
-- Food page and weekly summary can look at "days with gluten" rather than one item at a
-- time. Both lists are the person's to edit; the AI only picks from them.
CREATE TABLE IF NOT EXISTS food_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    sort INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS food_flags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    sort INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS food_item_flags (
    food_id INTEGER NOT NULL REFERENCES foods(id) ON DELETE CASCADE,
    flag_id INTEGER NOT NULL REFERENCES food_flags(id) ON DELETE CASCADE,
    PRIMARY KEY (food_id, flag_id)
);

ALTER TABLE foods ADD COLUMN category_id INTEGER REFERENCES food_categories(id);
-- Who set the category and flags: NULL = not yet tagged, 'ai' = tagged by the model,
-- 'user' = set or confirmed by hand. The model only ever tags NULL rows, so an edit is
-- never overwritten.
ALTER TABLE foods ADD COLUMN tag_source TEXT;

INSERT OR IGNORE INTO food_categories (name, sort) VALUES
    ('Fruit', 1), ('Vegetables', 2), ('Grains & bread', 3), ('Dairy', 4),
    ('Meat & fish', 5), ('Eggs', 6), ('Legumes', 7), ('Nuts & seeds', 8),
    ('Meals & dishes', 9), ('Snacks & sweets', 10), ('Sauces & spreads', 11),
    ('Hot drinks', 12), ('Cold drinks', 13), ('Alcohol', 14);

INSERT OR IGNORE INTO food_flags (name, sort) VALUES
    ('Gluten', 1), ('Dairy', 2), ('High-histamine', 3), ('Caffeine', 4), ('Alcohol', 5),
    ('Added sugar', 6), ('High-FODMAP', 7), ('Protein', 8), ('High-fiber', 9), ('High-carb', 10);
