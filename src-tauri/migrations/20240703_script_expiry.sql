-- When the current script runs out (YYYY-MM-DD); its repeats can't be filled after it.
-- NULL = not tracked. Warned about on the Medication page and Dashboard ($lib/medSupply.ts).
ALTER TABLE medications ADD COLUMN script_expiry TEXT;
