# Handover → Claude Code (Health Tracker)

This document hands the Health Tracker project from an OpenCode session back to Claude Code.
Read this first, then [`PLAN.md`](PLAN.md) (full roadmap, PEM formulas, backlog) and
[`CLAUDE.md`](CLAUDE.md) (project guide). Everything here is current as of 2026-06-27.

---

## 1. What this project is
A **Tauri v2 + SvelteKit 5 + SQLite** Windows desktop app replacing a ME/CFS "Fatigue Log"
spreadsheet, for one user (the owner) to track daily health and predict **PEM** (post-exertional
malaise / crashes). Stack: SvelteKit 5 (runes) frontend ↔ Tauri `invoke` ↔ Rust commands ↔
SQLite via sqlx (embedded migrations). Same stack/conventions as the sibling **Family Finance**
app at `../family-finances` (consult its `CLAUDE.md` for patterns).

## 2. Current state (what's done)
- **Phases 1–4 complete & verified**: scaffold, XLSX import engine, the page set, PEM model.
- **Phase 5 (Meridian CSS overhaul) complete**: full design token system, SVG nav icons, 9 redesigned
  screens matching design comps from `../health-tracker-claude-design-files/`.
- **Phase 9 (Chart.js integration) complete**: interactive dual-line compare-signals chart on
  Dashboard, selectable-metric 30-day trend chart on Sleep, sleep sparkline on Dashboard.
- **Section B (UI redesign) complete**: all 9 screens redesigned per Meridian design comps.
- **Section C (cross-cutting) complete**: C1 (DD/MM/YY formatter), a11y warnings resolved.
- **Spreadsheet imported & verified** into the live DB: 126 daily logs (21/02–26/06 2026),
  425 activities, 215 BP readings, **793 medication doses**, 57 calibration params.
- **Section A (data model) complete** — see §6.
- **Design comps received** at `../health-tracker-claude-design-files/` — "Meridian" theme,
  implemented into all existing routes.
- Repo: branch `main`, **no remote** (local only), clean tree, working tree matches these docs.

## 3. ⚠️ Critical environment notes (WSL vs Windows)
OpenCode runs in **WSL Ubuntu**; this app is a **native Windows** desktop app. This matters:

- **Project path in WSL:** `/mnt/c/Users/gavin/Projects/health-tracker`
- **The real database is on Windows OneDrive:**
  `C:\Users\gavin\OneDrive\Apps\HealthTracker\health.db`
  (WSL: `/mnt/c/Users/gavin/OneDrive/Apps/HealthTracker/health.db`). The app picks this path
  from the `%OneDrive%` env var. **In WSL that env var doesn't exist**, so if you run the app
  inside WSL it will fall back to a *different* Linux-side DB (`dirs::data_dir()/health-tracker/`)
  with **none of the imported data**. Don't be confused by an "empty" app in WSL.
- **Do code work in WSL; run/verify the real app on Windows.** Frontend work (`.svelte`/CSS) and
  type-checks are fully fine in WSL. But building/running the actual Windows app against the real
  OneDrive DB must happen on Windows (`pnpm tauri dev` in PowerShell). Ask the user to run it, or
  leave a verification note.
- **NEVER delete `health.db`.** It holds all the imported data and is OneDrive-synced across the
  user's machines.
- **Line endings:** `.gitattributes` pins `*.sql` to **LF** (the DB's sqlx migration checksums
  were computed from LF files — CRLF would cause a `VersionMismatch` panic). WSL writes LF
  natively, so just don't change this.

## 4. Verifying changes
Frontend type-check (must stay 0 errors) and Rust check — from the project root:
```bash
# pnpm 11 tries to reinstall before each run and prompts without a TTY — disable both:
CI=true npm_config_verify_deps_before_run=false pnpm check     # svelte-check, want 0 errors
cd src-tauri && cargo check                                    # Rust type-check (Linux target is fine for this)
```
Inspect the DB read-only with Node's built-in sqlite (no deps needed):
```bash
node -e "const {DatabaseSync}=require('node:sqlite'); const db=new DatabaseSync('/mnt/c/Users/gavin/OneDrive/Apps/HealthTracker/health.db'); console.log(db.prepare('SELECT COUNT(*) c FROM daily_logs').get());"
```
(If the live DB is locked by a running app, copy it first and query the copy.)

