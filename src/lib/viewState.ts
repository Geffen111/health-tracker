// Remembered view settings — a chart's range, metric picks, which tab is open.
//
// A page's `$state` dies with the component, so leaving a page and coming back used to
// reset every chart to its defaults. Each page reads its last settings with
// `recallView()` when it initialises and writes them back from an `$effect` with
// `rememberView()`. localStorage (like the sidebar's collapsed flag) keeps them across
// launches too. Only view choices belong here — never data, never anything that must
// sync between PCs.

const PREFIX = 'view:';

/** The saved settings for `page`, or `{}` when there are none (or they're unreadable). */
export function recallView<T extends object>(page: string): Partial<T> {
  try {
    const raw = localStorage.getItem(PREFIX + page);
    const parsed = raw ? JSON.parse(raw) : null;
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch {
    return {};
  }
}

export function rememberView(page: string, settings: object) {
  try {
    localStorage.setItem(PREFIX + page, JSON.stringify(settings));
  } catch {}
}

/** `value` when it's one of `allowed`, else `fallback` — guards against a stale saved option. */
export function oneOf<T>(value: unknown, allowed: readonly T[], fallback: T): T {
  return allowed.includes(value as T) ? (value as T) : fallback;
}
