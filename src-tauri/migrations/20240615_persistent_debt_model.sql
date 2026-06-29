-- Switch the fatigue prediction to a cross-validated, persistent-debt model.
--
-- The previous mapping predicted next-day fatigue from the composite
-- predicted_pem_risk (CV R^2 ~0.13). Analysis on the activity era (2026-06-29)
-- showed two things:
--   * Recovery debt was being recomputed from scratch each day with no carryover,
--     even though a debt_persistence param (0.55) existed but was never read. PEM
--     is cumulative, so giving debt real day-to-day memory lifts its correlation
--     with next-day fatigue from R^2 0.20 to 0.27.
--   * Of every candidate feature (today's fatigue, sleep deficit, average HR,
--     steps, calories), persistent recovery debt ALONE was the best predictor by
--     leave-one-out cross-validation (CV R^2 0.24, RMSE 1.60). Adding the others
--     reduced out-of-sample accuracy (52 obs can't support more features).
--
-- New model:  predicted_next_day_fatigue = intercept + slope * recovery_debt
--             where recovery_debt carries over (decayed by debt_persistence).
UPDATE pem_calibration SET param_value = 1.60 WHERE param_name = 'prediction_range';

-- Steps and calories carry essentially the same signal (r 0.31 vs 0.31); steps has
-- full coverage, calories does not, so the model uses steps only. Calories are still
-- captured (CSV import + manual entry) for reference and can be re-weighted later.
UPDATE pem_calibration SET param_value = 1.0 WHERE param_name = 'steps_weight';
UPDATE pem_calibration SET param_value = 0.0 WHERE param_name = 'calories_weight';

-- New params for the persistent-debt model.
INSERT OR IGNORE INTO pem_calibration (param_name, param_value, description) VALUES
    ('steps_load_cap', 4.0, 'Max step-load (steps/normaliser) before saturating; was a hardcoded 2.0'),
    ('fatigue_from_debt_intercept', 3.5696, 'Next-day fatigue when recovery debt is 0 (OLS on activity era)'),
    ('fatigue_from_debt_slope', 0.5316, 'Next-day fatigue added per unit of recovery debt (OLS on activity era)');

-- The old risk->fatigue coefficients are no longer used by the model.
DELETE FROM pem_calibration WHERE param_name IN ('fatigue_map_slope', 'fatigue_map_intercept');
