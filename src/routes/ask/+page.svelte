<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface AskResponse {
    answer: string;
    sql: string;
    explanation: string;
    columns: string[];
    rows: string[][];
    truncated: boolean;
  }

  interface InsightItem {
    title: string;
    detail: string;
    severity: string;
    icon: string;
  }

  interface HealthInsights {
    summary: string;
    patterns: InsightItem[];
    anomalies: InsightItem[];
    recommendations: InsightItem[];
    period_label: string;
    generated_at: string;
  }

  let darkMode = $state(false);
  function toggleTheme() {
    darkMode = !darkMode;
    document.documentElement.classList.toggle('dark', darkMode);
  }

  // Ask and Insights both need the OpenRouter key; guide to Settings rather than failing raw.
  let hasApiKey = $state(true);
  onMount(async () => {
    try {
      const k = await invoke<string | null>('get_api_key');
      hasApiKey = k != null && k.length > 0;
    } catch { hasApiKey = false; }
  });

  // --- Ask a question ---
  let question = $state('');
  let asking = $state(false);
  let answer = $state<AskResponse | null>(null);
  let askError = $state('');
  let showWorking = $state(false);

  const EXAMPLES = [
    'What was my average fatigue over the last 30 days?',
    'How many crash days did I have in the last 3 months?',
    'Is my fatigue worse the day after high-step days?',
    'What is my average sleep on days I worked at the office?',
  ];

  async function ask() {
    const q = question.trim();
    if (!q || asking) return;
    asking = true;
    askError = '';
    answer = null;
    showWorking = false;
    try {
      answer = await invoke<AskResponse>('ask_question', { question: q });
    } catch (e) {
      askError = String(e);
    } finally {
      asking = false;
    }
  }

  function useExample(ex: string) {
    question = ex;
    ask();
  }

  // --- AI Insights ---
  type InsightsRange = 'last1' | 'last3' | 'last6' | 'last12' | 'ytd';
  const RANGE_LABELS: Record<InsightsRange, string> = {
    last1: 'Last month',
    last3: 'Last 3 months',
    last6: 'Last 6 months',
    last12: 'Last 12 months',
    ytd: 'Year to date',
  };

  let insightsRange = $state<InsightsRange>('last3');
  let insights = $state<HealthInsights | null>(null);
  let insightsLoading = $state(false);
  let insightsError = $state('');
  let insightsGenerated = $state(false);

  function ymd(d: Date): string {
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${m}-${day}`;
  }

  function insightsParams(): { startDate?: string; endDate?: string } {
    const t = new Date();
    switch (insightsRange) {
      case 'last1':
        return {
          startDate: ymd(new Date(t.getFullYear(), t.getMonth() - 1, 1)),
          endDate: ymd(new Date(t.getFullYear(), t.getMonth(), 0)),
        };
      case 'last3':
        return { startDate: ymd(new Date(t.getFullYear(), t.getMonth() - 2, 1)) };
      case 'last6':
        return { startDate: ymd(new Date(t.getFullYear(), t.getMonth() - 5, 1)) };
      case 'last12':
        return { startDate: ymd(new Date(t.getFullYear(), t.getMonth() - 11, 1)) };
      case 'ytd':
        return { startDate: ymd(new Date(t.getFullYear(), 0, 1)) };
    }
  }

  function onRangeChange() {
    insights = null;
    insightsGenerated = false;
    insightsError = '';
  }

  async function generateInsights() {
    insightsLoading = true;
    insightsError = '';
    try {
      insights = await invoke<HealthInsights>('get_insights', insightsParams());
      insightsGenerated = true;
    } catch (e) {
      insightsError = String(e);
    } finally {
      insightsLoading = false;
    }
  }

  async function refreshInsights() {
    insightsLoading = true;
    insightsError = '';
    try {
      insights = await invoke<HealthInsights>('refresh_insights', insightsParams());
      insightsGenerated = true;
    } catch (e) {
      insightsError = String(e);
    } finally {
      insightsLoading = false;
    }
  }

  const allSections = $derived(insights
    ? [
        { heading: 'Patterns', items: insights.patterns },
        { heading: 'Anomalies', items: insights.anomalies },
        { heading: 'Recommendations', items: insights.recommendations },
      ].filter((s) => s.items && s.items.length > 0)
    : []);
</script>

<div class="page-header">
  <div>
    <div class="page-title">Ask</div>
    <div class="page-subtitle">Natural-language questions &amp; AI analysis of your log</div>
  </div>
  <button class="theme-btn" onclick={toggleTheme} aria-label="Toggle theme">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13.5A8 8 0 1 1 10.5 4a6.3 6.3 0 0 0 9.5 9.5Z"/></svg>
  </button>
</div>

{#if !hasApiKey}
  <div class="api-key-note">
    Add your OpenRouter API key in <a href="/settings">Settings</a> to use Ask and AI Insights.
  </div>
{/if}

<section class="ask-section">
  <div class="ask-box">
    <textarea
      class="ask-input"
      bind:value={question}
      placeholder="Ask anything about your log, e.g. “What was my average fatigue over the last 30 days?”"
      rows="2"
      onkeydown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); ask(); } }}
    ></textarea>
    <button class="btn-primary ask-btn" onclick={ask} disabled={asking || !question.trim() || !hasApiKey}>
      {asking ? 'Thinking…' : 'Ask'}
    </button>
  </div>

  <div class="examples">
    {#each EXAMPLES as ex}
      <button class="example-chip" onclick={() => useExample(ex)} disabled={asking || !hasApiKey}>{ex}</button>
    {/each}
  </div>

  {#if asking}
    <div class="ask-loading">
      <div class="skeleton-line lg"></div>
      <div class="skeleton-line md"></div>
    </div>
  {:else if askError}
    <div class="error-state">
      <p class="error-head">Couldn’t answer that.</p>
      <p class="error-detail">{askError}</p>
    </div>
  {:else if answer}
    <div class="answer-card">
      <p class="answer-text">{answer.answer}</p>
      <button class="working-toggle" onclick={() => (showWorking = !showWorking)}>
        {showWorking ? '▾' : '▸'} Show the working
      </button>
      {#if showWorking}
        <div class="working">
          {#if answer.explanation}<p class="working-explain">{answer.explanation}</p>{/if}
          <pre class="working-sql">{answer.sql}</pre>
          {#if answer.columns.length > 0}
            <div class="working-table-wrap">
              <table class="working-table">
                <thead>
                  <tr>{#each answer.columns as col}<th>{col}</th>{/each}</tr>
                </thead>
                <tbody>
                  {#each answer.rows as row}
                    <tr>{#each row as cell}<td>{cell}</td>{/each}</tr>
                  {/each}
                </tbody>
              </table>
              {#if answer.truncated}<p class="working-note">Showing the first 200 rows.</p>{/if}
              {#if answer.rows.length === 0}<p class="working-note">No rows returned.</p>{/if}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<section class="insights-section">
  <div class="insights-header">
    <div>
      <div class="card-heading">AI Insights</div>
      <div class="insights-sub">Analysis for {RANGE_LABELS[insightsRange].toLowerCase()}.</div>
    </div>
    <div class="insights-actions">
      <select class="range-select" bind:value={insightsRange} onchange={onRangeChange} disabled={insightsLoading}>
        {#each Object.entries(RANGE_LABELS) as [value, label]}
          <option {value}>{label}</option>
        {/each}
      </select>
      {#if !insightsGenerated && !insightsLoading}
        <button class="btn-primary" onclick={generateInsights} disabled={!hasApiKey}>Generate</button>
      {:else if insightsGenerated && !insightsLoading}
        <button class="btn-secondary" onclick={refreshInsights}>Refresh</button>
      {/if}
    </div>
  </div>

  {#if insightsLoading}
    <div class="insights-loading">
      <div class="skeleton-block summary"></div>
      <div class="skeleton-grid">
        {#each Array(3) as _}<div class="skeleton-block card"></div>{/each}
      </div>
    </div>
  {:else if insightsError}
    <div class="error-state">
      <p class="error-head">Couldn’t generate insights.</p>
      <p class="error-detail">{insightsError}</p>
      <button class="btn-secondary" onclick={generateInsights}>Retry</button>
    </div>
  {:else if insights}
    <div class="summary-card"><p>{insights.summary}</p></div>

    {#each allSections as section}
      <div class="insights-subheading">{section.heading}</div>
      <div class="insights-grid">
        {#each section.items as item}
          <div class="insight-card sev-{item.severity}">
            <span class="insight-icon">{item.icon}</span>
            <div class="insight-body">
              <div class="insight-title">{item.title}</div>
              <div class="insight-detail">{item.detail}</div>
            </div>
          </div>
        {/each}
      </div>
    {/each}

    <div class="generated-note">Last generated: {insights.generated_at}</div>
  {:else}
    <div class="insights-empty">
      <p>Generate an AI report to see patterns, anomalies and gentle pacing suggestions across your log.</p>
      <button class="btn-primary" onclick={generateInsights} disabled={!hasApiKey}>Generate insights</button>
    </div>
  {/if}
</section>

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:22px; }
  .page-title { font-family:'Source Serif 4',serif; font-size:30px; font-weight:600; color:var(--tp); letter-spacing:-.01em; }
  .page-subtitle { font-size:13.5px; color:var(--ts); margin-top:3px; }
  .theme-btn { width:36px;height:36px;border-radius:50%;border:1px solid var(--border);background:var(--card);color:var(--ts);display:flex;align-items:center;justify-content:center;cursor:pointer; }

  .api-key-note { margin-bottom:18px; padding:12px 15px; border-radius:12px; background:var(--amber-soft); border:1px solid var(--border); color:var(--amber-fg); font-size:13px; }
  .api-key-note a { color:var(--accent-fg); font-weight:700; }

  .btn-primary { background:var(--accent); color:#fff; border:none; border-radius:999px; padding:10px 20px; font-size:13px; font-weight:700; cursor:pointer; font-family:inherit; white-space:nowrap; }
  .btn-primary:disabled { opacity:.6; cursor:not-allowed; }
  .btn-secondary { background:var(--card); color:var(--tp); border:1px solid var(--border); border-radius:999px; padding:10px 18px; font-size:13px; font-weight:700; cursor:pointer; font-family:inherit; }

  /* Ask */
  .ask-section { margin-bottom:34px; max-width:860px; }
  .ask-box { display:flex; gap:10px; align-items:stretch; }
  .ask-input { flex:1; padding:12px 14px; border:1px solid var(--border); border-radius:14px; font-size:14px; font-family:inherit; background:var(--card); color:var(--tp); resize:vertical; box-shadow:var(--shadow); }
  .ask-btn { flex-shrink:0; align-self:stretch; }
  .examples { display:flex; flex-wrap:wrap; gap:8px; margin-top:12px; }
  .example-chip { font-size:12px; color:var(--ts); background:var(--card); border:1px solid var(--border); border-radius:999px; padding:7px 13px; cursor:pointer; font-family:inherit; }
  .example-chip:hover { background:var(--accent-soft); color:var(--accent-fg); }
  .example-chip:disabled { opacity:.5; cursor:not-allowed; }

  .ask-loading { display:flex; flex-direction:column; gap:10px; margin-top:22px; }
  .skeleton-line { background:var(--inset); border-radius:6px; height:16px; animation:pulse 1.5s infinite; }
  .skeleton-line.md { width:50%; }
  .skeleton-line.lg { width:85%; }
  @keyframes pulse { 0%,100% { opacity:1; } 50% { opacity:.45; } }

  .answer-card { margin-top:22px; background:var(--accent-soft); border:1px solid var(--border); border-radius:16px; padding:20px; }
  .answer-text { margin:0; font-size:15.5px; color:var(--tp); line-height:1.6; white-space:pre-wrap; }
  .working-toggle { margin-top:12px; background:none; border:none; color:var(--accent-fg); font-size:12.5px; font-weight:700; cursor:pointer; padding:0; }
  .working { margin-top:12px; border-top:1px solid var(--border); padding-top:12px; }
  .working-explain { margin:0 0 8px; font-size:12.5px; color:var(--ts); }
  .working-sql { background:var(--inset); border:1px solid var(--border); border-radius:10px; padding:10px 12px; font-size:12px; color:var(--tp); overflow-x:auto; white-space:pre-wrap; word-break:break-word; font-family:monospace; }
  .working-table-wrap { margin-top:12px; max-height:320px; overflow:auto; border:1px solid var(--border); border-radius:10px; }
  .working-table { width:100%; border-collapse:collapse; font-size:12px; }
  .working-table th { text-align:left; padding:7px 10px; background:var(--inset); border-bottom:1px solid var(--border); font-weight:700; color:var(--tp); position:sticky; top:0; }
  .working-table td { padding:6px 10px; border-bottom:1px solid var(--border); color:var(--tp); font-variant-numeric:tabular-nums; }
  .working-note { margin:8px 0 0; font-size:11.5px; color:var(--tm); }

  .error-state { margin-top:22px; padding:18px; color:var(--red-fg); background:var(--red-soft); border:1px solid var(--border); border-radius:14px; }
  .error-head { margin:0; font-weight:700; font-size:14px; }
  .error-detail { font-size:12px; color:var(--ts); margin:6px 0 0; word-break:break-all; }
  .error-state .btn-secondary { margin-top:12px; }

  /* Insights */
  .insights-section { margin-top:10px; padding-top:24px; border-top:1px solid var(--border); }
  .insights-header { display:flex; justify-content:space-between; align-items:flex-start; margin-bottom:16px; flex-wrap:wrap; gap:10px; }
  .card-heading { font-family:'Source Serif 4',serif; font-size:19px; font-weight:600; color:var(--tp); }
  .insights-sub { font-size:13px; color:var(--ts); margin-top:2px; }
  .insights-actions { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .range-select { padding:9px 12px; border:1px solid var(--border); border-radius:10px; font-size:13px; background:var(--card); color:var(--tp); cursor:pointer; font-family:inherit; }
  .range-select:disabled { opacity:.5; cursor:not-allowed; }

  .insights-loading { display:flex; flex-direction:column; gap:14px; }
  .skeleton-block { background:var(--inset); border-radius:14px; animation:pulse 1.5s infinite; }
  .skeleton-block.summary { height:64px; }
  .skeleton-block.card { height:92px; }
  .skeleton-grid { display:grid; grid-template-columns:repeat(auto-fill, minmax(280px, 1fr)); gap:14px; }

  .summary-card { background:var(--accent-soft); border:1px solid var(--border); border-radius:16px; padding:18px 20px; margin-bottom:20px; }
  .summary-card p { margin:0; font-size:15px; color:var(--tp); line-height:1.55; }

  .insights-subheading { font-size:13.5px; font-weight:700; color:var(--tp); margin:18px 0 10px; }
  .insights-grid { display:grid; grid-template-columns:repeat(auto-fill, minmax(280px, 1fr)); gap:14px; }

  .insight-card { background:var(--card); border:1px solid var(--border); border-radius:14px; padding:15px; display:flex; gap:11px; box-shadow:var(--shadow); border-left:4px solid var(--border); }
  .insight-card.sev-positive { border-left-color:var(--accent); }
  .insight-card.sev-warning { border-left-color:var(--amber); }
  .insight-card.sev-critical { border-left-color:var(--red); }
  .insight-icon { font-size:22px; flex-shrink:0; width:28px; text-align:center; }
  .insight-body { min-width:0; }
  .insight-title { font-weight:700; font-size:13.5px; color:var(--tp); margin-bottom:4px; }
  .insight-detail { font-size:12.5px; color:var(--ts); line-height:1.5; }

  .generated-note { margin-top:18px; font-size:11.5px; color:var(--tm); }
  .insights-empty { border:2px dashed var(--border); border-radius:16px; padding:36px 28px; text-align:center; color:var(--ts); font-size:13.5px; }
  .insights-empty .btn-primary { margin-top:14px; }
</style>
