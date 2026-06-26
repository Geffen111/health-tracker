# Health Tracker — Plan & Architecture

A Tauri v2 + SvelteKit 5 Windows desktop app that replaces the CFS/ME Fatigue Log
spreadsheet (`Fatigue_Log_V6.xlsx`). Same stack and conventions as the Family Finance
app (`../family-finances`), which is the reference implementation.

> This file is the source of truth for the project plan. The original multi-phase plan
> was lost once when a build session's context was interrupted — keep this file updated
> after each phase so it can never be lost again.

## Architecture & data flow

```
Samsung Health App (Android)
  → Health Sync app (Google Play)
  → Google Drive CSV export (steps, HR, sleep)
  → Health Tracker auto-import scheduler
      ↓
Daily entries (steps, HR, sleep stages) auto-populated
  + Manual entry (fatigue, headache, BP, meds, work hours, activities)
      ↓
DailySummary (category hours from ActivityLog)
      ↓
PEM Model → risk predictions → Dashboard
```

- **DB:** `%OneDrive%\Apps\HealthTracker\health.db` (OneDrive sync across devices); falls
  back to `dirs::data_dir()/health-tracker/`. See `src-tauri/src/db/mod.rs`.
- **Migrations** are embedded via `sqlx::migrate!("./migrations")` — never read from disk
  at runtime.
- **Source spreadsheet:** `%OneDrive%\Health\Fatigue_Log_V6.xlsx` (V6 is current, not V4/V5).

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Scaffold — Tauri v2 + SvelteKit 5 + sqlx + SQLite, dark sidebar, 6 migrations | ✅ Done, builds clean |
| 2 | XLSX import engine (calamine, serial-date conversion, idempotent upsert by `log_date`); `/import` page | ✅ Done + **run & verified 2026-06-26** |
| 3 | Pages: daily, sleep, activity, cardio, work, medication | ✅ Done |
| 4 | PEM model rewrite to match spreadsheet formulas exactly (33 calibration params) | ✅ Done |
| 5 | Token-based CSS overhaul (adopt Family Finance design system) | ⬜ TODO |
| 6 | Settings page — Google Drive CSV path, calibration viewer, data export (CSV/JSON) | ⬜ TODO |
| 7 | Google Drive CSV auto-import (poll Samsung Health CSV, parse steps/HR/sleep) | ⬜ TODO |
| 8 | Dose-logging UI (frontend for `get_doses_for_date` / `upsert_dose`) | ⬜ TODO |
| 9 | Chart.js integration (replace CSS trend bars with interactive charts) | ⬜ TODO |

(An OpenRouter AI integration like Family Finance's was proposed; `reqwest` is already a
dependency but no AI command file exists yet.)

## Import status (verified 2026-06-26)

Source `Copy of Fatigue_Log_V5 26 June.xlsx` (misnamed — actually V6) imported into
`health.db`. Verified row counts: **daily_logs 126** (2026-02-21 → 06-26), **activity_log
425**, **blood_pressure 215**, **medication_doses 726** (6 meds auto-created), pem_calibration 57.

Importer bugs fixed during this import (`commands/import_xlsx.rs`):
- **Dates** arrive as `Data::DateTime`, not `Float` — `get_date` now reads the serial from either.
- **Med/BP times** are time-formatted cells (DateTime/float), unreadable as text — added
  `cell_time_string` (Excel day-fraction → "HH:MM"); used for med + BP times.
- **Med doses** now import whenever a dose value is present (time optional) — previously the
  guard required a parseable time string, so 0 doses imported.
- **Activity re-import is idempotent** — `DELETE FROM activity_log` first (plain INSERT, no
  unique key); daily/BP/calibration already upsert.
- **Empty trailing rows** in a sheet's used range are skipped via `date_cell_empty` (was
  producing hundreds of bogus "errors").

## Feature backlog (user notes, 2026-06-26)

Grouped by what they touch. IDs match the user's note numbers.

### A. Data model / backend — ✅ DONE 2026-06-26 (migrations 08–10, commands wired)

Also fixed a data-loss bug found here: `upsert_daily_log` overwrote *every* column, so a
partial save from one page (e.g. Work hours) wiped another page's fields (sleep/steps/meds)
for that day. The `ON CONFLICT` now `COALESCE`s — a null incoming field leaves the stored
value as-is, so each page safely contributes its slice of the day. (The importer has its own
upsert and is unaffected.)

