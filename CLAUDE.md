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
`daily_log`, `medications` (+ dose logging), `blood_pressure`, `activity`, `pem`
(risk model + calibration), `dashboard`, `import_xlsx` (one-time spreadsheet import via
calamine). Register new commands in `src-tauri/src/lib.rs`.

## Conventions / gotchas
- Dates stored `YYYY-MM-DD`; spreadsheet dates are Excel serials (convert on import).
- XLSX import is idempotent: upsert by `log_date` (`ON CONFLICT DO UPDATE`).
- Svelte 5: `<slot>` is deprecated — use `{@render children()}` (one warning remains in
  `+layout.svelte`).
- Frontend calls the backend via `invoke('<command>', { ... })` from `@tauri-apps/api/core`.

## Workflow
Not yet a git repo. Keep changes scoped per phase; update `PLAN.md` status after each phase.
