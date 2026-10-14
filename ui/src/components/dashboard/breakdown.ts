import type { StatsFilter } from '@/stores/stats'

/** A column of a breakdown card. */
export interface BreakdownColumn {
  /** A metric of the row, or `conversions` / `conversion_rate` of the goal. */
  key: string
  label: string
  title?: string
  format?: 'count' | 'percent' | 'duration' | 'scroll'
}

/** One tab of a breakdown card: one dimension, its metrics and columns. */
export interface BreakdownTab {
  key: string
  label: string
  dimension: string
  metrics: string[]
  columns: BreakdownColumn[]
  /** Conditions of this tab only, such as "carries a campaign". */
  filters?: StatsFilter[]
  /** The tab a click on a row moves to, after filtering by the row. */
  next?: string
  /** Whether the tab can show a goal's conversions. */
  goals?: boolean
}