- **A1 — Medication history & lifecycle (#4).** New `medication_history` table recording
  started/ceased events with dates + free text ("Ceased medication X on …"). When a med's
  `active` flag (or default dose) changes, append a history row and surface an in-app banner
  noting the change. History entries must be editable.
- **A2 — Additional/ad-hoc meds migration (#3).** Migrate the `daily_logs.add_meds` free-text
  column into proper `medications` + `medication_doses` rows (e.g. migraine, hay-fever meds).
  Support adding new meds beyond the seeded 6, each with editable default dose.
- **A3 — Heart-rate fields (#6).** Add `hr_min` / `hr_max` columns to `daily_logs` (keep
  `ave_resting_hr`, `ave_hr`). These will be synced from the watch later (Phase 7).
- **A4 — Watch-calibration log (#5).** New table for watch-calibration events (date + time);
  used to flag the ~30-day recalibration.
- **A5 — Merge work notes into daily notes (#1).** Work page should not have its own notes;
  consolidate into the single `daily_logs.notes` ("Other Daily Notes"). No schema change
  (work notes already share `notes`).

### B. Interaction / UX (best defined with Claude Design — see handover #10)
- **B1 — Medication management UI (#3).** "Current Medications" list (like the spreadsheet's),
  each with an **"Add dose record"** button that defaults to the usual dose but is editable.
- **B2 — Cardio page rework (#5, #6).** Variable number of BP readings via an **"Add reading"**
  control (don't assume 3) + daily average + individual readings. Replace the current
  resting/avg HR blocks with resting / average / **min** / **max** HR. Add a
  **"Log watch calibration"** button (captures time).
- **B3 — Day navigation (#7).** Back/forward day arrows on every daily-log screen.
- **B4 — Sleep trend selector (#8).** The 30-day trend switches between data points
  (Actual sleep / REM / Deep / Awake / etc.) via a row of buttons.
- **B5 — Dashboard comparison viz (#9).** Adjustable visualisations comparing chosen data
  points (Chart.js, Phase 9).
- **B6 — Import tucked into Settings (#2).** Move the `/import` page into Settings as a
  collapsible section (rarely needed after setup).

### C. Cross-cutting
- **C1 — Date display format (#11).** Show dates as **DD/MM/YY** everywhere; keep ISO
  `YYYY-MM-DD` in the DB. Add a shared formatter.
- **C2 — Claude Design handover (#10).** Prepare a handover bundle for claude.ai/design to
  develop the visual style, once the data model (section A) is settled so Design knows the
  real entities/screens. Keep the bundle out of this git root (see
  Family Finance design-handoff convention).

## PEM model formulas (Spreadsheet V5 — authoritative)

```
Physical Load (G) =
  Pre-ActivityLog:  (Steps/2000)*0.4 + (Calories/500)*0.6
  Post-ActivityLog: AVERAGE(base, activity_physical)   [÷3]

Cognitive Load (H) =
  Pre:  (OfficeHrs*1.2 + WFHHrs*0.9)/3
  Post: AVERAGE(base, activity_cognitive)              [÷3]

Sensory/Social Load (I) =
  Pre: 0
  Post: activity_sensory_social / 3

Sensitivity (J) = 1 + Fatigue/9

3-Day Weighted (K) = (G+H+I) * 0.55   [single day, no carry-forward]

Recovery Debt (L) = MAX(0, K + fatigue_penalty - recovery_credit + active_penalty)
  fatigue_penalty = MAX(0, Fatigue-5) * 0.2
  recovery_credit = (10-Fatigue) * 0.14   [more credit when fatigue low]
  active_penalty  = IF(Fatigue>=6, HighEnergyHours * 0.1, 0)

Threshold Penalty (M) = IF(L > 4.0, (L-4.0)^1.3, 0)

Predicted PEM Risk (N) = MIN(10, (K*J + L*0.9 + M*1.1 + sleep_penalty) / 2.5)
  sleep_penalty = MAX(0, 8 - SleepAvg) * 0.2

Risk Band = N >= 4.5 → High, N >= 2 → Medium, else Low
Crash Flag = M > 0   [threshold penalty exists; not transition detection]
Next-Day Fatigue = 0.466 * Risk + 4.004
```

## Calibration parameters (33, stored in `pem_calibration`)

Key values that differ from naive defaults:
- `debt_persistence=0.55`, `recovery_credit=0.14`, `crash_threshold=4.0`, `threshold_exponent=1.3`
- `fatigue_sensitivity_divisor=9.0`, `fatigue_load_penalty=0.2`, `risk_divisor=2.5`
- `low_risk_band_cutoff=2.0`, `medium_risk_band_cutoff=4.5`
- `energy_factor_low=0.7`, `medium=1.0`, `high=2.0`
- `weight_physical_active=1.0`, `domestic=0.5`, `cognitive=1.4`, `hobby=0.5`, `social=0.6`, `screen=0.3`
- `sleep_weight=0.2`, `sleep_baseline=8.0`
- `fatigue_map_slope=0.466`, `intercept=4.004`, `prediction_range=1.6`
- `steps_normaliser=2000`, `steps_weight=0.4`, `calories_normaliser=500`, `calories_weight=0.6`
- `activity_log_start_date=46150` (Excel serial)
- `high_energy_fatigued_multiplier=0.1`

## Family Finance patterns to follow

1. CSS token system — `:root` + `:global(.dark)` with `--bg-primary`, `--accent`, `--radius-card`, etc.
2. Settings stored in a `settings.json` in the data dir — `save_api_key`/`get_api_key` pattern.
3. Component library — Toast, searchable combobox, theme via Svelte stores.
4. Butter (headings) + Figtree (body) font pairing.
5. CSV import with header auto-detection and upsert dedup.

## Verifying changes (Windows; toolchain not on PATH in non-interactive shells)

```bash
# Frontend type-check (keep at 0 errors):
CI=true npm_config_verify_deps_before_run=false pnpm check
# Rust backend:
cd src-tauri && RUSTFLAGS="" cargo check
# Run the app with hot reload:
pnpm tauri dev
```
