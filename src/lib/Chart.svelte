<script lang="ts">
  import { Chart, registerables } from 'chart.js';

  Chart.register(...registerables);

  let { type = 'line', datasets, labels, options = {}, chartArea = '' } = $props<{
    type?: string;
    datasets: { label: string; data: (number | null)[]; borderColor?: string; backgroundColor?: string; yAxisID?: string }[];
    labels: string[];
    options?: Record<string, any>;
    chartArea?: string;
  }>();

  let canvas: HTMLCanvasElement;
  let chartInstance: Chart | null = $state(null);

  function resolveCSSVar(v: string | undefined): string | undefined {
    if (!v || !v.startsWith('var(--')) return v;
    const name = v.slice(4, -1);
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || v;
  }

  $effect(() => {
    if (!canvas) return;
    const resolved = datasets.map((ds: { label: string; data: (number | null)[]; borderColor?: string; backgroundColor?: string }) => ({
      ...ds,
      borderColor: resolveCSSVar(ds.borderColor),
      backgroundColor: resolveCSSVar(ds.backgroundColor),
    }));
    const cfg = {
      type,
      data: { labels, datasets: resolved },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { grid: { display: false }, ticks: { color: resolveCSSVar('var(--tm)'), font: { size: 10 } } },
          y: { grid: { color: resolveCSSVar('var(--border)') }, ticks: { color: resolveCSSVar('var(--ts)'), font: { size: 11 } } },
        },
        ...options,
      },
    };
    if (chartInstance) {
      chartInstance.data.labels = labels;
      chartInstance.data.datasets = resolved as any;
      chartInstance.update('none');
    } else {
      chartInstance = new Chart(canvas, cfg);
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