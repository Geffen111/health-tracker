// Medication supply counter ("x days left"), opt-in per medication.
//
// On hand = the count recorded on `supply_date`, less `supply_per_day` for each day since,
// never below zero. `on_hand_at` in commands/medications.rs is the same sum (it applies a
// fill on top of it) — keep the two in step.

import { todayISO } from '$lib/formatDate';

function daysBetween(from: string, to: string): number {
  const a = Date.UTC(+from.slice(0, 4), +from.slice(5, 7) - 1, +from.slice(8, 10));
  const b = Date.UTC(+to.slice(0, 4), +to.slice(5, 7) - 1, +to.slice(8, 10));
  return Math.round((b - a) / 86400000);
}

/** Tablets on hand on `day` (default today), or null when no count has been recorded. */
export function onHandAt(med: any, day: string = todayISO()): number | null {
  if (med?.supply_count == null) return null;
  const perDay = med.supply_per_day ?? 0;
  const days = med.supply_date ? Math.max(0, daysBetween(med.supply_date, day)) : 0;
  return Math.max(0, med.supply_count - perDay * days);
}

/** Whole days the supply lasts from today, or null when it can't be worked out. */
export function daysLeft(med: any): number | null {
  const left = onHandAt(med);
  if (left == null || !med.supply_per_day) return null;
  return Math.floor(left / med.supply_per_day + 1e-9);
}

/** 'out' | 'low' (within the warning window) | 'ok', for colouring the badge. */
export function supplyLevel(med: any): 'out' | 'low' | 'ok' {
  const d = daysLeft(med);
  if (d == null) return 'ok';
  if (d <= 0) return 'out';
  return d <= (med.supply_warn_days ?? 7) ? 'low' : 'ok';
}

/** Tablets added by a usual fill: boxes x box size, or null without a box size. */
export function usualFill(med: any): number | null {
  return med?.supply_box_size ? med.supply_box_size * (med.supply_boxes_per_fill ?? 1) : null;
}
