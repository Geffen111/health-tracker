// Weekly-summary state shared by the layout (which generates it on launch), the sidebar
// dot, the Dashboard banner and the /weekly page. Generation takes a while (one model
// call), so it runs in the background and the banner appears when it lands.

import { invoke } from '@tauri-apps/api/core';

export interface WeeklyBanner {
  week_start: string;
  week_end: string;
  seen: boolean;
}

export const weekly = $state<{ banner: WeeklyBanner | null; generating: boolean; error: string | null }>({
  banner: null,
  generating: false,
  error: null,
});

export async function refreshWeeklyBanner() {
  try {
    weekly.banner = await invoke<WeeklyBanner | null>('get_weekly_banner');
  } catch (e) {
    console.warn('Weekly banner failed:', e);
  }
}

/** Launch hook: show whatever exists straight away, then make last week's if it's missing. */
export async function ensureWeeklySummary() {
  await refreshWeeklyBanner();
  weekly.generating = true;
  weekly.error = null;
  try {
    weekly.banner = await invoke<WeeklyBanner | null>('ensure_weekly_summary');
  } catch (e) {
    // Silent on screen (like the launch import); the /weekly page offers a retry.
    weekly.error = String(e);
    console.warn('Weekly summary failed:', e);
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
