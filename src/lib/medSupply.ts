// Medication supply ("x days left"): one supply per tablet strength / script.
//
// Everything is worked out in Rust (`supply_views` in commands/medications.rs, from the
// doses actually logged) and arrives as a `Supply` from `list_medication_supplies`; this
// file only labels and flags it.

import { todayISO } from '$lib/formatDate';

export interface Supply {
  id: number;
  medication_id: number;
  strength: number | null;
  label: string | null;
  box_size: number | null;
  boxes_per_fill: number;
  warn_days: number;
  repeats_left: number | null;
  needs_script: boolean;
  script_expiry: string | null;
  in_use_since: string;
  on_hand: number;
  per_day: number | null;
  /** 'doses' (recent logged doses) or 'schedule' (the quick-add buttons, while history is short). */
  usage_source: 'doses' | 'schedule' | null;
  usage_days: number;
  days_left: number | null;
  needs_strength: boolean;
}

/** How long before a script's expiry it's flagged. */
export const SCRIPT_WARN_DAYS = 30;

function daysBetween(from: string, to: string): number {
  const a = Date.UTC(+from.slice(0, 4), +from.slice(5, 7) - 1, +from.slice(8, 10));
  const b = Date.UTC(+to.slice(0, 4), +to.slice(5, 7) - 1, +to.slice(8, 10));
  return Math.round((b - a) / 86400000);
}

/** "8 mg", "8 mg · Brand X", or the label alone; '' when it has neither. */
export function supplyName(s: Supply, med: any): string {
  const strength = s.strength != null ? `${s.strength} ${med?.dose_unit || 'mg'}` : '';
  return [strength, s.label ?? ''].filter(Boolean).join(' · ');
}

/** 'out' | 'low' (within the warning window) | 'ok', for colouring the badge. */
export function supplyLevel(s: Supply): 'out' | 'low' | 'ok' {
  if (s.days_left == null) return s.on_hand <= 0 ? 'out' : 'ok';
  if (s.days_left <= 0) return 'out';
  return s.days_left <= s.warn_days ? 'low' : 'ok';
}

/** Days until the script expires (negative once it has), or null when no expiry is set. */
export function scriptDaysLeft(s: Supply): number | null {
  return s.script_expiry ? daysBetween(todayISO(), s.script_expiry) : null;
}

export interface SupplyAlert {
  med: any;
  supply: Supply;
  level: 'out' | 'low';
  text: string;
}

/**
 * What needs doing for one supply of an active med: running out, a new script needed, or
 * the script expiring soon. The Medication page badges and the Dashboard banner both use it.
 */
export function supplyAlerts(med: any, s: Supply): SupplyAlert[] {
  if (!med?.active) return [];
  const out: SupplyAlert[] = [];
  const lvl = supplyLevel(s);
  if (lvl !== 'ok') {
    const d = s.days_left;
    out.push({ med, supply: s, level: lvl, text: d == null || d <= 0 ? 'run out' : `${d} day${d === 1 ? '' : 's'} left` });
  }
  // Flagged early it's a reminder; with the supply also low it's urgent.
  if (s.needs_script) out.push({ med, supply: s, level: lvl !== 'ok' ? 'out' : 'low', text: 'new script needed' });
  const e = scriptDaysLeft(s);
  if (e != null && !s.needs_script && e <= SCRIPT_WARN_DAYS) {
    out.push({
      med, supply: s, level: e <= 7 ? 'out' : 'low',
      text: e < 0 ? 'script expired' : e === 0 ? 'script expires today' : `script expires in ${e} day${e === 1 ? '' : 's'}`,
    });
  }
  return out;
}

/** Tablets added by a usual fill: boxes x box size, or null without a box size. */
export function usualFill(s: Supply): number | null {
  return s.box_size ? s.box_size * (s.boxes_per_fill ?? 1) : null;
}

/** Round to one decimal for display ("—" for none). */
export function fmtCount(n: number | null | undefined): string {
  return n == null ? '—' : String(Math.round(n * 10) / 10);
}
