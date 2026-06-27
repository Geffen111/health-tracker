# Health Tracker

A private, local-first Windows desktop app for tracking ME/CFS — built to replace a
hand-maintained "Fatigue Log" spreadsheet. It records daily fatigue, symptoms, sleep,
activity, cardio/blood-pressure, medication and work load, pulls in wearable data from
Samsung Health, and includes a PEM (post-exertional malaise) model plus an optional AI
assistant for asking questions of your own data.

Built with **Tauri v2** (Rust) + **SvelteKit 5** + **SQLite** (via `sqlx`). All data
lives on your machine; nothing is sent anywhere except the optional AI calls you opt into.

---

## Features

- **Dashboard** — at-a-glance trends and comparison charts (Chart.js).
- **Daily Log** — fatigue & headache ratings, free-text symptoms, sleep rating, steps,
  alcohol; per-day notes (work notes live here too).
- **Sleep** — watch-derived sleep stages (asleep / REM / deep / awake) with a metric selector.
- **Activity** — quick per-row time entry; energy cost is auto-detected from the activity
  type. Two defaults (Phone, Walking) are always ready; add more in Settings.
- **Cardio** — heart-rate summary and blood-pressure-monitor calibration.
- **Medication** — split into **Regular** / **Occasional**, with per-med schedule
  quick-add buttons (pre-filled dose + time), full editing, a **Ceased** section, an
  editable started/ceased **history**, and a supplements toggle.
- **Work** — hours & status with a collapsible week-by-week monthly view; defaults
  (full-day hours + work days) are configurable in Settings and pre-fill each day.
- **PEM Model** — 33 calibratable parameters driving the post-exertional-malaise model.
- **Ask** — natural-language questions answered from your data (OpenRouter; optional).
- **Settings** — health sync, work/activity defaults, appearance, data export, AI config,
  and a one-time spreadsheet import.

---

## Data & privacy

- The live database is a single SQLite file kept in your user data area, **outside** the
  app folder, so reinstalling/updating never touches it.
  - On Windows the production DB lives under your synced app folder
    (`%OneDrive%\Apps\HealthTracker\health.db`).
- The **OpenRouter API key is stored only on this device** (`%LOCALAPPDATA%\health-tracker\secrets.json`)
  and is never written to the synced settings file.
- The AI assistant sends **only your database schema and small aggregates** to the model —
  never raw rows. If you never enter a key, no data leaves the machine at all.

---

## Samsung Health sync (how it works)

There is **no Google Drive login** in the app. It reads CSV files from a local folder that
Google Drive mirrors onto the PC:

1. The **Health Sync** app (Android) exports Samsung Health data to Google Drive.
2. **Google Drive for Desktop** mirrors those files to a local drive (e.g. `G:\My Drive`).
3. Health Tracker reads four sub-folders from that root (exact names):
   - `Health Sync Steps` — daily step total
   - `Health Sync Heart rate` — average / min / max bpm
   - `Health Sync Sleep` — asleep / REM / deep / awake hours (a sleep session is
     attributed to its **wake day**)
   - `Health Sync Energy burned` — active calories

Set the root in **Settings → Watch & health sync**. Import runs silently on launch (if
auto-import is on) and via **Sync now** / **Full re-sync**. Imports are COALESCE-upserts,
so manually-entered values are never overwritten, and only files modified since the last
sync are reprocessed (unless you choose Full re-sync).

> First run: use **Full re-sync** and spot-check a day or two against Samsung Health.
> Watch the step total (a `00:00:00` cumulative row is currently included) and sleep
> day attribution.

---

## AI assistant (optional)

The **Ask** page and AI insights use [OpenRouter](https://openrouter.ai). Add your API
key in **Settings → AI assistant** and pick a model (default `deepseek/deepseek-v4-flash`,
cheap & fast; any OpenRouter model id works). Ask is hybrid text-to-SQL: the schema is
sent to the model, a SELECT-only query is generated and validated, run locally, and the
model phrases the answer.

---

## Development

Requirements: Rust (stable), Node + [pnpm](https://pnpm.io), and the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
pnpm install

# Run the full desktop app (real SQLite DB). Run this on Windows — WSL uses a
# different, empty Linux-side DB.
pnpm tauri dev

# Frontend only
pnpm dev

# Type-check the Svelte/TS
CI=true npm_config_verify_deps_before_run=false pnpm check

# Rust check / tests
cd src-tauri && cargo check && cargo test
```

### Building a release

```bash
pnpm tauri build
```

CI (GitHub Actions) builds Windows x64 + ARM64 installers (MSI + NSIS) on push and
publishes them, plus a `build-info.json` marker, to a rolling `latest` GitHub release.
The app compares that marker (fetched in Rust to avoid a CORS block on the release-asset
host) against the commit baked into the build and shows an in-app "Update available"
banner when a newer build exists.

---

## Project layout

```
src/                      SvelteKit frontend
  routes/                 one folder per screen (+layout.svelte = sidebar/shell)
  lib/
    components/           Toast, ConfirmDialog, charts, …
    stores/               theme / toast / confirm (Svelte 5 runes)
    formatDate.ts         timezone-safe date helpers (todayISO / shiftISO / …)
src-tauri/
  src/
    commands/             Tauri commands (daily, activity, medications, cardio,
                          work, csv_import, export, ai, ask, insights, settings,
                          update, …)
    models.rs             shared serde structs
    lib.rs                command registration (generate_handler!)
  migrations/             sqlx embedded migrations (run on startup; *.sql pinned to LF)
PLAN.md                   roadmap, PEM formulas, backlog
CLAUDE.md / HANDOVER.md   contributor / handover notes
```

Dates are displayed `DD/MM/YY` (en-AU); ISO is stored. `.gitattributes` pins `*.sql` to
LF so migration checksums stay stable across machines.
