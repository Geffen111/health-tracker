-- Structured pathology/lab results extracted from the Obsidian Health Records
-- vault. Source of truth stays in the markdown notes; this table is a derived,
-- re-buildable cache so results can be charted over time and queried. Each row
-- keeps its raw value/reference text and the source note for verification.
CREATE TABLE IF NOT EXISTS lab_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    test_name TEXT NOT NULL,        -- canonical analyte, e.g. 'CRP', 'Ferritin', 'Haemoglobin'
    category TEXT,                  -- panel/group, e.g. 'FBE', 'Lipids', 'Iron Studies', 'Serology'
    result_date TEXT NOT NULL,      -- ISO 'YYYY-MM-DD' (first-of-month if only month/year known)
    value_num REAL,                 -- numeric value when parseable, else NULL (qualitative)
    value_text TEXT,                -- raw value as written, e.g. '<3', 'Not detected', '84'
    unit TEXT,
    ref_low REAL,                   -- numeric lower reference bound when known
    ref_high REAL,                  -- numeric upper reference bound when known
    ref_text TEXT,                  -- raw reference, e.g. '<4', '30-500', '>50'
    flag TEXT,                      -- 'HIGH' | 'LOW' | abnormal marker, else NULL/empty
    source_note TEXT NOT NULL,      -- vault rel_path the value was extracted from
    extracted_at TEXT NOT NULL,     -- ISO datetime of the extraction run
    UNIQUE(test_name, result_date, source_note)
);
CREATE INDEX IF NOT EXISTS idx_lab_results_test ON lab_results(test_name);
CREATE INDEX IF NOT EXISTS idx_lab_results_date ON lab_results(result_date);
