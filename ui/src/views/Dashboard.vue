<template>
  <div>
    <!-- Header -->
    <div class="d-flex align-center flex-wrap ga-3 mb-4">
      <h1 class="text-h5 font-weight-bold mr-auto">
        {{ headerTitle }}
      </h1>
      <RealtimeBadge :count="realtimeStore.currentVisitors" :visitors="realtimeStore.visitors" />
      <DatePicker :model-value="statsStore.dateRange" @change="onDateRangeChange" />
    </div>

    <!-- Filters -->
    <FilterBar
      :filters="statsStore.filters"
      @add="statsStore.setFilter"
      @remove="statsStore.removeFilter"
      @clear="statsStore.clearFilters"
      class="mb-4"
    />

    <!-- Loading -->
    <v-progress-linear v-if="statsStore.loading" indeterminate color="primary" class="mb-4" />

    <!-- Metric Cards -->
    <MetricCards
      :current="statsStore.overview"
      :previous="statsStore.previousOverview"
      :comparison="comparison"
      class="mb-6"
    />

    <!-- Main Chart -->
    <TopChart :timeseries="statsStore.timeseries" :previous="statsStore.previousTimeseries" class="mb-6" />

    <!-- Reports: a click on a row filters the whole dashboard by it -->
    <v-row class="mb-6">
      <v-col cols="12" md="6">
        <BreakdownCard title="Sources" :tabs="SOURCE_TABS" @filter="statsStore.setFilter" />
      </v-col>
      <v-col cols="12" md="6">
        <BreakdownCard
          title="Campaigns"
          :tabs="CAMPAIGN_TABS"
          :goals="singleSiteId ? goalNames : undefined"
          @filter="statsStore.setFilter"
        />
      </v-col>
      <v-col cols="12" md="6">
        <BreakdownCard title="Pages" :tabs="PAGE_TABS" @filter="statsStore.setFilter" />
      </v-col>
      <!-- Goals belong to a site -->
      <v-col v-if="singleSiteId" cols="12" md="6">
        <GoalsTable
          :goals="goalsStore.goals"
          :results="statsStore.goalResults"
          :previous="statsStore.previousGoalResults"
          :comparison="comparison"
        />
      </v-col>
      <v-col cols="12" md="6">
        <BreakdownCard title="Locations" :tabs="LOCATION_TABS" @filter="statsStore.setFilter" />
      </v-col>
      <v-col cols="12" md="6">
        <BreakdownCard title="Devices" :tabs="DEVICE_TABS" @filter="statsStore.setFilter" />
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'
import { useStatsStore } from '@/stores/stats'
import { useRealtimeStore } from '@/stores/realtime'
import { useGoalsStore } from '@/stores/goals'
import { lengthInDays } from '@/utils/period'
import MetricCards from '@/components/dashboard/MetricCards.vue'
import TopChart from '@/components/dashboard/TopChart.vue'
import RealtimeBadge from '@/components/dashboard/RealtimeBadge.vue'
import BreakdownCard from '@/components/dashboard/BreakdownCard.vue'
import type { BreakdownTab } from '@/components/dashboard/breakdown'
import GoalsTable from '@/components/dashboard/GoalsTable.vue'
import DatePicker from '@/components/dashboard/DatePicker.vue'
import FilterBar from '@/components/dashboard/FilterBar.vue'

const route = useRoute()
const orgStore = useOrgStore()
const siteStore = useSiteStore()
const statsStore = useStatsStore()
const realtimeStore = useRealtimeStore()
const goalsStore = useGoalsStore()

const orgId = route.params.orgId as string
const singleSiteId = route.params.siteId as string | undefined

// Until this dashboard names its site(s), no card reads the last one's.
statsStore.enterDashboard(orgId, singleSiteId ?? null)

const VISITORS = [{ key: 'visitors', label: 'Visitors' }]
const VISITORS_AND_VISITS = [
  { key: 'visitors', label: 'Visitors' },
  { key: 'visits', label: 'Visits' },
]

// Channels group the visit's referrer by FR-102 §3b, on the server.
const SOURCE_TABS: BreakdownTab[] = [
  { key: 'channel', label: 'Channels', dimension: 'channel', metrics: ['visitors', 'visits'], columns: VISITORS_AND_VISITS, next: 'source' },
  { key: 'source', label: 'Sources', dimension: 'referrer_source', metrics: ['visitors', 'visits'], columns: VISITORS_AND_VISITS, next: 'referrer' },
  { key: 'referrer', label: 'Referrers', dimension: 'referrer_host', metrics: ['visitors', 'visits'], columns: VISITORS_AND_VISITS },
]

