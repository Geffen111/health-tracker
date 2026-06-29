-- Recalibrate the Risk -> next-day-fatigue mapping, fit on the activity-tracking era.
--
-- Activity logging began 2026-05-08. Before that the model has no activity data and
-- runs a blinder fallback formula; pooling both eras gave a weak fit (R^2 0.09) and
-- the old spreadsheet coefficients (slope 0.466, intercept 4.004) sat ~0.8 below the
-- actual mean fatigue, collapsing most days toward ~4.4.
--
-- Refitting next_day_fatigue ~ predicted_pem_risk on the 52 post-8-May observations
-- (2026-06-29) gives a markedly stronger fit (R^2 0.15 vs 0.02 pre-era) and these
-- coefficients. prediction_range is set to the post-era residual SD (~1.70) so the
-- confidence band reflects real error in the era we actually trust.
--
-- activity_log_start_date is bumped 46150 -> 46151 so the model boundary lands on
-- 2026-05-08 (the true first day of activity tracking) rather than 2026-05-07.
--
-- INSERT OR IGNORE in the original seed cannot change existing rows, so we UPDATE.
UPDATE pem_calibration SET param_value = 0.6980 WHERE param_name = 'fatigue_map_slope';
UPDATE pem_calibration SET param_value = 4.4608 WHERE param_name = 'fatigue_map_intercept';
UPDATE pem_calibration SET param_value = 1.70   WHERE param_name = 'prediction_range';
UPDATE pem_calibration SET param_value = 46151  WHERE param_name = 'activity_log_start_date';
