<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { onMount } from 'svelte';
  import { marked } from 'marked';
  import Chart from '$lib/Chart.svelte';
  import { showToast } from '$lib/stores/toast.svelte';

  interface VaultNote {
    rel_path: string;
    title: string;
    folder: string;
    stem: string;
    note_type: string | null;
    date: string | null;
    tags: string[];
  }
  interface VaultIndex { root: string; exists: boolean; notes: VaultNote[]; }
  interface VaultNoteContent { rel_path: string; title: string; body: string; }

  interface LabTestSummary {
    test_name: string; category: string | null; n: number;
    latest_date: string; latest_value_text: string | null;
    latest_value_num: number | null; unit: string | null; flag: string | null;
  }
  interface LabPoint {
    result_date: string; value_num: number | null; value_text: string | null;
    unit: string | null; ref_low: number | null; ref_high: number | null;
    ref_text: string | null; flag: string | null; source_note: string;
  }
  interface SourceRef { title: string; rel_path: string; }
  interface RecordsAnswer { answer: string; sources: SourceRef[]; }

  type Tab = 'browse' | 'labs' | 'ask';
  let tab = $state<Tab>('browse');

  // ── Browse ──
  let loading = $state(true);
  let index = $state<VaultIndex | null>(null);
  let query = $state('');
  let selected = $state<string | null>(null);
  let content = $state<VaultNoteContent | null>(null);
  let rendered = $state('');
  let loadingNote = $state(false);

  marked.setOptions({ gfm: true, breaks: false });

  const FOLDER_ORDER = ['Health Topics', 'Pathology Results', 'Reports', 'Documents', 'Insurance'];
  const ROOT_GROUP = 'General';

  let byStem = $derived(buildLookup(index?.notes ?? [], (n) => n.stem));
  let byTitle = $derived(buildLookup(index?.notes ?? [], (n) => n.title));

  function buildLookup(notes: VaultNote[], key: (n: VaultNote) => string): Map<string, string> {
    const m = new Map<string, string>();
    for (const n of notes) m.set(key(n).toLowerCase(), n.rel_path);
    return m;
  }

  let filtered = $derived.by(() => {
    const notes = index?.notes ?? [];
    const q = query.trim().toLowerCase();
    if (!q) return notes;
    return notes.filter((n) => {
      const hay = `${n.title} ${n.stem} ${n.note_type ?? ''} ${n.folder} ${n.tags.join(' ')}`.toLowerCase();
      return hay.includes(q);
    });
  });

  let groups = $derived.by(() => {
    const map = new Map<string, VaultNote[]>();
    for (const n of filtered) {
      const g = n.folder || ROOT_GROUP;
      (map.get(g) ?? map.set(g, []).get(g)!).push(n);
    }
    const names = [...map.keys()].sort((a, b) => {
      const ia = FOLDER_ORDER.indexOf(a);
      const ib = FOLDER_ORDER.indexOf(b);
      if (ia !== -1 || ib !== -1) return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
      if (a === ROOT_GROUP) return 1;
      if (b === ROOT_GROUP) return -1;
      return a.localeCompare(b);
    });
    return names.map((name) => ({ name, notes: map.get(name)! }));
  });

  onMount(async () => {
    try {
      index = await invoke<VaultIndex>('get_vault_index');
      if (index.exists && index.notes.length) {
        const dash = index.notes.find((n) => /dashboard/i.test(n.stem));
        selectNote((dash ?? index.notes[0]).rel_path);
      }
    } catch (e) {
      console.error('Error loading vault index:', e);
      showToast('Could not read the records vault', 'error');
    } finally {
      loading = false;
    }
  });

  async function selectNote(relPath: string) {
    selected = relPath;
    loadingNote = true;
    try {
      content = await invoke<VaultNoteContent>('read_vault_note', { relPath });
      rendered = renderMarkdown(content.body);
    } catch (e) {
      console.error('Error reading note:', e);
      rendered = '';
      showToast('Could not open that record', 'error');
    } finally {
      loadingNote = false;
    }
  }

  function openInBrowse(relPath: string) {
    tab = 'browse';
    selectNote(relPath);
  }

  function renderMarkdown(md: string): string {
    let pre = md.replace(/!\[\[([^\]]+)\]\]/g, (_m, inner: string) => `*(embedded: ${inner.split('|')[0].trim()})*`);
    pre = pre.replace(/\[\[([^\]]+)\]\]/g, (_m, inner: string) => {
      const [target, alias] = inner.split('|');
      const label = (alias ?? target).trim();
      return `[${label}](wiki:${encodeURIComponent(target.trim())})`;
    });
    const html = marked.parse(pre) as string;
    return sanitize(html);
  }

  function sanitize(html: string): string {
    return html
      .replace(/<script[\s\S]*?<\/script>/gi, '')
      .replace(/<style[\s\S]*?<\/style>/gi, '')
      .replace(/ on[a-z]+="[^"]*"/gi, '')
      .replace(/ on[a-z]+='[^']*'/gi, '');
  }

  function resolveWiki(target: string): string | null {
    const key = target.toLowerCase();
    return byStem.get(key) ?? byTitle.get(key) ?? null;
  }

  function handleContentClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest('a');
    if (!a) return;
    const href = a.getAttribute('href') ?? '';
    if (href.startsWith('wiki:')) {
      e.preventDefault();
      const target = decodeURIComponent(href.slice(5));
      const rel = resolveWiki(target);
      if (rel) selectNote(rel);
      else showToast(`No record named "${target}"`, 'error');
    } else if (/^https?:/i.test(href)) {
      e.preventDefault();
      openUrl(href).catch(() => {});
    }
  }

  function prettyDate(d: string | null): string {
    if (!d) return '';
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(d);
    return m ? `${m[3]}/${m[2]}/${m[1].slice(2)}` : d;
  }
  function prettyType(t: string | null): string { return t ? t.replace(/_/g, ' ') : ''; }

  let selectedNote = $derived(index?.notes.find((n) => n.rel_path === selected) ?? null);

  // ── Labs ──
  let labsLoaded = $state(false);
  let labTests = $state<LabTestSummary[]>([]);
  let selectedTest = $state<string | null>(null);
  let series = $state<LabPoint[]>([]);
  let labsLastExtract = $state<string | null>(null);
  let extracting = $state(false);
  let extractMsg = $state('');

  let labGroups = $derived.by(() => {
    const map = new Map<string, LabTestSummary[]>();
    for (const t of labTests) {
      const g = t.category || 'Other';
      (map.get(g) ?? map.set(g, []).get(g)!).push(t);
    }
    return [...map.entries()].map(([name, tests]) => ({ name, tests }));
  });

  async function ensureLabs() {
    if (labsLoaded) return;
    try {
      labsLastExtract = await invoke<string | null>('get_labs_last_extract');
      labTests = await invoke<LabTestSummary[]>('get_lab_tests');
      labsLoaded = true;
      if (labTests.length && !selectedTest) selectTest(labTests[0].test_name);
    } catch (e) {
      console.error('Error loading labs:', e);
      showToast('Could not load lab results', 'error');
    }
  }

  async function selectTest(name: string) {
    selectedTest = name;
    try {
      series = await invoke<LabPoint[]>('get_lab_series', { testName: name });
    } catch (e) {
      console.error('Error loading series:', e);
      series = [];
    }
  }

  async function extractNow() {
    extracting = true;
    extractMsg = '';
    try {
      const r: any = await invoke('extract_lab_results');
      labsLastExtract = r.extracted_at;
      labTests = await invoke<LabTestSummary[]>('get_lab_tests');
      labsLoaded = true;
      extractMsg = `Extracted ${r.rows_extracted} results from ${r.notes_processed} notes`
        + (r.notes_failed ? ` · ${r.notes_failed} note(s) failed` : '');
      if (labTests.length) selectTest(selectedTest && labTests.some(t => t.test_name === selectedTest) ? selectedTest : labTests[0].test_name);
      showToast('Pathology data updated');
    } catch (e) {
      console.error('Extraction failed:', e);
      extractMsg = `Extraction failed: ${e}`;
      showToast('Extraction failed', 'error');
    } finally {
      extracting = false;
    }
  }

  let selectedTestSummary = $derived(labTests.find((t) => t.test_name === selectedTest) ?? null);
  let hasRef = $derived(series.some((p) => p.ref_low != null || p.ref_high != null));

  let chartLabels = $derived(series.map((p) => prettyDate(p.result_date)));
  let chartDatasets = $derived.by<any[]>(() => {
    const ds: any[] = [
      {
        label: selectedTest ?? 'Value',
        data: series.map((p) => p.value_num),
        borderColor: 'var(--accent)',
        backgroundColor: 'var(--accent)',
        tension: 0.25,
        spanGaps: true,
        pointRadius: series.map((p) => (p.flag ? 6 : 4)),
        pointBackgroundColor: series.map((p) => (p.flag ? 'var(--red)' : 'var(--accent)')),
        pointBorderColor: series.map((p) => (p.flag ? 'var(--red)' : 'var(--accent)')),
        order: 0,
      },
    ];
    if (hasRef) {
      ds.push({
        label: 'Ref high', data: series.map((p) => p.ref_high),
        borderColor: 'var(--amber-soft)', borderDash: [4, 4], pointRadius: 0,
        backgroundColor: 'var(--amber-soft)', fill: '+1', tension: 0, order: 2,
      });
      ds.push({
        label: 'Ref low', data: series.map((p) => p.ref_low),
        borderColor: 'var(--amber-soft)', borderDash: [4, 4], pointRadius: 0,
        fill: false, tension: 0, order: 3,
      });
    }
    return ds;
  });

  // ── Ask ──
  let askQ = $state('');
  let asking = $state(false);
  let askAnswer = $state('');
  let askSources = $state<SourceRef[]>([]);
  const ASK_SUGGESTIONS = [
    'Summarise my neurology history',
    'What were my most recent abnormal results?',
    'Which specialists have I been referred to?',
  ];

  async function runAsk() {
    const q = askQ.trim();
    if (!q || asking) return;
    asking = true;
    askAnswer = '';
    askSources = [];
    try {
      const r = await invoke<RecordsAnswer>('ask_records', { question: q });
      askAnswer = r.answer;
      askSources = r.sources;
    } catch (e) {
      console.error('ask_records failed:', e);
      askAnswer = `Couldn't answer that: ${e}`;
    } finally {
      asking = false;
    }
  }

  function switchTab(t: Tab) {
    tab = t;
    if (t === 'labs') ensureLabs();
  }
