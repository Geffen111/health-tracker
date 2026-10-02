# Health Tracker — project guide

Local-first Tauri v2 + SvelteKit 5 desktop app replacing a CFS/ME Fatigue Log spreadsheet.
Built on the same stack and conventions as the **Family Finance** app (`../family-finances`)
— consult that repo's `CLAUDE.md` for patterns not yet documented here. See `PLAN.md` for
the phase roadmap, PEM formulas, and calibration params (source of truth — keep it updated).

## Stack
- **Shell:** Tauri v2 (Rust backend, `src-tauri/`)
- **Frontend:** SvelteKit 5 (runes) + TypeScript + Chart.js (`src/routes/`), static adapter → `build/`
- **DB:** SQLite via `sqlx` (bundled), embedded migrations in `src-tauri/migrations/`
- **Package manager:** pnpm

## Verifying changes (do before committing)
The Windows toolchain isn't on PATH in non-interactive shells, and pnpm 11 tries to
reinstall before each run — disable that:
```bash
# Frontend type-check (keep at 0 errors):
CI=true npm_config_verify_deps_before_run=false pnpm check
# Rust backend (catches type errors without a full link):
cd src-tauri && RUSTFLAGS="" cargo check
# Run with hot reload:
pnpm tauri dev
```

## Database
- Path: `%OneDrive%\Apps\HealthTracker\health.db` on Windows (syncs across devices), else
  `dirs::data_dir()/health-tracker/`. See `src-tauri/src/db/mod.rs`.
- **Migrations are embedded** via `sqlx::migrate!("./migrations")` — never read from disk at
  runtime (`CARGO_MANIFEST_DIR` only exists on the build machine).

## Backend layout (`src-tauri/src/commands/`)
`daily_log`, `medications` (+ dose logging, day note), `blood_pressure`, `activity`, `pacing`
(descriptive activity/exertion history), `dashboard`, `food` (food & drink log, groups),
`exposures` (exposures of note + photo attachments), `weekly` (Monday-to-Sunday AI summaries), `health_notes` (dated appointment/test
notes + `get_timeline_events`, the merged event list for the dashboard Timeline), `import_xlsx` (one-time spreadsheet
import via calamine). Register new commands in `src-tauri/src/lib.rs`.

## Conventions / gotchas
- Dates stored `YYYY-MM-DD`; spreadsheet dates are Excel serials (convert on import).
- XLSX import is idempotent: upsert by `log_date` (`ON CONFLICT DO UPDATE`).
- Svelte 5: `<slot>` is deprecated — use `{@render children()}` (one warning remains in
  `+layout.svelte`).
- Frontend calls the backend via `invoke('<command>', { ... })` from `@tauri-apps/api/core`.
- **Date-scoped pages carry their day in `?date=YYYY-MM-DD`** (`$lib/dateParam.ts`): initialise
  `selectedDate` with `dateFromUrl($page.url)` and call `pushDate()` from the day arrows, so a
  link from one page to another lands on the same day. Link across with `dateHref()`.
  The date between the arrows is a `<DayPicker onpick={goDay}>` (opens a calendar); each page
  has one `goDay(date)` that the arrows and the picker both go through.
- **Activity load has one definition, in two places.** `LOAD_EXPR`/`BUCKET_EXPR` in
  `commands/pacing.rs` and `computeDayLoad` in `src/lib/load.ts` must stay in step:
  hours x `activity_categories.energy_weight` x energy-cost factor (Low 0.7 / Medium 1.0 /
  High 2.0), bucketed by the category's `load_group` column. Never re-derive the bucket from
  the category name — that was the old bug (migration 20240623).
- **Nothing predicts forward.** The PEM risk model and its next-day fatigue estimate were
  removed in migration 20240622 after measuring no better than a constant (see PLAN.md).
  Exertion has no measurable correlation with the next day's fatigue in this log, and low
  activity *follows* a bad day rather than preceding it. Don't reintroduce a risk score,
  crash flag or predicted-fatigue number without new evidence — describe, don't forecast.
