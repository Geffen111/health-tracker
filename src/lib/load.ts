// Per-category activity load for a day: duration × category energy weight ×
// energy-cost factor.
//   Physical          = Physical / Active + Domestic
//   Cognitive         = Cognitive / Active + Hobby / Creative
//   Sensory / social  = everything else (Social, Screen / Sedentary)
// Kept in step with `LOAD_EXPR`/`BUCKET_EXPR` in src-tauri/src/commands/pacing.rs so a
// day reads the same on the Activity page as it does in the Pacing charts.

export interface DayLoad {
  phys: number;
  cog: number;
  sens: number;
  total: number;
}

export function computeDayLoad(entries: any[], activityTypes: any[], categories: any[]): DayLoad {
  let phys = 0, cog = 0, sens = 0;
  for (const entry of entries) {
    const type = activityTypes.find((t: any) => t.id === entry.activity_type_id);
    if (!type) continue;
    const cat = categories.find((c: any) => c.id === type.category_id);
    if (!cat) continue;
    // Older rows predate the auto-filled energy cost, so fall back to the type's default.
    const cost = entry.energy_cost ?? type.default_energy_cost;
    const weight = cost === 'Low' ? 0.7 : cost === 'High' ? 2.0 : 1.0;
    const v = entry.duration_hours * (cat.energy_weight ?? 1) * weight;
    const name = (cat.name ?? '').toLowerCase();
    if (name.includes('physical') || name.includes('domestic') || name === 'active') phys += v;
    else if (name.includes('cognitive') || name.includes('hobby')) cog += v;
    else sens += v;
  }
  return { phys, cog, sens, total: phys + cog + sens };
}