## 5. The agreed plan & sequence
Decided with the user (see `PLAN.md` "Feature backlog" for the full grouped list with IDs):

1. **Section A — data model/backend.** ✅ DONE.
2. **Claude Design handover.** ✅ Brief prepared, comps received at
   `../health-tracker-claude-design-files/`.
3. **Section B — implement the designed UI** ✅ DONE — Meridian theme implemented across all routes.
4. **Section C — cross-cutting** ✅ DONE — DD/MM/YY format, a11y cleanup.
5. **Phase 9 — Chart.js integration** ✅ DONE — Dashboard + Sleep interactive charts.

**What remains:**
- **Phase 7 — Google Drive CSV auto-import.** Backend not yet started. The `csv` crate is already
  in `Cargo.toml`. Needs: new `commands/csv_import.rs`, a settings store (JSON file in data dir),
  a scheduled/polling mechanism, and wiring in the Settings page (CSV path input, auto-import
  toggle, last-synced display).

**Recently done (2026-06-27, Claude Code):**
- **Data export** — `commands/export.rs` adds `export_csv` (one CSV per table in a timestamped
  folder) and `export_json` (full pretty-printed dump), both written to `<data_dir>/exports/`
  (OneDrive-synced). Settings buttons wired; `db::get_data_dir()` exposed for the shared path.
- **Blood-pressure monitor calibration** — the calibration log is now framed as BP-monitor
  calibration (every ~30 days), not a generic "watch sync". Cardio page takes manual date + time
  and shows "Last calibrated: …" from the latest record. The misleading hardcoded
  "Watch synced / 2 hours ago" sidebar block was removed. (Backend `log_watch_calibration` already
  accepted `cal_date`/`cal_time`; the `watch_calibration` table name is kept to avoid a migration.)
- **Removed the orphaned `/import` route** — import lives in Settings now.
- **AI Ask + Insights (Phase 10)** — OpenRouter (`deepseek-v4-flash`), mirroring Family Finance.
  New `/ask` route + nav item: natural-language Q&A (`ask_question` does hybrid text-to-SQL —
  schema sent, not data; SELECT-only validation + run locally + phrase answer) and an AI insights
  report (`get_insights`/`refresh_insights` aggregate fatigue/sleep/PEM/activity, cached in
  `ai_insights`, migration 20240611). API key entered in Settings, stored in
  `<data_dir>/settings.json`. Files: `commands/{ai,ask,insights,settings}.rs`. Only schema +
  aggregated figures are sent to the model — never raw rows.

## 6. Section A — what was added (backend API surface for the UI to use)
Migrations `20240608`–`20240610`; all commands registered in `src-tauri/src/lib.rs`.

- **Heart-rate range:** `daily_logs.hr_min` / `hr_max` columns (watch-synced later). Wired into
  `get_daily_log` / `upsert_daily_log` and the `DailyLog` model.
- **`upsert_daily_log` now COALESCEs** on conflict — a `null` incoming field *preserves* the
  stored value. This fixed a data-loss bug (the Work page was wiping sleep/steps/meds). **Design
  implication:** each page only needs to send the fields it manages; nulls won't clobber.
