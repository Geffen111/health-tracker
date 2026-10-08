// Shared types and helpers for the Food page and its panels.

export interface Food {
  id: number;
  name: string;
  kind: 'food' | 'drink';
  regular: boolean;
  active: boolean;
  days_logged: number;
  times_logged: number;
  last_logged: string | null;
  category_id: number | null;
  /** null = not yet tagged, 'ai' = tagged by the model, 'user' = set by hand. */
  tag_source: 'ai' | 'user' | null;
  flag_ids: number[];
}

/** A category or flag, with how many items carry it. */
export interface FoodTag { id: number; name: string; items: number; }

export interface FoodLogEntry {
  id: number;
  log_date: string;
  time_taken: string | null;
  food_id: number;
  food_name: string;
  kind: 'food' | 'drink';
  amount: string | null;
  group_id: number | null;
  group_name: string | null;
  created_at: string;
}

export interface CleanupSuggestion {
  action: 'merge' | 'rename' | 'split';
  food_ids: number[];
  names: string[];
  reason: string;
}

/** A set of items had together, and how often that exact set turns up. */
export interface Meal {
  key: string;
  food_ids: number[];
  names: string[];
  count: number;
  /** Most recent occasion, and its time when it had one. */
  last: string;
  time: string | null;
  /** True when every occasion was a group being logged — already one tap away. */
  fromGroup: boolean;
}

/**
 * The meals in a stretch of log: entries on the same day at the same time, or (with no
 * time) saved in the same action, form one occasion; an occasion of two or more items is
 * a meal. Identical sets of items are counted together. Newest first.
 */
export function findMeals(entries: FoodLogEntry[]): Meal[] {
  const occasions = new Map<string, FoodLogEntry[]>();
  for (const e of entries) {
    const key = e.time_taken ? `${e.log_date} ${e.time_taken}` : `${e.log_date} @${e.created_at}`;
    (occasions.get(key) ?? occasions.set(key, []).get(key)!).push(e);
  }
  const meals = new Map<string, Meal>();
  for (const occ of occasions.values()) {
    const ids = [...new Set(occ.map((e) => e.food_id))].sort((a, b) => a - b);
    if (ids.length < 2) continue;
    const key = ids.join(',');
    const date = occ[0].log_date;
    const grouped = occ.every((e) => e.group_id != null && e.group_id === occ[0].group_id);
    const m = meals.get(key) ?? {
      key,
      food_ids: ids,
      names: ids.map((id) => occ.find((e) => e.food_id === id)!.food_name),
      count: 0,
      last: '',
      time: null,
      fromGroup: true,
    };
    m.count++;
    m.fromGroup &&= grouped;
    if (date > m.last) { m.last = date; m.time = occ[0].time_taken; }
    meals.set(key, m);
  }
  return [...meals.values()].sort((a, b) => b.last.localeCompare(a.last) || b.count - a.count);
}

/** A name for a suggested group, from the time it's usually had. */
export function mealName(time: string | null): string {
  const h = time ? parseInt(time.slice(0, 2), 10) : NaN;
  if (Number.isNaN(h)) return 'Usual meal';
  if (h < 11) return 'Usual breakfast';
  if (h < 15) return 'Usual lunch';
  if (h < 17) return 'Afternoon snack';
  return 'Usual dinner';
}
