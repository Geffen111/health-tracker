CREATE TABLE IF NOT EXISTS ai_insights (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    insight_type TEXT NOT NULL,
    insight_data TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    generated_at TEXT NOT NULL DEFAULT (datetime('now')),
    data_hash TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_insights_period ON ai_insights(period_start, period_end);