- **Medication lifecycle & history** (`medication_history` table):
  - `update_medication(...)` now returns `Option<String>` — a **banner message** when a dose or
    active/ceased change was recorded (show it in the UI). It auto-logs history rows.
  - `create_medication` logs a "started" event; `archive_medication` logs "ceased".
  - `get_medication_history(medication_id?)`, `add_medication_history(...)`,
    `update_medication_history(id, ...)`, `delete_medication_history(id)` — for the editable
    history timeline (#4).
- **Watch calibration** (`watch_calibration` table): `log_watch_calibration(date?, time?, notes?)`
  (defaults to now), `list_watch_calibrations(limit?)`, `delete_watch_calibration(id)`,
  `days_since_calibration()` → `Option<i64>` for the ~30-day overdue nudge (#5).
- **Variable BP readings** are already supported: `blood_pressure` is keyed by
  `(log_date, reading_num)`, and `upsert_bp` upserts by those. The Cardio "Add reading" UI (#5)
  just increments `reading_num` — no backend change needed.
- **Additional meds migrated** (#3): 6 PRN meds created as entities (Ozempic, Loratadine,
  Cetirizine, Gaviscon, Rizatriptan, Paracetamol) with 67 historical doses parsed from the
  `daily_logs.add_meds` free text. Originals retained in `add_meds`.

Full existing command list is in `src-tauri/src/lib.rs` `invoke_handler![...]`.

## 7. Meridian design comps (Section B input)
Design comps are at `../health-tracker-claude-design-files/` (WSL:
`/mnt/c/Users/gavin/Projects/health-tracker-claude-design-files/`). Contents:

- **9 screen comps** (`.dc.html` files) — Dashboard, Daily Log, Sleep, Activity, Cardio,
  Medication, Work, PEM Model, Settings + 1 Dashboard Directions reference.
- **`ICONS.md`** — SVG line icons for all nav items and utility glyphs.
- **Design tokens** documented in each comp's `<style>` block (light + dark CSS custom
  properties).

The **Meridian** direction was chosen: calm, spa-like blue/teal palette, Source Serif 4
headings + Public Sans body, 18px soft cards, 999px pills/buttons. Distinct from Family
Finance's "Hearth" but shares the sidebar + page header + card structure.

**All comps have been implemented** into the existing SvelteKit routes. The backend/data
wiring was unchanged — the design layer was purely CSS + layout.

Key implementation notes:
- Chart.js replaces static SVG trends on Dashboard (compare signals) and Sleep (30-day trend).
- The `csv` crate is already in Cargo.toml (for Phase 7 CSV import).
- Day arrows, medication dose logging, watch calibration, and variable BP are all wired to
  real backend commands.

## 8. Loose ends / things to be aware of
- **Two A2 items flagged for the user** (not yet resolved): the `add_meds` cell
  `"Rizatriptan ODT x 1 - 1:30"` was migrated assuming **13:30** (1:30 PM); and `"Flu Vax 12:50pm"`
  was **not** migrated (it's a vaccination, not a tracked med). Confirm/adjust with the user.
- The **XLSX importer is one-time** and idempotent (daily/BP/meds upsert; activity_log is cleared
  first). The import UI now lives in Settings (collapsible section) — the old `/import` route
  still exists but is not linked from the sidebar.
- **Routes:** `/` (dashboard), `/daily`, `/sleep`, `/activity`, `/cardio`, `/medication`,
  `/work`, `/pem-model`, `/settings`. Nine nav items in the sidebar, all with SVG icons from
  `ICONS.md`. Import is no longer in the nav.
- **Chart.js component** at `src/lib/Chart.svelte` — Svelte 5 wrapper that reads CSS custom
  properties for theming. Uses `$effect` for reactive updates and cleanup.
- **Shared date formatter** at `src/lib/formatDate.ts` — `formatDate()` (DD/MM/YY),
  `formatDateLong()` (e.g. "Fri 27/06/26"), `formatDateShort()` (DD/MM), `formatTime()`.
- **Build status:** `pnpm check` → 0 errors, 0 warnings; `cargo check` → passes clean.
- **Phase 7 (CSV auto-import) not started** — backend needs to be built. The `csv` crate is
  ready in Cargo.toml. The Settings page has the CSV path input and auto-import toggle UI but
  no backend wiring yet.

## 9. Suggested next moves for Claude Code

1. Read `PLAN.md` + this file + `CLAUDE.md`. Confirm `pnpm check` and `cargo check` pass.
2. **Phase 7 — Google Drive CSV auto-import** is the largest remaining feature. It needs:
   - New Rust command module `commands/csv_import.rs` registered in `lib.rs`.
   - A settings store (JSON file in data dir, following Family Finance's pattern) to persist
     the CSV path and auto-import toggle.
   - Samsung Health CSV format parsing (the `csv` crate is already in `Cargo.toml`). Health Sync
     writes CSV with columns like `Date`, `Steps`, `HeartRate`, `Sleep` — needs investigation.
   - Upsert into `daily_logs` (steps, `ave_resting_hr`, `ave_hr`, `hr_min`, `hr_max`, sleep
     stages like `sleep_actual_asleep`, `sleep_rem`, `sleep_deep`, `sleep_awake`).
   - Frontend wiring: the Settings page CSV path input and auto-import toggle both exist in the
     UI but call no backend commands yet. The last-synced timestamp is also placeholder.
3. Smaller items — ✅ all done 2026-06-27 (data export, BP calibration manual logging +
   sidebar cleanup, `/import` route removal). See "Recently done" under §5.
4. Keep `PLAN.md` updated as work progresses.
5. Commit per coherent unit. No remote — no pushing needed.
