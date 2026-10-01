-- Weekly AI summaries: one row per Monday-to-Sunday week.
--   metrics_json   — the deterministic figures (scorecard, meds, food, labs…) computed in Rust
--   narrative_json — the model's write-up of those figures
-- `seen` drives the Dashboard banner; it lives in the DB so it follows the user across PCs.
CREATE TABLE IF NOT EXISTS weekly_summaries (
    week_start     TEXT PRIMARY KEY,            -- the Monday, YYYY-MM-DD
    week_end       TEXT NOT NULL,               -- the Sunday, YYYY-MM-DD
    generated_at   TEXT NOT NULL,
    model          TEXT,
    metrics_json   TEXT NOT NULL,
    narrative_json TEXT NOT NULL,
    seen           INTEGER NOT NULL DEFAULT 0
);
