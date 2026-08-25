// Chart.js draws to a canvas and can't read CSS variables, so every `var(--x)`
// anywhere in a chart config — dataset colours AND axis/grid/legend colours in
// `options` — must be resolved to a concrete colour before it reaches Chart.js.
// Shared by `Chart.svelte` (on-screen) and `chartExport.ts` (offscreen PNG), so
// an exported image is coloured exactly like the one on the page.

export function resolveCSSVar(v: string): string {
  if (!v.startsWith('var(--')) return v;
  const name = v.slice(4, -1);
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || v;
}

/** Walk a config replacing every `var(--x)` string; numbers, null and functions
 *  (tooltip callbacks) pass through untouched. */
export function resolveDeep<T>(x: T): T {
  if (typeof x === 'string') return resolveCSSVar(x) as unknown as T;
  if (Array.isArray(x)) return x.map(resolveDeep) as unknown as T;
  if (x && typeof x === 'object') {
    const out: Record<string, any> = {};
    for (const k in x) out[k] = resolveDeep((x as Record<string, any>)[k]);
    return out as T;
  }
  return x;
}
