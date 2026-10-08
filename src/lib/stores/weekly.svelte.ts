// Weekly-summary state shared by the layout (which checks on launch whether last week
// still needs one), the sidebar dot, the Dashboard banner and the /weekly page.
// Nothing is written automatically: on a Monday Sunday's entries are often still being
// filled in, so `pending` is offered and the person runs it when the week is complete.
// Generation takes a while (one model call), so it runs in the background.

import { invoke } from '@tauri-apps/api/core';

export interface WeeklyBanner {
  week_start: string;
  week_end: string;
  seen: boolean;
}

export const weekly = $state<{
  banner: WeeklyBanner | null;
  /** Last full week, when it has no summary yet and one can be written. */
  pending: WeeklyBanner | null;
  generating: boolean;
  error: string | null;
}>({
  banner: null,
  pending: null,
  generating: false,
  error: null,
});

export async function refreshWeeklyBanner() {
  try {
    [weekly.banner, weekly.pending] = await Promise.all([
      invoke<WeeklyBanner | null>('get_weekly_banner'),
      invoke<WeeklyBanner | null>('get_pending_weekly'),
    ]);
  } catch (e) {
    console.warn('Weekly banner failed:', e);
  }
}

/** Write the pending week's summary — run from the Dashboard banner or the /weekly page. */
export async function runPendingWeekly(): Promise<boolean> {
  const week = weekly.pending;
  if (!week || weekly.generating) return false;
  weekly.generating = true;
  weekly.error = null;
  try {
    await invoke('generate_weekly_summary', { weekStart: week.week_start });
    await refreshWeeklyBanner();
    return true;
  } catch (e) {
    weekly.error = String(e);
    return false;
  } finally {
    weekly.generating = false;
  }
}

export async function markWeeklySeen(weekStart: string) {
  try {
    await invoke('mark_weekly_seen', { weekStart });
    if (weekly.banner?.week_start === weekStart) weekly.banner = { ...weekly.banner, seen: true };
  } catch (e) {
    console.warn('Mark seen failed:', e);
  }
}
