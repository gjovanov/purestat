import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface StatsFilter {
  dimension: string
  operator: string
  value: string
}

interface TimeseriesPoint {
  date: string
  metrics: Record<string, number>
}

interface DimensionResult {
  dimension: string
  value: string
  metrics: Record<string, number>
}

interface StatsData {
  metrics: Record<string, number>
  dimensions?: DimensionResult[]
  timeseries?: TimeseriesPoint[]
}

export const useStatsStore = defineStore('stats', () => {
  const dateRange = ref('30d')
  const dateFrom = ref<string | null>(null)
  const dateTo = ref<string | null>(null)
  const filters = ref<StatsFilter[]>([])
  const interval = ref('day')

  const overview = ref<Record<string, number>>({})
  const timeseries = ref<TimeseriesPoint[]>([])
  const topSources = ref<DimensionResult[]>([])
  const topPages = ref<DimensionResult[]>([])
  const locations = ref<DimensionResult[]>([])
  const devices = ref<DimensionResult[]>([])
  const loading = ref(false)

  async function fetchDashboard(orgId: string, siteId: string) {
    loading.value = true
    const { post } = useHttpClient()
    const base = `/org/${orgId}/site/${siteId}/stats`

    try {
      const [overviewData, timeseriesData, sourcesData, pagesData, locationsData, devicesData] =
        await Promise.all([
          post<StatsData>(base, {
            date_range: dateRange.value,
            date_from: dateFrom.value,
            date_to: dateTo.value,
            metrics: ['visitors', 'pageviews', 'bounce_rate', 'visit_duration'],
            filters: filters.value.length ? filters.value : undefined,
          }),
          post<StatsData>(base, {
            date_range: dateRange.value,
            date_from: dateFrom.value,
            date_to: dateTo.value,
            metrics: ['visitors', 'pageviews'],
            interval: interval.value,
            filters: filters.value.length ? filters.value : undefined,
          }),
          post<StatsData>(base, {
            date_range: dateRange.value,
            metrics: ['visitors', 'pageviews'],
            dimensions: ['source'],
            limit: 10,
            filters: filters.value.length ? filters.value : undefined,
          }),
          post<StatsData>(base, {
            date_range: dateRange.value,
            metrics: ['visitors', 'pageviews'],
            dimensions: ['page'],
            limit: 10,
            filters: filters.value.length ? filters.value : undefined,
          }),
          post<StatsData>(base, {
            date_range: dateRange.value,
            metrics: ['visitors'],
            dimensions: ['country'],
            limit: 10,
            filters: filters.value.length ? filters.value : undefined,
          }),
          post<StatsData>(base, {
            date_range: dateRange.value,
            metrics: ['visitors'],
            dimensions: ['device_type'],
            filters: filters.value.length ? filters.value : undefined,
          }),
        ])

      overview.value = overviewData.metrics || {}
      timeseries.value = timeseriesData.timeseries || []
      topSources.value = sourcesData.dimensions || []
      topPages.value = pagesData.dimensions || []
      locations.value = locationsData.dimensions || []
      devices.value = devicesData.dimensions || []
    } finally {
      loading.value = false
    }
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

  function removeFilter(index: number) {
    filters.value.splice(index, 1)
  }

  function clearFilters() {
    filters.value = []
  }

  return {
    dateRange,
    dateFrom,
    dateTo,
    filters,
    interval,
    overview,
    timeseries,
    topSources,
    topPages,
    locations,
    devices,
    loading,
    fetchDashboard,
    setDateRange,
    setCustomRange,
    addFilter,
    removeFilter,
    clearFilters,
  }
})
