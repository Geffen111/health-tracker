-- Monthly daily-average steps & active calories, for the dashboard's monthly
-- steps/calories card (recreated from the spreadsheet's "Steps Daily Average" and
-- "Calories Daily Average" tables + charts).
--
-- Each row is one calendar month's DAILY AVERAGE (not the monthly total). Historical
-- months pre-date the app's own daily logging (which begins 2026-02), so they are
-- seeded here as manual history transcribed from the spreadsheet. From 2026-06 onward
-- the app computes these averages from daily_logs, so those months are deliberately
-- NOT seeded (a seeded row always wins over the computed value). Any month can later
-- be overridden manually from the dashboard table, which writes back to this table.
CREATE TABLE IF NOT EXISTS monthly_activity (
    year         INTEGER NOT NULL,   -- e.g. 2025
    month        INTEGER NOT NULL,   -- 1..12
    steps_avg    REAL,               -- daily-average steps for the month
    calories_avg REAL,               -- daily-average active calories for the month
    PRIMARY KEY (year, month)
);

INSERT OR IGNORE INTO monthly_activity (year, month, steps_avg, calories_avg) VALUES
    (2024, 1, 11869, 586), (2024, 2, 10735, 555), (2024, 3, 10335, 578),
    (2024, 4, 10116, 548), (2024, 5, 10733, 641), (2024, 6, 10135, 524),
    (2024, 7,  9242, 450), (2024, 8,  9526, 454), (2024, 9, 10292, 482),
    (2024, 10, 11347, 644), (2024, 11, 12498, 702), (2024, 12, 13497, 761),
    (2025, 1, 11617, 695), (2025, 2, 11858, 727), (2025, 3, 12916, 752),
    (2025, 4, 12383, 742), (2025, 5, 12830, 728), (2025, 6, 11925, 623),
    (2025, 7,  9569, 536), (2025, 8, 10756, 561), (2025, 9,  9817, 501),
    (2025, 10, 10177, 522), (2025, 11, 10921, 547), (2025, 12, 11728, 567),
    (2026, 1, 10231, 500), (2026, 2,  8734, 435), (2026, 3,  8367, 497),
    (2026, 4,  7497, 382), (2026, 5,  7314, 350);
