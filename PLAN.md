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
Pacing view → activity & fatigue history → Dashboard
```

- **DB:** `%OneDrive%\Apps\HealthTracker\health.db` (OneDrive sync across devices); falls
  back to `dirs::data_dir()/health-tracker/`. See `src-tauri/src/db/mod.rs`.
- **Migrations** are embedded via `sqlx::migrate!("./migrations")` — never read from disk
  at runtime.
- **Source spreadsheet:** `%OneDrive%\Health\Fatigue_Log_V6.xlsx` (V6 is current, not V4/V5).
- **Health Sync CSVs (Phase 7):** four folders under a Drive root (default `G:\My Drive`):
  `Health Sync Steps` (`Date,Time,Steps` → SUM/day), `Health Sync Heart rate`
  (`Date,Time,Heart rate,Source` → mean/min/max → `ave_hr`/`hr_min`/`hr_max`),
  `Health Sync Energy burned` (`Date,Time,Active/Resting/Total calories` → active → `activity_calories`),
  `Health Sync Sleep` (`Date,Time,Duration in seconds,Sleep stage` → asleep/rem/deep/awake/on-pillow
  hours, attributed to the **wake day**). COALESCE-upsert; resting HR left untouched. Reprocess by
  file mtime > last sync. See `commands/csv_import.rs`.

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Scaffold — Tauri v2 + SvelteKit 5 + sqlx + SQLite, dark sidebar, 6 migrations | ✅ Done, builds clean |
| 2 | XLSX import engine (calamine, serial-date conversion, idempotent upsert by `log_date`); `/import` page | ✅ Done + **run & verified 2026-06-26** |
| 3 | Pages: daily, sleep, activity, cardio, work, medication | ✅ Done |
| 4 | PEM model rewrite to match spreadsheet formulas exactly (33 calibration params) | ✅ Done |
| 5 | Token-based CSS overhaul (adopt Meridian design system) | ✅ Done — full Meridian theme implemented 2026-06-27 |
| 6 | Settings page — Google Drive CSV path, calibration viewer, data export (CSV/JSON), collapsible import | ✅ Done 2026-06-27 |
| 7 | Google Drive CSV auto-import (Samsung Health via Health Sync — steps/HR/sleep/energy) | ✅ Done 2026-06-27 — `commands/csv_import.rs`, on-launch + Sync now |
| 8 | Dose-logging UI (frontend for `get_doses_for_date` / `upsert_dose`) | ⬜ TODO |
| 9 | Chart.js integration (replace static SVG trends with interactive charts) | ✅ Done 2026-06-27 — Chart.svelte wrapper, Dashboard compare-signals dual-line chart, Sleep 30-day selectable-metric chart, Dashboard sleep sparkline |

| 10 | AI Ask + Insights (OpenRouter, like Family Finance) | ✅ Done 2026-06-27 — `/ask` route, NL→SQL `ask_question`, `get_insights`/`refresh_insights`, API key in Settings |
| 11 | Entry restructure — day carried in `?date=`; Daily Log rebuilt as two dated columns (previous day + selected day) absorbing resting HR and BP; Cardio becomes analysis-only | ✅ Done 2026-07-28 |
| 12 | Retire the PEM risk model; `/pem-model` → `/pacing` (descriptive activity & fatigue history) | ✅ Done 2026-08-19 — see "Retired: the PEM risk model" below |
| 13 | Activity & category management — categories/types editable in-app, `load_group` made explicit | ✅ Done 2026-08-19 |

**AI integration (Phase 10):** OpenRouter (`deepseek/deepseek-v4-flash`), mirroring Family
Finance. `commands/ai.rs` (shared client), `commands/ask.rs` (hybrid text-to-SQL — schema sent,
not data; SELECT-only validation), `commands/insights.rs` (aggregates → model → cached in
`ai_insights`, migration 20240611), `commands/settings.rs` (API key in `<data_dir>/settings.json`).
Key is entered on the Settings page. Privacy: only the schema + small aggregated results are sent
to the model, never raw row-level health data.

**2026-06-27 (Claude Code), smaller wins after the OpenCode handover:** data export is now
genuinely wired (`commands/export.rs` → `export_csv`/`export_json`, writing to
`<data_dir>/exports/`); the `watch_calibration` table/commands are now surfaced as
**blood-pressure-monitor calibration** (every ~30 days) on the Cardio page with manual
date/time entry + a "Last calibrated: …" note — the misleading "Watch synced" sidebar block
was removed; the orphaned `/import` route was deleted (import lives in Settings).

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

### B. Interaction / UX — ✅ DONE 2026-06-27 (Meridian design implemented)
- **B1 — Medication management UI (#3).** Current meds list (Regular + PRN) with "Add dose record"
  editable inline form, cease/restart toggle with history banner.
- **B2 — Cardio page rework (#5, #6).** Variable BP readings via "Add reading" control with delete,
  daily average, HR block with resting/average/min/max, "Log watch calibration" button with
  days-since display and overdue nudge.
- **B3 — Day navigation (#7).** Back/forward day arrows on Daily Log, Sleep, Activity, Cardio, Work.
- **B4 — Sleep trend selector (#8).** 30-day trend chart with selectable metric
  (Asleep/REM/Deep/Awake/Rating) via segmented toggle.
- **B5 — Dashboard comparison viz (#9).** Compare signals card with metric pickers and
  range selector (Chart.js integration TBD).
- **B6 — Import tucked into Settings (#2).** Collapsible "Import data" section in Settings with
  path input and idempotent-import notes.

### C. Cross-cutting
- **C1 — Date display format (#11).** Show dates as **DD/MM/YY** everywhere; keep ISO
  `YYYY-MM-DD` in the DB. Add a shared formatter.
- **C2 — Claude Design handover (#10).** ✅ Brief prepared 2026-06-26 at
  `../health-tracker-design-handoff/README.md` (sibling folder, out of this git root per the
  Family Finance design-handoff convention). Greenfield design brief: app context, ME/CFS
  design principles, per-screen requirements (incl. all the UX notes), data dictionary, PEM
  context. Awaiting Claude Design comps (`.dc.html` + screenshots + ICONS.md) to come back,
  then section B implementation.

## Retired: the PEM risk model (2026-08-19)

The `predicted_pem_risk` score and the `predicted_next_day_fatigue` estimate were removed,
along with `pem_predictions`, `pem_calibration` and the 33 calibration params. Migration
`20240622_retire_pem_predictions.sql` carries the full rationale; the measurements, taken
over the whole activity era (2026-05-08 → 08-18, 102 consecutive-day pairs — double the 52
the June refit used):

| Predicting next-day fatigue | RMSE |
|---|---|
| Shipped model (in-sample) | 1.88 |
| "Always guess the mean (5.9)" | 1.90 |
| "Tomorrow = today" | 2.05 |

- Across 88 predictions the model emitted **Medium 66x, High 22x, Low 0x** against an actual
  41 High / 40 Medium / 7 Low. It never once predicted a good day; output spanned 3.6–8.1
  against an actual 2.5–9.5.
- Correlation with next-day fatigue: today's fatigue +0.371, calories +0.098, work hours
  +0.078, steps +0.077, sleep −0.062, high-energy hours −0.022, activity hours −0.097.
  Every exertion input is noise.
- **Reverse causation dominates.** Steps vs *same-day* fatigue is −0.222 and calories −0.270,
  both stronger than any forward effect: low exertion follows a bad day rather than preceding
  one, so a load→risk formula fights the sign.
- `recovery_debt` was computed from the same day's `fatigue_rating`, so the 0.457 same-day
  correlation that justified the June refit was largely self-reference.
- Fatigue autocorrelation: +0.37 at lag 1, +0.19 at lag 2, −0.03 at lag 3. No recoverable
  delayed-PEM structure at daily resolution.
- Best honest model by leave-one-out CV is today's fatigue alone (CV R² 0.104, RMSE 1.73).
  Too weak to display as a number, so nothing predicts forward any more.

This is a null result about **daily aggregates**, not about pacing. The likely reasons the
signal is absent: pacing already compresses the exertion range, daily totals are too coarse
for what triggers PEM, and reverse causation swamps the forward effect. A finer-grained
record (an explicit "did I overdo it?" flag, or intensity peaks rather than daily sums) is
the data-collection change that could carry signal — not another model on the same inputs.

### What replaced it

`commands/pacing.rs` — `get_daily_loads` and `get_activity_history`, both purely descriptive
and computed on demand (nothing is stored). Load is
`duration x activity_categories.energy_weight x energy-cost factor` (Low 0.7 / Medium 1.0 /
High 2.0), kept in step with `src/lib/load.ts` so a day reads the same everywhere.

The `/pem-model` route became `/pacing`:
- Headline tiles — 7-day fatigue with week-on-week delta, 7-day activity hours, days since a
  bad day (fatigue ≥ 8), bad days in the last 30.
- **Activity over time** — stacked bars of hours per week or month, grouped by category or by
  individual activity, with average fatigue overlaid on a right axis. The activity picker
  ranks by *load*, not hours, so demanding work surfaces above screen time; chips isolate a
  chosen few (e.g. Yard Work + Walking).
- **Fatigue over time** — daily rating plus a 7-day rolling average.
- **Signal check** — the correlations above, recomputed live from the log each time the page
  opens, so the honesty survives as the data grows.
- **Recent days** — 14 days of fatigue, hours, high-energy hours, steps and longest activity.

Dashboard: the risk gauge now shows today's *logged* fatigue; recovery debt became
"Activity · last 7 days"; the "Risk · last 7 days" dots (which were **hardcoded**, not real
data) became actual banded fatigue dots; `crash_count_30d` became `bad_days_30d`.

## Activity categories & types (2026-08-19)

Categories and activity types were **seed-only** until now: the migrations created them and
nothing in the app could add, rename or retune one. Both are now managed from a collapsible
**Manage activities & categories** panel at the bottom of the Activity page.

- Categories: name, `energy_weight`, and `load_group`.
- Activity types: name, category, `default_energy_cost`, plus a count of days logged.
- Commands live in `commands/activity.rs`: `create/update/delete_activity_category`,
  `create/update/delete_activity_type`, `list_activity_types_with_usage`.

**`load_group` is new** (migration `20240623_activity_category_load_group.sql`). Which load
bucket a category fed used to be inferred from substrings of its *name*, duplicated in
`BUCKET_EXPR` (`commands/pacing.rs`) and `computeDayLoad` (`src/lib/load.ts`). That made the
mapping invisible, fragile (renaming "Domestic" would silently move it to sensory) and
unsettable for any category the user added. It is now a stored, editable column with values
`physical` | `cognitive` | `sensory`. The migration backfills exactly what the name rules
produced — verified identical across all 104 days of history.

Guards, so the manage panel can't corrupt the log:
- A category with activity types still in it cannot be deleted.
- An activity type logged on any day cannot be deleted (the button is disabled and says why).
- Names are UNIQUE; the duplicate error is surfaced in the panel rather than failing silently.
- Editing a type's energy cost changes the DEFAULT for future entries only — `activity_log`
  rows keep the cost stored on them, so history doesn't shift underfoot. Editing a category's
  `energy_weight` or `load_group` *does* apply to history, since load is computed on demand.

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
