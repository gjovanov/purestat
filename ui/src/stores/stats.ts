import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'
import { useGoalsStore } from '@/stores/goals'
import { daysOf, presetPeriod, previousPeriod, utcToday, type Period } from '@/utils/period'

export interface StatsFilter {
  dimension: string
  operator: string
  value: string
}

/** One row of a report: the v1 query's row (docs/api.md). */
export interface ReportRow {
  dimensions: Record<string, string>
  metrics: Record<string, number | null>
  goals: Record<string, number>
  conversion_rates?: Record<string, number | null>
}

export interface ReportAnswer {
  rows: ReportRow[]
  meta: { has_more: boolean }
}

export interface ReportRequest {
  dimensions?: string[]
  metrics: string[]
  goals?: string[]
  /** Added to the dashboard's own filters, for this request only. */
  filters?: StatsFilter[]
  limit?: number
  offset?: number
}

interface TimeseriesPoint {
  date: string
  metrics: Record<string, number>
}

export interface GoalResult {
  visitors: number
  rate: number | null
}

/** The totals the metric cards show. */
export const TOTAL_METRICS = ['visitors', 'visits', 'pageviews', 'bounce_rate', 'visit_duration']

/** The most goals one query may name (the API's limit). */
const GOALS_PER_QUERY = 10

function chunks<T>(items: T[], size: number): T[][] {
  const out: T[][] = []
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size))
  return out
}

