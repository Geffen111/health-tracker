import { goto } from '$app/navigation';
import { todayISO } from './formatDate';

const ISO = /^\d{4}-\d{2}-\d{2}$/;

/**
 * The day a date-scoped page should open on.
 *
 * Every page that has day arrows carries its day in `?date=`, so a link from one
 * page to another lands on the same day. Without this each page held its own
 * `selectedDate` defaulting to today, and jumping from a daily log parked on the
 * 24th to the Medication page silently logged the dose against today.
 *
 * Falls back to today for a missing, malformed or future date — the arrows stop
 * at today, so a URL past it would leave the page in a state you can't navigate
 * back out of.
 */
export function dateFromUrl(url: URL): string {
  const d = url.searchParams.get('date');
  const today = todayISO();
  if (!d || !ISO.test(d) || d > today) return today;
  return d;
}

/**
 * Reflect the day back into the URL so it survives a jump to another page.
 * `replaceState` keeps the day arrows out of the back stack — stepping back a
 * week shouldn't take seven Back presses to undo — and `keepFocus` stops the
 * field you're typing in from losing focus mid-edit.
 */
export function pushDate(date: string) {
  const url = new URL(window.location.href);
  url.searchParams.set('date', date);
  void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
}

/** Link to another date-scoped page on the same day. */
export function dateHref(path: string, date: string): string {
  return `${path}?date=${date}`;
}
