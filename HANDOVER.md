# Handover → OpenCode (Health Tracker)

This document hands the Health Tracker project from a Claude Code session to OpenCode (running
in WSL Ubuntu). Read this first, then [`PLAN.md`](PLAN.md) (full roadmap, PEM formulas,
backlog) and [`CLAUDE.md`](CLAUDE.md) (project guide). Everything here is current as of
2026-06-27.

---

## 1. What this project is
A **Tauri v2 + SvelteKit 5 + SQLite** Windows desktop app replacing a ME/CFS "Fatigue Log"
spreadsheet, for one user (the owner) to track daily health and predict **PEM** (post-exertional
malaise / crashes). Stack: SvelteKit 5 (runes) frontend ↔ Tauri `invoke` ↔ Rust commands ↔
SQLite via sqlx (embedded migrations). Same stack/conventions as the sibling **Family Finance**
app at `../family-finances` (consult its `CLAUDE.md` for patterns).

## 2. Current state (what's done)
- **Phases 1–4 complete & verified**: scaffold, XLSX import engine, the page set, PEM model.
- **Spreadsheet imported & verified** into the live DB: 126 daily logs (21/02–26/06 2026),
  425 activities, 215 BP readings, **793 medication doses**, 57 calibration params.
- **Section A (data model) complete & committed** — see §6.
- **Design handover brief prepared** for Claude Design (see §7) — section B (UI) is intended to
  follow once design comps come back.
- Repo: branch `main`, **no remote** (local only), clean tree, 3 commits:
  `b855c4a` (plan), `672c320` (Section A), `7b53c81` (initial). The working tree is the same
  files OpenCode sees via `/mnt/c/...`.

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
2. **Claude Design handover.** ✅ Brief written (§7). **Awaiting comps.**
3. **Section B — implement the designed UI** (the bulk of remaining work) once comps arrive.
4. **Section C — cross-cutting** (date format, etc.), folded in as screens are touched.

**What OpenCode can start now vs. what waits for design comps:**
- **Can start now (not design-dependent):**
  - **C1 — DD/MM/YY date display** everywhere (store stays ISO `YYYY-MM-DD`; add a shared
    formatter and apply it). The user prefers `DD/MM/YY`; times 24h `HH:MM`.
  - Wiring/using the **new backend commands** from §6 into existing pages (e.g. show medication
    history, the watch-calibration button) even before the visual redesign.
  - The two **A2 review items** (§8).
- **Waits for design comps (Section B UI):** the visual style/token system, and the per-screen
  redesigns (dashboard comparison charts #9, day-nav arrows #7, sleep metric selector #8, Cardio
  rework #5/#6, Medications management UI #3/#4, Import-into-Settings #2). If the user wants to
  proceed without waiting for comps, build functionally first, style later — but the agreed plan
  is design-first for the UI.

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

## 7. Design handover (Section B input)
Brief is at the **sibling folder** (kept out of this git root by convention):
`../health-tracker-design-handoff/README.md`
(WSL: `/mnt/c/Users/gavin/Projects/health-tracker-design-handoff/README.md`).

It's a greenfield design brief: app/ME-CFS context and design principles (low cognitive load,
calm-not-clinical, red reserved for real high PEM risk, accessible), per-screen requirements
folding in **all** the user's UX notes, a data dictionary, and PEM context. The workflow: user
takes it to claude.ai/design → comps come back (`.dc.html` + screenshots + `ICONS.md`) into that
folder → implement into the existing SvelteKit routes (backend wiring unchanged). The brief
leaves the **visual direction open** (Design proposes it); it may share a family resemblance with
Family Finance's "Hearth" theme but be its own calmer identity.

## 8. Loose ends / things to be aware of
- **Two A2 items flagged for the user** (not yet resolved): the `add_meds` cell
  `"Rizatriptan ODT x 1 - 1:30"` was migrated assuming **13:30** (1:30 PM); and `"Flu Vax 12:50pm"`
  was **not** migrated (it's a vaccination, not a tracked med). Confirm/adjust with the user.
- The **importer is one-time** and idempotent (daily/BP/meds upsert; activity_log is cleared
  first). The default import path in `src/routes/import/+page.svelte` is the Windows path to
  `Copy of Fatigue_Log_V5 26 June.xlsx` (misnamed — it's actually the current "V6" data).
- **Routes:** `/` (dashboard), `/daily`, `/sleep`, `/activity`, `/cardio`, `/medication`,
  `/work`, `/pem-model`, `/settings`, `/import`. Sidebar/layout in `src/routes/+layout.svelte`
  (still uses a deprecated `<slot>` → should become `{@render children()}`).
- **Known frontend warnings:** ~13 svelte-check a11y "label not associated with control"
  warnings exist across pages — fine to clean up as screens are redesigned.

## 9. How to continue (suggested first move for OpenCode)
1. Read `PLAN.md` + this file. Confirm `pnpm check` is 0 errors and `cargo check` passes.
2. If design comps are in `../health-tracker-design-handoff/`, begin Section B from the tokens
   first (define the CSS custom properties in `+layout.svelte`), then screen by screen.
3. If comps are not yet back, do the non-design-dependent items: C1 (DD/MM/YY formatter) and
   surface the new backend data (medication history, watch calibration) in the existing pages.
4. Keep `PLAN.md` updated as phases complete (this is what a prior lost session failed to do).
5. Commit per coherent unit; end commit messages with the Co-Authored-By trailer. No remote, so
   no pushing needed.
