import { Chart, registerables } from 'chart.js';
import { invoke } from '@tauri-apps/api/core';
import { resolveCSSVar, resolveDeep } from '$lib/chartTheme';

Chart.register(...registerables);

export interface ChartExportSpec {
  type: string;
  labels: string[];
  datasets: any[];
  options?: Record<string, any>;
  /** Drawn above the chart so the file makes sense on its own. */
  title: string;
  subtitle?: string;
  /** Logical size of the chart area; the file is this x `scale` pixels. */
  width?: number;
  height?: number;
  scale?: number;
}

/** Render the chart again, offscreen and much larger than the page shows it, then
 *  return the PNG bytes.
 *
 *  It is a second Chart.js instance rather than a grab of the live canvas because
 *  the live one is only ~220 CSS px tall — screenshotting it gives an image too
 *  small to read. The offscreen container is real DOM (parked off-viewport) so
 *  Chart.js's responsive sizing measures it normally; animation is off so the
 *  first frame is the finished chart. */
export async function renderChartPng(spec: ChartExportSpec): Promise<Uint8Array> {
  const width = spec.width ?? 1400;
  const height = spec.height ?? 620;
  const scale = spec.scale ?? 2;
  const pad = 32;
  const headerH = spec.subtitle ? 78 : 56;

  const bg = resolveCSSVar('var(--card)') || '#ffffff';
  const titleColor = resolveCSSVar('var(--tp)');
  const subtitleColor = resolveCSSVar('var(--ts)');

  const holder = document.createElement('div');
  holder.style.cssText =
    `position:fixed;left:-20000px;top:0;width:${width}px;height:${height}px;background:${bg};`;
  const canvas = document.createElement('canvas');
  holder.appendChild(canvas);
  document.body.appendChild(holder);

  let chart: Chart | null = null;
  try {
    chart = new Chart(canvas, {
      type: spec.type as any,
      data: { labels: spec.labels, datasets: resolveDeep(spec.datasets) },
      options: resolveDeep({
        ...(spec.options ?? {}),
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
        devicePixelRatio: scale,
      }),
    });
    chart.update('none');

    const sheetW = width + pad * 2;
    const sheetH = height + headerH + pad * 2;
    const out = document.createElement('canvas');
    out.width = sheetW * scale;
    out.height = sheetH * scale;
    const ctx = out.getContext('2d');
    if (!ctx) throw new Error('Could not get a 2D drawing context');
    // Everything below is drawn in the same logical units as `holder`, so the
    // chart lands pixel-for-pixel when scaled back up.
    ctx.scale(scale, scale);

    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, sheetW, sheetH);

    ctx.textBaseline = 'top';
    ctx.fillStyle = titleColor;
    ctx.font = '700 26px system-ui, -apple-system, "Segoe UI", sans-serif';
    ctx.fillText(spec.title, pad, pad);
    if (spec.subtitle) {
      ctx.fillStyle = subtitleColor;
      ctx.font = '400 15px system-ui, -apple-system, "Segoe UI", sans-serif';
      ctx.fillText(spec.subtitle, pad, pad + 36);
    }

    ctx.drawImage(canvas, pad, pad + headerH, width, height);

    const blob: Blob = await new Promise((resolve, reject) =>
      out.toBlob((b) => (b ? resolve(b) : reject(new Error('Could not encode the PNG'))), 'image/png')
    );
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    chart?.destroy();
    holder.remove();
  }
}

/** Render, save next to the other exports, and open in the default image viewer.
 *  Returns the saved path. */
export async function exportChartPng(spec: ChartExportSpec, fileStem: string): Promise<string> {
  const bytes = await renderChartPng(spec);
  return invoke<string>('save_chart_png', { bytes: Array.from(bytes), fileStem });
}
