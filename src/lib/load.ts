// Raw per-category activity load for a day — the figure shown on the Activity
// page (and, for recognisability, the PEM page's contribution bars). This is the
// pre-model load: duration × category energy weight × energy-cost factor.
//   Physical          = Physical / Active + Domestic
//   Cognitive         = Cognitive / Active + Hobby / Creative
//   Sensory / social  = everything else (Social, Screen / Sedentary)
// The PEM *risk* uses its own scaled/blended version of these in commands/pem.rs.

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
    const weight = entry.energy_cost === 'Low' ? 0.7 : entry.energy_cost === 'High' ? 2.0 : 1.0;
    const v = entry.duration_hours * (cat.energy_weight ?? 1) * weight;
    const name = (cat.name ?? '').toLowerCase();
    if (name.includes('physical') || name.includes('domestic') || name === 'active') phys += v;
    else if (name.includes('cognitive') || name.includes('hobby')) cog += v;
    else sens += v;
  }
  return { phys, cog, sens, total: phys + cog + sens };
}