- **One editor per column.** The Daily Log owns entry for `daily_logs`; Cardio, Sleep and the
  Dashboard display those fields read-only. Two autosaving editors for one column is how you
  get edits that quietly revert.
- **The CSV import tracks files individually, never by a clock.** `csv_state.json` (machine-local,
  next to `secrets.json`) maps each file to `mtime:size`; a file is re-read when that changes.
  Do not reintroduce a "last sync" watermark: a Health Sync CSV's mtime is when the *phone*
  uploaded it, which is always before Google Drive delivers it to a given PC, so every
  late-arriving file fell behind the line and was skipped forever. The state has to stay
  machine-local for the same reason — one PC having read a file says nothing about another.
- **The launch CSV import gates the page.** `+layout.svelte` renders no route until the
  auto-import finishes: a child's `onMount` runs before the layout's, so a page mounted
  alongside it reads pre-import data (and the Daily Log autosaves those stale totals back).
- **`blood_pressure.source` decides who may touch a row.** `'watch'` = created by the sync, which
  refreshes its numbers but never its note. Anything else (a device name, or NULL) was typed in
  and the sync leaves it entirely alone — a cuff reading minutes after a watch reading is a
  second reading, not a correction. Readings are displayed in `time_taken` order; `reading_num`
  is only an identity key.
- **View settings persist via `$lib/viewState.ts`** (localStorage, key `view:<page>`): a page
  reads `recallView()` at init and writes `rememberView()` from an `$effect`. Chart ranges,
  metric picks, tabs — never data. Validate recalled values (`oneOf`) since options change.
- **File drops are HTML drops.** `dragDropEnabled: false` in `tauri.conf.json` so pages get
  dropped photos as ordinary `drop` events; `+layout.svelte` refuses drops anywhere that isn't
  a drop zone (otherwise the webview navigates to the file). Drop zones must `preventDefault`.
- **Exposure photos live in the DB** (`exposure_attachments.data` BLOB, base64 over IPC).
  `$lib/images.ts` shrinks anything large to a <=2000px JPEG first — the DB syncs via OneDrive.
- **Photo recognition uses a separate vision model** (`settings::vision_model()`, default
  `ai::VISION_MODEL`); the text model can't see images. The meal photo is sent to OpenRouter
  and never stored; suggestions are only logged once picked.
- **The dashboard Timeline's event strip is a second chart** padded to the main chart's plot area
  (an inline plugin reports `chartArea`), so markers line up with dates. Events are descriptive
  markers only — same rule as Pacing.
  Each source (`exposures`, `health_notes`, `medication_history`) has a `hide_from_timeline`
  flag that `get_timeline_events` filters on — a new marker source needs one too.
- **Food vs fatigue is descriptive only**: mean fatigue on days an item was had / the day after,
  beside other tracked days. Same rule as Pacing — no scores, no forecasts.

- **Weekly summary: Rust owns the numbers, the model only writes prose.** `weekly.rs` builds
  `WeekMetrics` (scorecard vs the previous 8 weekly means, flagged at >= 1 SD; meds; food;
  exposures; new labs; changed vault notes; lag correlations with n shown) and the model
  narrates it. Stored in `weekly_summaries`, so old weeks never re-call the model. It is made
  by `ensureWeeklySummary()` from the layout *after* the launch import, un-awaited, for the
  last full week only (a missed Monday is caught up on the next launch). A "missed" dose is
  *inferred* (current schedule slots minus doses logged, on days with any dose logged) — there
  is no skipped-dose record. Lab results are listed once: `reported_lab_keys` in the previous
  summary's metrics suppresses repeats. Same descriptive-only rule: no forecasts.

## Workflow
Git repo, `origin` = github.com/Geffen111/health-tracker. Solo project — commit and
push straight to `main`, no feature branch or PR. Keep changes scoped per phase;
update `PLAN.md` status after each phase.
