<script lang="ts">
  import { formatDate } from '$lib/formatDate';

  let { summary }: { summary: any } = $props();

  const m = $derived(summary.metrics);
  const n = $derived(summary.narrative);

  function fmt(v: number | null | undefined, decimals: number): string {
    if (v == null) return '—';
    return v.toLocaleString(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  }

  /** Whether a flagged departure is good, bad or just a change, given which way is worse. */
  function tone(row: any): 'good' | 'bad' | 'plain' {
    if (!row.flag || row.higher_is_worse == null) return 'plain';
    const higher = row.flag === 'higher';
    return higher === row.higher_is_worse ? 'bad' : 'good';
  }

  function spark(history: (number | null)[]): string {
    const pts = history.map((v, i) => ({ v, i })).filter((p): p is { v: number; i: number } => p.v != null);
    if (pts.length < 2) return '';
    const lo = Math.min(...pts.map((p) => p.v));
    const hi = Math.max(...pts.map((p) => p.v));
    const span = hi - lo || 1;
    return pts
      .map((p) => `${((p.i / (history.length - 1)) * 76 + 2).toFixed(1)},${(20 - ((p.v - lo) / span) * 16).toFixed(1)}`)
      .join(' ');
  }

  const SEVERITY: Record<string, string> = { positive: 'sev-good', warning: 'sev-warn', critical: 'sev-bad' };

  const c = $derived(m.completeness);
  const hasMeds = $derived(
    m.meds.changes.length || m.meds.adherence.length || m.meds.occasional.length || m.meds.notes.length
  );
</script>

<div class="report">
  <p class="headline">{n.headline}</p>
  {#if n.overview}<p class="overview">{n.overview}</p>{/if}

  <div class="chips">
    <span class="chip">{c.fatigue_days}/{c.days_in_week} days rated</span>
    <span class="chip">{m.bad_days} bad day{m.bad_days === 1 ? '' : 's'} (fatigue ≥ 8)</span>
    <span class="chip">{m.good_days} good day{m.good_days === 1 ? '' : 's'} (≤ 4)</span>
    {#if c.steps_days < 7}<span class="chip dim">watch data on {c.steps_days}/7 days</span>{/if}
  </div>

  <section class="block">
    <h3>Scorecard <span class="sub">this week vs your previous 8 weeks</span></h3>
    <div class="table-wrap">
      <table class="score">
        <thead>
          <tr><th>Measure</th><th>This week</th><th>Last week</th><th>Usual</th><th></th><th>9-week trend</th></tr>
        </thead>
        <tbody>
          {#each m.scorecard as row}
            <tr>
              <td class="label">{row.label}</td>
              <td class="num strong">{fmt(row.this_week, row.decimals)}<span class="unit"> {row.unit}</span></td>
              <td class="num">{fmt(row.last_week, row.decimals)}</td>
              <td class="num">{fmt(row.baseline, row.decimals)}</td>
              <td>
                {#if row.flag}
                  <span class="flag {tone(row)}" title={row.strong ? 'Two or more standard deviations from your usual' : 'One or more standard deviations from your usual'}>
                    {row.flag === 'higher' ? '▲' : '▼'} {row.strong ? 'well ' : ''}{row.flag === 'higher' ? 'above' : 'below'} usual
                  </span>
                {/if}
              </td>
              <td>
                {#if spark(row.history)}
                  <svg width="80" height="22" viewBox="0 0 80 22" aria-hidden="true">
                    <polyline points={spark(row.history)} fill="none" stroke="var(--ts)" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round" />
                  </svg>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">"Usual" is the average of your previous 8 weekly averages. A measure is only flagged when this week sits at least one standard deviation away from it.</p>
  </section>

  {#each n.sections as section}
    {#if section.points?.length}
      <section class="block card-sec {SEVERITY[section.severity] ?? ''}">
        <h3><span class="icon">{section.icon}</span> {section.title}</h3>
        <ul>
          {#each section.points as p}<li>{p}</li>{/each}
        </ul>
      </section>
    {/if}
  {/each}

  {#if n.to_raise?.length}
    <section class="block card-sec sev-warn">
      <h3><span class="icon">🩺</span> Worth raising with a clinician</h3>
      <ul>{#each n.to_raise as p}<li>{p}</li>{/each}</ul>
    </section>
  {/if}

  {#if hasMeds}
    <section class="block">
      <h3>Medication detail</h3>
      {#if m.meds.changes.length}
        <h4>Changes</h4>
        <ul class="plain">{#each m.meds.changes as line}<li>{line}</li>{/each}</ul>
      {/if}
      {#if m.meds.adherence.length}
        <h4>Doses logged vs scheduled</h4>
        <div class="table-wrap">
          <table class="score">
            <thead><tr><th>Medication</th><th>Logged</th><th>Scheduled</th><th>No dose logged</th></tr></thead>
            <tbody>
              {#each m.meds.adherence as a}
                <tr>
                  <td class="label">{a.name}</td>
                  <td class="num">{a.logged_doses}</td>
                  <td class="num">{a.expected_doses}</td>
                  <td>{a.missed_doses ? a.days_missed.join(', ') : '—'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <p class="note">Inferred from each medication's current schedule, counting only days with at least one dose logged.
          {#if m.meds.days_without_any_dose.length}
            No doses at all were logged on {m.meds.days_without_any_dose.map(formatDate).join(', ')}.
          {/if}
        </p>
      {/if}
      {#if m.meds.occasional.length}
        <h4>Occasional medication</h4>
        <ul class="plain">
          {#each m.meds.occasional as o}<li>{o.name}: {o.doses} dose{o.doses === 1 ? '' : 's'} ({o.days.map(formatDate).join(', ')})</li>{/each}
        </ul>
      {/if}
      {#if m.meds.notes.length}
        <h4>Notes</h4>
        <ul class="plain">{#each m.meds.notes as line}<li>{line}</li>{/each}</ul>
      {/if}
    </section>
  {/if}

  {#if m.new_lab_results.length || m.vault_changes.length}
    <section class="block">
      <h3>Records</h3>
      {#if m.new_lab_results.length}
        <h4>New results</h4>
        <div class="table-wrap">
          <table class="score">
            <thead><tr><th>Test</th><th>Date</th><th>Result</th><th>Reference</th><th>Previous</th></tr></thead>
            <tbody>
              {#each m.new_lab_results as l}
                <tr>
                  <td class="label">{l.test}</td>
                  <td>{formatDate(l.date)}</td>
                  <td class="num strong">{l.value}{#if l.flag} <span class="flag bad">{l.flag}</span>{/if}</td>
                  <td>{l.reference ?? '—'}</td>
                  <td>{l.previous ?? '—'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
      {#if m.vault_changes.length}
        <h4>Notes added or edited in the vault</h4>
        <ul class="plain">
          {#each m.vault_changes as v}<li>{v.folder ? `${v.folder} — ` : ''}{v.title}</li>{/each}
        </ul>
      {/if}
    </section>
  {/if}

  <p class="foot">Week of {formatDate(m.week_start)} – {formatDate(m.week_end)} · written {summary.generated_at}. Descriptive only: it shows what was logged, not a forecast.</p>
</div>

<style>
  .report { display: flex; flex-direction: column; gap: 14px; }
  .headline { font-family: 'Source Serif 4', serif; font-size: 21px; font-weight: 600; color: var(--tp); line-height: 1.35; margin: 0; }
  .overview { font-size: 14.5px; color: var(--ts); line-height: 1.6; margin: 0; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; }
  .chip { font-size: 12px; font-weight: 600; padding: 4px 11px; border-radius: 999px; background: var(--accent-soft); color: var(--accent-fg); }
  .chip.dim { background: var(--inset); color: var(--ts); }

  .block { background: var(--card); border: 1px solid var(--border); border-radius: 16px; padding: 18px 22px; box-shadow: var(--shadow); }
  .block h3 { font-size: 15px; font-weight: 700; color: var(--tp); margin: 0 0 10px; display: flex; align-items: baseline; gap: 8px; }
  .block h3 .sub { font-size: 12.5px; font-weight: 500; color: var(--tm); }
  .block h4 { font-size: 12.5px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: var(--ts); margin: 14px 0 6px; }
  .icon { font-size: 16px; }
  .card-sec { border-left: 4px solid var(--border); }
  .card-sec.sev-good { border-left-color: var(--accent); }
  .card-sec.sev-warn { border-left-color: var(--amber); }
  .card-sec.sev-bad { border-left-color: var(--red); }
  ul { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 7px; font-size: 14px; color: var(--tp); line-height: 1.55; }
  ul.plain { font-size: 13.5px; gap: 4px; }

  .table-wrap { overflow-x: auto; }
  table.score { width: 100%; border-collapse: collapse; font-size: 13.5px; }
  .score th { text-align: left; font-size: 11.5px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--tm); font-weight: 700; padding: 6px 10px; border-bottom: 1px solid var(--border); }
  .score td { padding: 8px 10px; border-bottom: 1px solid var(--border); color: var(--ts); }
  .score tr:last-child td { border-bottom: none; }
  .score .label { color: var(--tp); font-weight: 600; }
  .score .num { font-variant-numeric: tabular-nums; }
  .score .strong { color: var(--tp); font-weight: 700; }
  .unit { color: var(--tm); font-weight: 500; font-size: 12px; }
  .flag { font-size: 12px; font-weight: 700; padding: 2px 8px; border-radius: 999px; background: var(--amber-soft); color: var(--amber-fg); white-space: nowrap; }
  .flag.bad { background: var(--red-soft); color: var(--red-fg); }
  .flag.good { background: var(--accent-soft); color: var(--accent-fg); }
  .note { font-size: 12.5px; color: var(--tm); margin: 10px 0 0; line-height: 1.5; }
  .foot { font-size: 12px; color: var(--tm); margin: 2px 0 0; }
</style>
