// How the dashboard writes numbers. A value the API reports as null (a rate
// without visitors, a median without readings) is written as a dash.

export const NONE = '—'

/** 12345 as "12.3k" (cards). */
export function formatCompact(value: number | null | undefined): string {
  if (value == null) return NONE
  if (Math.abs(value) >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (Math.abs(value) >= 1000) return `${(value / 1000).toFixed(1)}k`
  return String(Math.round(value))
}

/** 12345 as "12,345" (tables). */
export function formatCount(value: number | null | undefined): string {
  if (value == null) return NONE
  return Math.round(value).toLocaleString('en-US')
}

/** A fraction (0.253) as a percentage ("25.3%"). */
export function formatPercent(fraction: number | null | undefined): string {
  if (fraction == null) return NONE
  const percent = fraction * 100
  return `${percent >= 10 || percent === 0 ? Math.round(percent) : percent.toFixed(1)}%`
}

/** Seconds as "4m 05s", "45s". */
export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null) return NONE
  const total = Math.round(seconds)
  const minutes = Math.floor(total / 60)
  const rest = total % 60
  if (minutes >= 60) return `${Math.floor(minutes / 60)}h ${minutes % 60}m`
  return minutes > 0 ? `${minutes}m ${String(rest).padStart(2, '0')}s` : `${rest}s`
}

/** A relative change (0.25) as "+25%". */
export function formatChange(change: number | null): string {
  if (change == null) return NONE
  const percent = Math.round(change * 100)
  return `${percent > 0 ? '+' : ''}${percent}%`
}

/** A change of rates (0.025) in percentage points: "+2.5 pp". */
export function formatPoints(change: number | null): string {
  if (change == null) return NONE
  const points = Math.round(change * 1000) / 10
  return `${points > 0 ? '+' : ''}${points} pp`
}
