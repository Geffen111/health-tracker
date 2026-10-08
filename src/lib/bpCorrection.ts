// Calibration adjustment for the watch's blood pressure readings.
//
// The watch doesn't measure BP absolutely: it is calibrated against a cuff, and the cuff
// is itself only accurate to a few mmHg. So each calibration moves every watch reading
// after it up or down by a roughly constant amount — a step in the chart that is the
// cuff's error on that day, not a change in BP.
//
// This removes those steps (a level-shift / "homogenisation" adjustment):
//  1. Readings are split into periods at each calibration (date + time).
//  2. At each calibration, the step is the median of the first STEP_WINDOW days after it
//     minus the median of the last STEP_WINDOW days before it (already adjusted). Medians,
//     so one odd day doesn't set the step. With fewer than MIN_DAYS either side there's no
//     estimate and the period keeps the previous offset.
//  3. Each period's offset is the running total of the steps. All offsets are then shifted
//     so the adjusted readings keep the same overall mean as the raw ones: no single
//     calibration is treated as the true one — the level is the average of all of them.
//
// Trends *within* a period are untouched, so a gradual real change survives; a genuine
// change that happens exactly at a calibration would be removed with the step.
// Only `source = 'watch'` readings are adjusted. A cuff reading is a measurement in its
// own right and stays as taken.

export interface BpReading {
  log_date: string;
  time_taken: string | null;
  systolic: number;
  diastolic: number;
  source: string | null;
}

export interface Calibration {
  cal_date: string;
  cal_time: string | null;
}

/** One calibration period and what was added to its watch readings (mmHg). */
export interface BpPeriod {
  /** The calibration that opened the period; null for readings before the first one. */
  from: string | null;
  sys: number;
  dia: number;
  /** Days with watch readings in this period. */
  days: number;
}

export interface BpDaily {
  log_date: string;
  avg_systolic: number;
  avg_diastolic: number;
}

const STEP_WINDOW = 7;
const MIN_DAYS = 3;

const WATCH = 'watch';
const stamp = (date: string, time: string | null) => `${date} ${time ?? '00:00'}`;

function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}
function mean(xs: number[]): number {
  return xs.reduce((a, b) => a + b, 0) / xs.length;
}

/** The adjusted readings (watch ones shifted, others as taken) and the per-period shifts. */
export function adjustForCalibration(
  readings: BpReading[],
  calibrations: Calibration[],
): { readings: BpReading[]; periods: BpPeriod[] } {
  const cuts = calibrations.map((c) => stamp(c.cal_date, c.cal_time)).sort();
  const periodOf = (r: BpReading) => {
    const t = stamp(r.log_date, r.time_taken);
    let p = 0;
    while (p < cuts.length && t >= cuts[p]) p++;
    return p;
  };

  // Daily means of the watch readings, per period (a calibration day can sit in two).
  const watch = readings.filter((r) => r.source === WATCH);
  const byDay = new Map<string, { period: number; date: string; sys: number[]; dia: number[] }>();
  for (const r of watch) {
    const period = periodOf(r);
    const key = `${period}|${r.log_date}`;
    const d = byDay.get(key) ?? { period, date: r.log_date, sys: [], dia: [] };
    d.sys.push(r.systolic);
    d.dia.push(r.diastolic);
    byDay.set(key, d);
  }
  const days = [...byDay.values()]
    .map((d) => ({ period: d.period, date: d.date, sys: mean(d.sys), dia: mean(d.dia) }))
    .sort((a, b) => a.period - b.period || a.date.localeCompare(b.date));

  // Offsets are what the period reads above the first one; adjusted = raw - offset.
  const offSys = [0], offDia = [0];
  for (let p = 1; p <= cuts.length; p++) {
    const before = days.filter((d) => d.period < p).slice(-STEP_WINDOW);
    const after = days.filter((d) => d.period === p).slice(0, STEP_WINDOW);
    if (before.length >= MIN_DAYS && after.length >= MIN_DAYS) {
      offSys[p] = median(after.map((d) => d.sys)) - median(before.map((d) => d.sys - offSys[d.period]));
      offDia[p] = median(after.map((d) => d.dia)) - median(before.map((d) => d.dia - offDia[d.period]));
    } else {
      offSys[p] = offSys[p - 1];
      offDia[p] = offDia[p - 1];
    }
  }

  // Keep the overall mean: centre the offsets on their reading-weighted average.
  const counts = new Array(cuts.length + 1).fill(0);
  for (const r of watch) counts[periodOf(r)]++;
  const n = watch.length || 1;
  const centreSys = offSys.reduce((a, o, p) => a + o * counts[p], 0) / n;
  const centreDia = offDia.reduce((a, o, p) => a + o * counts[p], 0) / n;
  const addSys = offSys.map((o) => centreSys - o);
  const addDia = offDia.map((o) => centreDia - o);

  const periods: BpPeriod[] = addSys.map((sys, p) => ({
    from: p === 0 ? null : cuts[p - 1].slice(0, 10),
    sys,
    dia: addDia[p],
    days: days.filter((d) => d.period === p).length,
  }));

  return {
    readings: readings.map((r) => {
      if (r.source !== WATCH) return r;
      const p = periodOf(r);
      return { ...r, systolic: r.systolic + addSys[p], diastolic: r.diastolic + addDia[p] };
    }),
    periods,
  };
}

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
