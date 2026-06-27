const MONTHS_SHORT = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const DAYS_SHORT = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

function parseDate(dateStr: string): Date {
  return new Date(dateStr + 'T00:00:00');
}

export function formatDate(dateStr: string | null | undefined): string {
  if (!dateStr) return '—';
  const d = parseDate(dateStr);
  const dd = String(d.getDate()).padStart(2, '0');
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const yy = String(d.getFullYear()).slice(2);
  return `${dd}/${mm}/${yy}`;
}

export function formatDateLong(dateStr: string | null | undefined): string {
  if (!dateStr) return '—';
  const d = parseDate(dateStr);
  const dow = DAYS_SHORT[d.getDay()];
  const dd = String(d.getDate()).padStart(2, '0');
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const yy = String(d.getFullYear()).slice(2);
  return `${dow} ${dd}/${mm}/${yy}`;
}

export function formatTime(timeStr: string | null | undefined): string {
  if (!timeStr) return '—';
  return timeStr;
}

export function formatDateShort(dateStr: string | null | undefined): string {
  if (!dateStr) return '';
  const d = parseDate(dateStr);
  const dd = String(d.getDate()).padStart(2, '0');
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  return `${dd}/${mm}`;
}

/** Format a Date to YYYY-MM-DD using LOCAL date parts (no UTC round-trip). */
export function formatISODate(date: Date): string {
  const yy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, '0');
  const dd = String(date.getDate()).padStart(2, '0');
  return `${yy}-${mm}-${dd}`;
}

/** Today's date as YYYY-MM-DD in the user's local timezone. */
export function todayISO(): string {
  return formatISODate(new Date());
}

/**
 * Shift a YYYY-MM-DD string by `delta` days, staying in local time.
 * Avoids the toISOString() UTC round-trip that silently broke "next day"
 * for users east of UTC (e.g. AEST), where it could land on the same date.
 */
export function shiftISO(dateStr: string, delta: number): string {
  const [y, m, d] = dateStr.split('-').map(Number);
  const dt = new Date(y, m - 1, d);
  dt.setDate(dt.getDate() + delta);
  return formatISODate(dt);
}

/** Weekday index for a YYYY-MM-DD string (0=Sun … 6=Sat), in local time. */
export function weekdayIndex(dateStr: string): number {
  const [y, m, d] = dateStr.split('-').map(Number);
  return new Date(y, m - 1, d).getDay();
}