/** A campaign tab lists the visits that carry one. */
function campaignTab(key: string, label: string, next?: string): BreakdownTab {
  const dimension = `utm_${key}`
  return {
    key,
    label,
    dimension,
    metrics: ['visitors', 'visits'],
    columns: VISITORS_AND_VISITS,
    filters: [{ dimension, operator: 'is_not', value: '' }],
    next,
    goals: true,
  }
}

const CAMPAIGN_TABS: BreakdownTab[] = [
  campaignTab('source', 'Source', 'medium'),
  campaignTab('medium', 'Medium', 'campaign'),
  campaignTab('campaign', 'Campaign', 'content'),
  campaignTab('content', 'Content', 'term'),
  campaignTab('term', 'Term'),
  {
    ...campaignTab('source_group', 'Source group', 'medium_group'),
    filters: [{ dimension: 'utm_source', operator: 'is_not', value: '' }],
  },
  {
    ...campaignTab('medium_group', 'Medium group'),
    filters: [{ dimension: 'utm_medium', operator: 'is_not', value: '' }],
  },
]

const PAGE_TABS: BreakdownTab[] = [
  { key: 'page', label: 'Pages', dimension: 'path', metrics: ['visitors', 'pageviews'], columns: [...VISITORS, { key: 'pageviews', label: 'Pageviews' }] },
  { key: 'entry', label: 'Entry pages', dimension: 'entry_page', metrics: ['visitors', 'visits'], columns: VISITORS_AND_VISITS },
  { key: 'exit', label: 'Exit pages', dimension: 'exit_page', metrics: ['visitors', 'visits'], columns: VISITORS_AND_VISITS },
]

const LOCATION_TABS: BreakdownTab[] = [
  { key: 'country', label: 'Countries', dimension: 'country', metrics: ['visitors'], columns: VISITORS },
]

const DEVICE_TABS: BreakdownTab[] = [
  { key: 'device', label: 'Devices', dimension: 'device_type', metrics: ['visitors'], columns: VISITORS, next: 'browser' },
  { key: 'browser', label: 'Browsers', dimension: 'browser', metrics: ['visitors'], columns: VISITORS, next: 'os' },
  { key: 'os', label: 'Operating systems', dimension: 'os', metrics: ['visitors'], columns: VISITORS },
]

const goalNames = computed(() => [...new Set(goalsStore.goals.map((g) => g.name))])

const comparison = computed(() => {
  const days = lengthInDays(statsStore.period)
  return days === 1 ? 'vs the day before' : `vs the previous ${days} days`
})

const headerTitle = computed(() => {
  if (singleSiteId) {
    return siteStore.currentSite?.domain || ''
  }
  const count = siteStore.selectedSiteIds.length
  if (count === 0) return 'No sites selected'
  if (count === 1) {
    return siteStore.siteDomainsById[siteStore.selectedSiteIds[0]] || '1 site'
  }
  return `${count} sites`
})

onMounted(async () => {
  if (singleSiteId) {
    // Single-site mode
    await Promise.all([
      orgStore.fetchOrg(orgId),
      siteStore.fetchSite(orgId, singleSiteId),
      goalsStore.fetchGoals(orgId, singleSiteId),
    ])
    loadDashboard()
    realtimeStore.startPolling(orgId, singleSiteId)
  } else {
    // Multi-site mode
    await orgStore.fetchOrg(orgId)
    if (siteStore.sites.length === 0) {
      await siteStore.fetchSites(orgId)
    }
    if (siteStore.selectedSiteIds.length === 0) {
      siteStore.selectAllSites()
    }
    loadDashboard()
    startMultiPolling()
  }
})

onUnmounted(() => {
  realtimeStore.stopPolling()
})

function loadDashboard() {
  if (singleSiteId) {
    statsStore.fetchDashboard(orgId, singleSiteId)
  } else if (siteStore.selectedSiteIds.length > 0) {
    statsStore.fetchMultiSiteDashboard(orgId, siteStore.selectedSiteIds)
  }
}

function startMultiPolling() {
  if (siteStore.selectedSiteIds.length > 0) {
    realtimeStore.startPollingMulti(orgId, siteStore.selectedSiteIds)
  }
}

function onDateRangeChange(range: string) {
  statsStore.setDateRange(range)
  loadDashboard()
}

// Watch for site selection changes in multi-site mode
watch(
  () => siteStore.selectedSiteIds,
  () => {
    if (!singleSiteId) {
      realtimeStore.stopPolling()
      loadDashboard()
      startMultiPolling()
    }
  },
  { deep: true },
)

watch(() => statsStore.filters, loadDashboard, { deep: true })
</script>
