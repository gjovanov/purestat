// The dashboard's periods and their comparison. Every date is a UTC day as
// `YYYY-MM-DD`, the way the API counts days, and a period includes both ends.

export interface Period {
  from: string
  to: string
}

/** The presets of the date picker, by the number of days each covers. */
export const PRESET_DAYS: Record<string, number> = {
  '1d': 1,
  '7d': 7,
  '30d': 30,
  '6mo': 182,
  '12mo': 365,
}

const DAY_MS = 86_400_000

function parse(date: string): number {
  return Date.parse(`${date}T00:00:00Z`)
}

function format(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10)
}

/** Today in UTC. */
export function utcToday(now: Date = new Date()): string {
  return now.toISOString().slice(0, 10)
}

export function addDays(date: string, days: number): string {
  return format(parse(date) + days * DAY_MS)
}

/** How many days a period covers, both ends included. */
export function lengthInDays(period: Period): number {
  return Math.round((parse(period.to) - parse(period.from)) / DAY_MS) + 1
}

/** The period a preset names, ending today. An unknown preset is 30 days. */
export function presetPeriod(preset: string, today: string): Period {
  const days = PRESET_DAYS[preset] ?? 30
  return { from: addDays(today, -(days - 1)), to: today }
}

/** The period of the same length that ends the day before `period` starts. */
export function previousPeriod(period: Period): Period {
  const days = lengthInDays(period)
  return { from: addDays(period.from, -days), to: addDays(period.from, -1) }
}

/** Every day of the period, oldest first. */
export function daysOf(period: Period): string[] {
  const days: string[] = []
  for (let d = period.from; d <= period.to; d = addDays(d, 1)) days.push(d)
  return days
}

/**
 * The relative change from `previous` to `current`, as a fraction (0.25 is
 * +25 %). `null` when there is nothing to compare: a value is missing, or
 * the previous one is 0.
 */
export function relativeChange(
  current: number | null | undefined,
  previous: number | null | undefined,
): number | null {
  if (current == null || previous == null || previous === 0) return null
  return (current - previous) / previous
}

/** The difference of two rates (fractions), for a change in points. */
export function pointChange(
  current: number | null | undefined,
  previous: number | null | undefined,
): number | null {
  if (current == null || previous == null) return null
  return current - previous
}