</script>

<div class="page-header">
  <div>
    <div class="page-title">Records</div>
    <div class="page-subtitle">Your Health Records vault — browse, chart pathology, and ask</div>
  </div>
  {#if index?.exists}
    <div class="count-pill">{index.notes.length} records</div>
  {/if}
</div>

<div class="tab-bar">
  <button class="tab" class:active={tab === 'browse'} onclick={() => switchTab('browse')}>Browse</button>
  <button class="tab" class:active={tab === 'labs'} onclick={() => switchTab('labs')}>Labs</button>
  <button class="tab" class:active={tab === 'ask'} onclick={() => switchTab('ask')}>Ask</button>
</div>

{#if loading}
  <div class="empty-state">Loading records…</div>
{:else if !index?.exists}
  <div class="card missing-card">
    <div class="card-heading">Vault not found</div>
    <div class="card-subtitle">
      Couldn't find the Health Records vault at:
      <div class="mono-path">{index?.root}</div>
    </div>
    <p class="missing-note">
      Set the correct folder in <a href="/settings" class="inline-link">Settings → Health Records vault</a>, then come back here.
    </p>
  </div>

{:else if tab === 'browse'}
  <div class="records-layout">
    <aside class="record-list">
      <input class="search-input" placeholder="Search records…" bind:value={query} />
      {#if filtered.length === 0}
        <div class="no-results">No records match “{query}”.</div>
      {/if}
      {#each groups as group (group.name)}
        <div class="list-group">
          <div class="group-label">{group.name}</div>
          {#each group.notes as note (note.rel_path)}
            <button class="record-item" class:active={selected === note.rel_path} onclick={() => selectNote(note.rel_path)}>
              <span class="record-title">{note.title}</span>
              {#if note.date}<span class="record-date">{prettyDate(note.date)}</span>{/if}
            </button>
          {/each}
        </div>
      {/each}
    </aside>

    <section class="record-view">
      {#if !selectedNote}
        <div class="empty-state">Select a record to read it.</div>
      {:else}
        <div class="record-meta">
          {#if selectedNote.folder}<span class="meta-chip">{selectedNote.folder}</span>{/if}
          {#if selectedNote.note_type}<span class="meta-chip subtle">{prettyType(selectedNote.note_type)}</span>{/if}
          {#if selectedNote.date}<span class="meta-date">{prettyDate(selectedNote.date)}</span>{/if}
        </div>
        {#if loadingNote}
          <div class="empty-state">Loading…</div>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
          <article class="markdown" onclick={handleContentClick}>{@html rendered}</article>
        {/if}
        {#if selectedNote.tags.length}
          <div class="tag-row">{#each selectedNote.tags as tag}<span class="tag">#{tag}</span>{/each}</div>
        {/if}
      {/if}
    </section>
  </div>

{:else if tab === 'labs'}
  <div class="labs-toolbar">
    <div class="labs-toolbar-text">
      {#if labsLastExtract}
        <span class="labs-sub">Last extracted {prettyDate(labsLastExtract.slice(0, 10))}</span>
      {:else}
        <span class="labs-sub">No pathology data extracted yet</span>
      {/if}
      {#if extractMsg}<span class="extract-msg">{extractMsg}</span>{/if}
    </div>
    <button class="extract-btn" onclick={extractNow} disabled={extracting}>
      {extracting ? 'Extracting… (this can take a minute)' : labsLastExtract ? 'Re-extract pathology' : 'Extract pathology data'}
    </button>
  </div>

  {#if labTests.length === 0}
    <div class="card missing-card">
      <div class="card-heading">No lab results yet</div>
      <div class="card-subtitle">
        Click <strong>Extract pathology data</strong> above. The app reads each pathology note in your vault and pulls
        the numeric results into a chartable table. Values keep a link back to the source note so you can verify them.
      </div>
      <p class="missing-note">Extraction sends note content to your configured AI provider (OpenRouter) — your choice, set in Settings.</p>
    </div>
  {:else}
    <div class="records-layout">
      <aside class="record-list">
        {#each labGroups as group (group.name)}
          <div class="list-group">
            <div class="group-label">{group.name}</div>
            {#each group.tests as t (t.test_name)}
              <button class="record-item" class:active={selectedTest === t.test_name} onclick={() => selectTest(t.test_name)}>
                <span class="record-title">{t.test_name}</span>
                <span class="lab-latest">
                  {t.latest_value_text ?? (t.latest_value_num ?? '')}{t.unit ? ' ' + t.unit : ''}
                  {#if t.flag}<span class="flag-badge">{t.flag}</span>{/if}
                  <span class="lab-count">· {t.n}</span>
                </span>
              </button>
            {/each}
          </div>
        {/each}
      </aside>

      <section class="record-view">
        {#if !selectedTestSummary}
          <div class="empty-state">Select a marker to chart it.</div>
        {:else}
          <div class="lab-head">
            <div>
              <div class="lab-title">{selectedTestSummary.test_name}</div>
              {#if selectedTestSummary.category}<div class="lab-cat">{selectedTestSummary.category}</div>{/if}
            </div>
            <div class="lab-latest-big">
              {selectedTestSummary.latest_value_text ?? (selectedTestSummary.latest_value_num ?? '—')}
              {#if selectedTestSummary.unit}<span class="lab-unit">{selectedTestSummary.unit}</span>{/if}
              {#if selectedTestSummary.flag}<span class="flag-badge">{selectedTestSummary.flag}</span>{/if}
            </div>
          </div>

          {#if series.filter((p) => p.value_num != null).length >= 1}
            <Chart type="line" labels={chartLabels} datasets={chartDatasets} chartArea="260px" />
            {#if hasRef}<div class="chart-caption">Shaded band = reference range · red points = flagged abnormal</div>{/if}
          {:else}
            <div class="empty-state">This marker has no numeric values to chart (qualitative result).</div>
          {/if}

          <table class="lab-table">
            <thead>
              <tr><th>Date</th><th>Result</th><th>Unit</th><th>Reference</th><th>Flag</th><th>Source</th></tr>
            </thead>
            <tbody>
              {#each [...series].reverse() as p (p.result_date + p.source_note)}
                <tr class:flagged={!!p.flag}>
                  <td>{prettyDate(p.result_date)}</td>
                  <td class="num">{p.value_text ?? (p.value_num ?? '')}</td>
                  <td>{p.unit ?? ''}</td>
                  <td>{p.ref_text ?? ''}</td>
                  <td>{#if p.flag}<span class="flag-badge">{p.flag}</span>{/if}</td>
                  <td><button class="src-link" onclick={() => openInBrowse(p.source_note)}>note</button></td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </section>
    </div>
  {/if}

{:else if tab === 'ask'}
  <div class="ask-wrap">
    <div class="card ask-card">
      <div class="card-heading">Ask your records</div>
      <div class="card-subtitle">Free-text questions over your vault notes (referrals, topics, timeline, pathology). Answers cite the notes used.</div>
      <div class="ask-row">
        <input class="ask-input" placeholder="e.g. Summarise my neurology history" bind:value={askQ}
          onkeydown={(e) => e.key === 'Enter' && runAsk()} />
        <button class="extract-btn" onclick={runAsk} disabled={asking || !askQ.trim()}>
          {asking ? 'Thinking…' : 'Ask'}
        </button>
      </div>
      <div class="ask-suggest">
        {#each ASK_SUGGESTIONS as s}
          <button class="suggest-chip" onclick={() => { askQ = s; runAsk(); }}>{s}</button>
        {/each}
      </div>
      <p class="missing-note">Sends the relevant note text to your AI provider (OpenRouter) to answer. Not medical advice.</p>
    </div>

    {#if askAnswer}
      <div class="card answer-card">
        <div class="answer-text">{askAnswer}</div>
        {#if askSources.length}
          <div class="answer-sources">
            <span class="sources-label">Sources</span>
            {#each askSources as s (s.rel_path)}
              <button class="src-chip" onclick={() => openInBrowse(s.rel_path)}>{s.title}</button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 18px; }
  .page-title { font-family: 'Source Serif 4', serif; font-size: 26px; font-weight: 600; color: var(--tp); }
  .page-subtitle { font-size: 13.5px; color: var(--ts); margin-top: 2px; }
  .count-pill { font-size: 12.5px; font-weight: 600; color: var(--accent-fg); background: var(--accent-soft); padding: 6px 12px; border-radius: 20px; white-space: nowrap; }

  .tab-bar { display: flex; gap: 4px; margin-bottom: 22px; border-bottom: 1px solid var(--border); }
  .tab { background: none; border: none; padding: 9px 16px; font-size: 13.5px; font-weight: 600; color: var(--ts); cursor: pointer; border-bottom: 2px solid transparent; margin-bottom: -1px; }
  .tab:hover { color: var(--accent-fg); }
  .tab.active { color: var(--accent-fg); border-bottom-color: var(--accent); }

  .empty-state { color: var(--tm); font-size: 14px; padding: 40px 4px; text-align: center; }

  .missing-card { max-width: 620px; }
  .card { background: var(--card); border: 1px solid var(--border); border-radius: 16px; padding: 22px; box-shadow: var(--shadow); display: flex; flex-direction: column; gap: 8px; }
  .card-heading { font-size: 15px; font-weight: 700; color: var(--tp); }
  .card-subtitle { font-size: 13px; color: var(--ts); line-height: 1.5; }
  .mono-path { font-family: ui-monospace, 'Cascadia Code', monospace; font-size: 12.5px; color: var(--tp); background: var(--inset); border: 1px solid var(--border); border-radius: 8px; padding: 7px 10px; margin-top: 8px; word-break: break-all; }
  .missing-note { font-size: 12.5px; color: var(--tm); margin-top: 4px; line-height: 1.5; }
  .inline-link { color: var(--accent-fg); font-weight: 600; text-decoration: none; }
  .inline-link:hover { text-decoration: underline; }

  .records-layout { display: grid; grid-template-columns: 290px 1fr; gap: 22px; align-items: start; }
  .record-list { position: sticky; top: 0; max-height: calc(100vh - 190px); overflow-y: auto; display: flex; flex-direction: column; gap: 4px; padding-right: 4px; }
  .search-input { width: 100%; padding: 9px 12px; margin-bottom: 8px; border: 1px solid var(--border); border-radius: 11px; background: var(--card); color: var(--tp); font-size: 13px; }
  .search-input:focus { outline: none; border-color: var(--accent); }
  .no-results { font-size: 12.5px; color: var(--tm); padding: 8px 6px; }

  .list-group { margin-bottom: 10px; }
  .group-label { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: var(--tm); padding: 6px 8px 4px; }
  .record-item { display: flex; flex-direction: column; gap: 2px; width: 100%; text-align: left; padding: 8px 11px; border: none; border-radius: 10px; background: transparent; cursor: pointer; color: var(--ts); transition: background 0.13s, color 0.13s; }
  .record-item:hover { background: var(--accent-soft); }
  .record-item.active { background: var(--accent-soft); color: var(--accent-fg); }
  .record-title { font-size: 13px; font-weight: 500; line-height: 1.3; }
  .record-item.active .record-title { font-weight: 700; }
  .record-date { font-size: 11px; color: var(--tm); }
  .lab-latest { font-size: 11.5px; color: var(--tm); display: flex; align-items: center; gap: 5px; }
  .lab-count { color: var(--tm); }

  .record-view { background: var(--card); border: 1px solid var(--border); border-radius: 16px; padding: 28px 32px; box-shadow: var(--shadow); min-height: 300px; min-width: 0; }
  .record-meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-bottom: 16px; }
  .meta-chip { font-size: 11.5px; font-weight: 600; color: var(--accent-fg); background: var(--accent-soft); padding: 4px 10px; border-radius: 14px; }
  .meta-chip.subtle { color: var(--ts); background: var(--inset); text-transform: capitalize; }
  .meta-date { font-size: 12px; color: var(--tm); margin-left: auto; }

  .tag-row { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 22px; padding-top: 16px; border-top: 1px solid var(--border); }
  .tag { font-size: 11.5px; color: var(--ts); background: var(--inset); padding: 3px 9px; border-radius: 12px; }

  /* Labs */
  .labs-toolbar { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 18px; flex-wrap: wrap; }
  .labs-toolbar-text { display: flex; flex-direction: column; gap: 2px; }
  .labs-sub { font-size: 12.5px; color: var(--ts); }
  .extract-msg { font-size: 12px; color: var(--accent-fg); font-weight: 600; }
  .extract-btn { padding: 9px 16px; border: none; border-radius: 11px; background: var(--accent); color: #fff; font-size: 13px; font-weight: 600; cursor: pointer; white-space: nowrap; }
  .extract-btn:hover { filter: brightness(0.96); }
  .extract-btn:disabled { opacity: 0.6; cursor: default; }

  .lab-head { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 16px; gap: 12px; }
  .lab-title { font-family: 'Source Serif 4', serif; font-size: 20px; font-weight: 600; color: var(--tp); }
  .lab-cat { font-size: 12px; color: var(--tm); margin-top: 1px; }
  .lab-latest-big { font-size: 22px; font-weight: 700; color: var(--tp); display: flex; align-items: baseline; gap: 7px; }
  .lab-unit { font-size: 13px; font-weight: 500; color: var(--tm); }
  .flag-badge { font-size: 10px; font-weight: 700; color: var(--red-fg); background: var(--red-soft); padding: 1px 6px; border-radius: 8px; letter-spacing: 0.03em; align-self: center; }
  .chart-caption { font-size: 11.5px; color: var(--tm); margin-top: 8px; text-align: center; }

  .lab-table { width: 100%; border-collapse: collapse; margin-top: 22px; font-size: 13px; }
  .lab-table th, .lab-table td { border-bottom: 1px solid var(--border); padding: 7px 10px; text-align: left; }
  .lab-table th { font-size: 11px; text-transform: uppercase; letter-spacing: 0.03em; color: var(--tm); font-weight: 700; }
  .lab-table td.num { font-weight: 600; color: var(--tp); }
  .lab-table tr.flagged td.num { color: var(--red-fg); }
  .src-link, .src-chip { background: none; border: none; color: var(--accent-fg); font-size: 12.5px; font-weight: 600; cursor: pointer; padding: 0; }
  .src-link:hover, .src-chip:hover { text-decoration: underline; }

  /* Ask */
  .ask-wrap { display: flex; flex-direction: column; gap: 16px; max-width: 760px; }
  .ask-row { display: flex; gap: 8px; margin-top: 4px; }
  .ask-input { flex: 1; padding: 10px 13px; border: 1px solid var(--border); border-radius: 11px; background: var(--card); color: var(--tp); font-size: 14px; }
  .ask-input:focus { outline: none; border-color: var(--accent); }
  .ask-suggest { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 4px; }
  .suggest-chip { font-size: 12px; color: var(--ts); background: var(--inset); border: 1px solid var(--border); border-radius: 14px; padding: 5px 11px; cursor: pointer; }
  .suggest-chip:hover { background: var(--accent-soft); color: var(--accent-fg); border-color: var(--accent-soft); }
  .answer-card { gap: 14px; }
  .answer-text { font-size: 14px; line-height: 1.6; color: var(--tp); white-space: pre-wrap; }
  .answer-sources { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding-top: 12px; border-top: 1px solid var(--border); }
  .sources-label { font-size: 11px; text-transform: uppercase; letter-spacing: 0.03em; color: var(--tm); font-weight: 700; }
  .src-chip { background: var(--accent-soft); padding: 4px 10px; border-radius: 13px; }
  .src-chip:hover { text-decoration: none; filter: brightness(0.97); }

  /* Rendered markdown */
  .markdown { font-size: 14px; line-height: 1.6; color: var(--tp); min-width: 0; overflow-wrap: break-word; }
  .markdown :global(h1) { font-family: 'Source Serif 4', serif; font-size: 22px; font-weight: 600; margin: 0 0 14px; color: var(--tp); }
  .markdown :global(h2) { font-family: 'Source Serif 4', serif; font-size: 18px; font-weight: 600; margin: 22px 0 10px; color: var(--tp); }
  .markdown :global(h3) { font-size: 15px; font-weight: 700; margin: 18px 0 8px; color: var(--tp); }
  .markdown :global(h4) { font-size: 13.5px; font-weight: 700; margin: 14px 0 6px; color: var(--ts); }
  .markdown :global(p) { margin: 0 0 12px; }
  .markdown :global(ul), .markdown :global(ol) { margin: 0 0 12px; padding-left: 22px; }
  .markdown :global(li) { margin: 3px 0; }
  .markdown :global(a) { color: var(--accent-fg); text-decoration: none; font-weight: 500; cursor: pointer; }
  .markdown :global(a:hover) { text-decoration: underline; }
  .markdown :global(code) { font-family: ui-monospace, 'Cascadia Code', monospace; font-size: 12.5px; background: var(--inset); padding: 1px 5px; border-radius: 5px; }
  .markdown :global(pre) { background: var(--inset); border: 1px solid var(--border); border-radius: 10px; padding: 12px 14px; overflow-x: auto; margin: 0 0 14px; }
  .markdown :global(pre code) { background: none; padding: 0; }
  .markdown :global(blockquote) { border-left: 3px solid var(--border); margin: 0 0 12px; padding: 2px 0 2px 14px; color: var(--ts); }
  .markdown :global(hr) { border: none; border-top: 1px solid var(--border); margin: 20px 0; }
  .markdown :global(table) { border-collapse: collapse; width: 100%; margin: 0 0 16px; font-size: 13px; }
  .markdown :global(th), .markdown :global(td) { border: 1px solid var(--border); padding: 7px 10px; text-align: left; }
  .markdown :global(th) { background: var(--inset); font-weight: 700; color: var(--tp); }
  .markdown :global(tbody tr:nth-child(even)) { background: var(--inset); }
  .markdown :global(strong) { font-weight: 700; color: var(--tp); }
  .markdown :global(img) { max-width: 100%; border-radius: 8px; }

  @media (max-width: 900px) {
    .records-layout { grid-template-columns: 1fr; }
    .record-list { position: static; max-height: none; }
  }
</style>
