<script lang="ts">
  import { Chart, registerables } from 'chart.js';
  import { theme } from '$lib/stores/theme.svelte';

  Chart.register(...registerables);

  // datasets is `any[]` because callers pass a range of Chart.js dataset shapes —
  // plain line/bar series, floating bars ([low, high] data), and scatter-style point
  // datasets — not just the simple {label, data} form.
  let { type = 'line', datasets, labels, options = {}, chartArea = '' } = $props<{
    type?: string;
    datasets: any[];
    labels: string[];
    options?: Record<string, any>;
    chartArea?: string;
  }>();

  let canvas: HTMLCanvasElement;
  let chartInstance: Chart | null = $state(null);

  function resolveCSSVar(v: string): string {
    if (!v.startsWith('var(--')) return v;
    const name = v.slice(4, -1);
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || v;
  }

  // Chart.js draws to a canvas and can't read CSS variables, so every `var(--x)`
  // anywhere in the config — dataset colours AND caller-supplied axis/grid/legend
  // colours in `options` — must be resolved to a concrete colour first. A shallow
  // resolve (colours only on datasets) left axis labels and gridlines unstyled, so
  // they fell back to Chart.js's grey and looked wrong in dark mode.
  function resolveDeep(x: any): any {
    if (typeof x === 'string') return resolveCSSVar(x);
    if (Array.isArray(x)) return x.map(resolveDeep);
    if (x && typeof x === 'object') {
      const out: Record<string, any> = {};
      for (const k in x) out[k] = resolveDeep(x[k]);
      return out;
    }
    return x; // numbers, null, functions (tooltip callbacks) pass through untouched
  }

  $effect(() => {
    if (!canvas) return;
    // Re-run when the theme flips so the canvas colours follow the CSS variables
    // (getComputedStyle below then returns the new light/dark values).
    theme.dark;

    const resolvedDatasets = resolveDeep(datasets);
    const resolvedOptions = resolveDeep({
      responsive: true,
      maintainAspectRatio: false,
      plugins: { legend: { display: false } },
      scales: {
        x: { grid: { display: false }, ticks: { color: 'var(--tm)', font: { size: 10 } } },
        y: { grid: { color: 'var(--border)' }, ticks: { color: 'var(--ts)', font: { size: 11 } } },
      },
      ...options,
    });

    // Chart.js can't change a chart's base type on update, so if `type` flips
    // (e.g. switching a cardio metric from a bar range back to a line), tear the
    // instance down and rebuild rather than reusing a stale bar/line chart.
    if (chartInstance && (chartInstance.config as any).type !== type) {
      chartInstance.destroy();
      chartInstance = null;
    }
    if (chartInstance) {
      chartInstance.data.labels = labels;
      chartInstance.data.datasets = resolvedDatasets;
      chartInstance.options = resolvedOptions;
      chartInstance.update('none');
    } else {
      chartInstance = new Chart(canvas, { type, data: { labels, datasets: resolvedDatasets }, options: resolvedOptions });
    }
  });

  $effect(() => {
    return () => {
      if (chartInstance) { chartInstance.destroy(); chartInstance = null; }
    };
  });
</script>

<div class="chart-wrap" style={chartArea ? `height:${chartArea}` : ''}>
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .chart-wrap { width: 100%; position: relative; }
</style>