// Blood pressure series for the Cardio chart. The calibration adjustment itself lives in
// one place, the backend (`load_bp_series` in commands/blood_pressure.rs), so the chart
// and the weekly summary always agree; this file only has its types and the daily mean.

export interface BpReading {
  log_date: string;
  time_taken: string | null;
  systolic: number;
  diastolic: number;
  source: string | null;
}

/** One calibration period and what was added to its watch readings (mmHg). */
export interface BpPeriod {
  /** Date of the calibration that opened the period; null before the first one. */
  from: string | null;
  sys: number;
  dia: number;
  /** Days with watch readings in this period. */
  days: number;
}

/** `get_bp_series`: every reading as measured, the same with watch ones adjusted, and the shifts. */
export interface BpSeries {
  raw: BpReading[];
  adjusted: BpReading[];
  periods: BpPeriod[];
}

export interface BpDaily {
  log_date: string;
  avg_systolic: number;
  avg_diastolic: number;
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;

/** Mean systolic / diastolic per day, oldest first. */
export function dailyAverages(readings: BpReading[]): BpDaily[] {
  const byDate = new Map<string, BpReading[]>();
  for (const r of readings) (byDate.get(r.log_date) ?? byDate.set(r.log_date, []).get(r.log_date)!).push(r);
  return [...byDate.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([log_date, rs]) => ({
      log_date,
      avg_systolic: mean(rs.map((r) => r.systolic)),
      avg_diastolic: mean(rs.map((r) => r.diastolic)),
    }));
}