export const useStatsStore = defineStore('stats', () => {
  // What the dashboard reads: one site, or several sites of one org.
  const orgId = ref<string | null>(null)
  const siteId = ref<string | null>(null)
  const siteIds = ref<string[]>([])

  const dateRange = ref('30d')
  const dateFrom = ref<string | null>(null)
  const dateTo = ref<string | null>(null)
  const filters = ref<StatsFilter[]>([])

  const overview = ref<Record<string, number | null>>({})
  const previousOverview = ref<Record<string, number | null>>({})
  const timeseries = ref<TimeseriesPoint[]>([])
  const previousTimeseries = ref<TimeseriesPoint[]>([])
  const goalResults = ref<Record<string, GoalResult>>({})
  const previousGoalResults = ref<Record<string, GoalResult>>({})
  const loading = ref(false)
  /** Bumped whenever what the reports read changes, so every card refetches. */
  const version = ref(0)
  let loadId = 0

  /** The period on screen, and the one it is compared with. */
  const period = computed<Period>(() =>
    dateRange.value === 'custom' && dateFrom.value && dateTo.value
      ? { from: dateFrom.value, to: dateTo.value }
      : presetPeriod(dateRange.value, utcToday()),
  )
  const previous = computed<Period>(() => previousPeriod(period.value))

  /**
   * One report over the dashboard's site(s), period and filters: the same
   * query and the same rows `POST /api/v1/query` answers.
   */
  async function runReport(request: ReportRequest, range: Period = period.value): Promise<ReportAnswer> {
    const { post } = useHttpClient()
    const allFilters = [...filters.value, ...(request.filters ?? [])]
    const body: Record<string, unknown> = {
      date_from: range.from,
      date_to: range.to,
      metrics: request.metrics,
    }
    if (request.dimensions?.length) body.dimensions = request.dimensions
    if (request.goals?.length) body.goals = request.goals
    if (allFilters.length) body.filters = allFilters
    if (request.limit !== undefined) body.limit = request.limit
    if (request.offset !== undefined) body.offset = request.offset
    if (siteId.value) {
      return post<ReportAnswer>(`/org/${orgId.value}/site/${siteId.value}/query`, body)
    }
    const ids = encodeURIComponent(siteIds.value.join(','))
    return post<ReportAnswer>(`/org/${orgId.value}/analytics/query?site_ids=${ids}`, body)
  }

  /** The totals of a period, and per goal its converting visitors and rate. */
  async function loadTotals(range: Period, goalNames: string[]) {
    const groups = chunks(goalNames, GOALS_PER_QUERY)
    const answers = await Promise.all([
      runReport({ metrics: TOTAL_METRICS, goals: groups[0] }, range),
      ...groups.slice(1).map((goals) => runReport({ metrics: ['visitors'], goals }, range)),
    ])
    const goals: Record<string, GoalResult> = {}
    for (const answer of answers) {
      const row = answer.rows[0]
      for (const [name, visitors] of Object.entries(row?.goals ?? {})) {
        goals[name] = { visitors, rate: row?.conversion_rates?.[name] ?? null }
      }
    }
    return { metrics: answers[0].rows[0]?.metrics ?? {}, goals }
  }

  /** Visitors and pageviews per day, with a 0 for a day without any. */
  async function loadSeries(range: Period): Promise<TimeseriesPoint[]> {
    const answer = await runReport(
      { dimensions: ['date'], metrics: ['visitors', 'pageviews'], limit: 1000 },
      range,
    )
    const byDay = new Map(answer.rows.map((r) => [r.dimensions.date, r.metrics]))
    return daysOf(range).map((date) => ({
      date,
      metrics: {
        visitors: Number(byDay.get(date)?.visitors ?? 0),
        pageviews: Number(byDay.get(date)?.pageviews ?? 0),
      },
    }))
  }

  async function load() {
    const id = ++loadId
    version.value++
    loading.value = true
    // Goals belong to a site, so only a one-site dashboard reports them.
    const goalNames = siteId.value
      ? [...new Set(useGoalsStore().goals.map((g) => g.name))]
      : []
    try {
      const [current, before, series, previousSeries] = await Promise.all([
        loadTotals(period.value, goalNames),
        loadTotals(previous.value, goalNames),
        loadSeries(period.value),
        loadSeries(previous.value),
      ])
      if (id !== loadId) return
      overview.value = current.metrics
      goalResults.value = current.goals
      previousOverview.value = before.metrics
      previousGoalResults.value = before.goals
      timeseries.value = series
      previousTimeseries.value = previousSeries
    } finally {
      if (id === loadId) loading.value = false
    }
  }

  let dashboardKey = ''

  /**
   * As a dashboard opens: forget the previous one's sites until this one
   * names its own, so no card reads the wrong site meanwhile, and its
   * filters too when it was another dashboard.
   */
  function enterDashboard(org: string, site: string | null) {
    const key = `${org}/${site ?? '*'}`
    if (key !== dashboardKey) filters.value = []
    dashboardKey = key
    orgId.value = null
    siteId.value = null
    siteIds.value = []
  }

  async function fetchDashboard(org: string, site: string) {
    orgId.value = org
    siteId.value = site
    siteIds.value = [site]
    await load()
  }

  async function fetchMultiSiteDashboard(org: string, sites: string[]) {
    orgId.value = org
    siteId.value = null
    siteIds.value = [...sites]
    await load()
  }

  function setDateRange(range: string) {
    dateRange.value = range
    dateFrom.value = null
    dateTo.value = null
  }

  function setCustomRange(from: string, to: string) {
    dateRange.value = 'custom'
    dateFrom.value = from
    dateTo.value = to
  }

  function addFilter(filter: StatsFilter) {
    filters.value.push(filter)
  }

  /** A filter in place of any other on the same dimension (a drill-down). */
  function setFilter(filter: StatsFilter) {
    filters.value = [...filters.value.filter((f) => f.dimension !== filter.dimension), filter]
  }

  function removeFilter(index: number) {
    filters.value.splice(index, 1)
  }

  function clearFilters() {
    filters.value = []
  }

  return {
    orgId,
    siteId,
    siteIds,
    dateRange,
    dateFrom,
    dateTo,
    filters,
    period,
    previous,
    overview,
    previousOverview,
    timeseries,
    previousTimeseries,
    goalResults,
    previousGoalResults,
    loading,
    version,
    runReport,
    enterDashboard,
    fetchDashboard,
    fetchMultiSiteDashboard,
    setDateRange,
    setCustomRange,
    addFilter,
    setFilter,
    removeFilter,
    clearFilters,
  }
})
