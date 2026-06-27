<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { onMount } from 'svelte';
  import { marked } from 'marked';
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
  interface VaultIndex {
    root: string;
    exists: boolean;
    notes: VaultNote[];
  }
  interface VaultNoteContent {
    rel_path: string;
    title: string;
    body: string;
  }

  let loading = $state(true);
  let index = $state<VaultIndex | null>(null);
  let query = $state('');
  let selected = $state<string | null>(null);
  let content = $state<VaultNoteContent | null>(null);
  let rendered = $state('');
  let loadingNote = $state(false);

  marked.setOptions({ gfm: true, breaks: false });

  // Order folders sensibly; anything not listed falls after, alphabetically.
  const FOLDER_ORDER = ['Health Topics', 'Pathology Results', 'Reports', 'Documents', 'Insurance'];
  const ROOT_GROUP = 'General';

  // Lookup maps for resolving [[wikilinks]] (by filename stem and by title).
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

  // Group the (filtered) notes by folder, in preferred order.
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
      if (a === ROOT_GROUP) return 1; // general/root notes last
      if (b === ROOT_GROUP) return -1;
      return a.localeCompare(b);
    });
    return names.map((name) => ({ name, notes: map.get(name)! }));
  });

  onMount(async () => {
    try {
      index = await invoke<VaultIndex>('get_vault_index');
      if (index.exists && index.notes.length) {
        // Open the dashboard note first if it exists, else the first record.
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

  function renderMarkdown(md: string): string {
    // Embedded files (![[x]]) can't be shown inline — note them as plain text.
    let pre = md.replace(/!\[\[([^\]]+)\]\]/g, (_m, inner: string) => `*(embedded: ${inner.split('|')[0].trim()})*`);
    // Convert Obsidian [[target|alias]] / [[target]] into resolvable links.
    pre = pre.replace(/\[\[([^\]]+)\]\]/g, (_m, inner: string) => {
      const [target, alias] = inner.split('|');
      const label = (alias ?? target).trim();
      return `[${label}](wiki:${encodeURIComponent(target.trim())})`;
    });
    const html = marked.parse(pre) as string;
    return sanitize(html);
  }

  // The vault is the user's own local files, but strip scripts / inline handlers defensively.
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

  // Intercept clicks inside the rendered note: wikilinks navigate in-app,
  // external links open in the system browser.
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
    // ISO YYYY-MM-DD → DD/MM/YY (en-AU display convention).
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(d);
    return m ? `${m[3]}/${m[2]}/${m[1].slice(2)}` : d;
  }

  function prettyType(t: string | null): string {
    return t ? t.replace(/_/g, ' ') : '';
  }

  let selectedNote = $derived(index?.notes.find((n) => n.rel_path === selected) ?? null);
</script>

<div class="page-header">
  <div>
    <div class="page-title">Records</div>
    <div class="page-subtitle">Read-only view of your Health Records vault</div>
  </div>
  {#if index?.exists}
    <div class="count-pill">{index.notes.length} records</div>
  {/if}
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
      Set the correct folder in <a href="/settings" class="inline-link">Settings → Health Records vault</a>,
      then come back here.
    </p>
  </div>
{:else}
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
            <button
              class="record-item"
              class:active={selected === note.rel_path}
              onclick={() => selectNote(note.rel_path)}
            >
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
          <article class="markdown" onclick={handleContentClick}>
            {@html rendered}
          </article>
        {/if}
        {#if selectedNote.tags.length}
          <div class="tag-row">
            {#each selectedNote.tags as tag}<span class="tag">#{tag}</span>{/each}
          </div>
        {/if}
      {/if}
    </section>
  </div>
{/if}

<style>
  .page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
  .page-title { font-family: 'Source Serif 4', serif; font-size: 26px; font-weight: 600; color: var(--tp); }
  .page-subtitle { font-size: 13.5px; color: var(--ts); margin-top: 2px; }
  .count-pill {
    font-size: 12.5px; font-weight: 600; color: var(--accent-fg);
    background: var(--accent-soft); padding: 6px 12px; border-radius: 20px; white-space: nowrap;
  }

  .empty-state { color: var(--tm); font-size: 14px; padding: 40px 4px; text-align: center; }

  .missing-card { max-width: 560px; }
  .card { background: var(--card); border: 1px solid var(--border); border-radius: 16px; padding: 22px; box-shadow: var(--shadow); display: flex; flex-direction: column; gap: 8px; }
  .card-heading { font-size: 15px; font-weight: 700; color: var(--tp); }
  .card-subtitle { font-size: 13px; color: var(--ts); line-height: 1.5; }
  .mono-path { font-family: ui-monospace, 'Cascadia Code', monospace; font-size: 12.5px; color: var(--tp); background: var(--inset); border: 1px solid var(--border); border-radius: 8px; padding: 7px 10px; margin-top: 8px; word-break: break-all; }
  .missing-note { font-size: 13px; color: var(--ts); margin-top: 4px; }
  .inline-link { color: var(--accent-fg); font-weight: 600; text-decoration: none; }
  .inline-link:hover { text-decoration: underline; }

  .records-layout { display: grid; grid-template-columns: 290px 1fr; gap: 22px; align-items: start; }

  .record-list {
    position: sticky; top: 0;
    max-height: calc(100vh - 150px); overflow-y: auto;
    display: flex; flex-direction: column; gap: 4px;
    padding-right: 4px;
  }
  .search-input {
    width: 100%; padding: 9px 12px; margin-bottom: 8px;
    border: 1px solid var(--border); border-radius: 11px;
    background: var(--card); color: var(--tp); font-size: 13px;
  }
  .search-input:focus { outline: none; border-color: var(--accent); }
  .no-results { font-size: 12.5px; color: var(--tm); padding: 8px 6px; }

  .list-group { margin-bottom: 10px; }
  .group-label {
    font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em;
    color: var(--tm); padding: 6px 8px 4px;
  }
  .record-item {
    display: flex; flex-direction: column; gap: 2px; width: 100%; text-align: left;
    padding: 8px 11px; border: none; border-radius: 10px; background: transparent;
    cursor: pointer; color: var(--ts); transition: background 0.13s, color 0.13s;
  }
  .record-item:hover { background: var(--accent-soft); }
  .record-item.active { background: var(--accent-soft); color: var(--accent-fg); }
  .record-title { font-size: 13px; font-weight: 500; line-height: 1.3; }
  .record-item.active .record-title { font-weight: 700; }
  .record-date { font-size: 11px; color: var(--tm); }

  .record-view {
    background: var(--card); border: 1px solid var(--border); border-radius: 16px;
    padding: 28px 32px; box-shadow: var(--shadow); min-height: 300px; min-width: 0;
  }
  .record-meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-bottom: 16px; }
  .meta-chip {
    font-size: 11.5px; font-weight: 600; color: var(--accent-fg);
    background: var(--accent-soft); padding: 4px 10px; border-radius: 14px;
  }
  .meta-chip.subtle { color: var(--ts); background: var(--inset); text-transform: capitalize; }
  .meta-date { font-size: 12px; color: var(--tm); margin-left: auto; }

  .tag-row { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 22px; padding-top: 16px; border-top: 1px solid var(--border); }
  .tag { font-size: 11.5px; color: var(--ts); background: var(--inset); padding: 3px 9px; border-radius: 12px; }

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
