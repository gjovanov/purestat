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
      @remove="statsStore.removeFilter"
      @clear="statsStore.clearFilters"
      class="mb-4"
    />

    <!-- Loading -->
    <v-progress-linear v-if="statsStore.loading" indeterminate color="primary" class="mb-4" />

    <!-- Metric Cards -->
    <MetricCards
      :visitors="statsStore.overview.visitors || 0"
      :pageviews="statsStore.overview.pageviews || 0"
      :bounce-rate="statsStore.overview.bounce_rate || 0"
      :visit-duration="statsStore.overview.visit_duration || 0"
      class="mb-6"
    />

    <!-- Main Chart -->
    <TopChart :timeseries="statsStore.timeseries" class="mb-6" />

    <!-- Detail Tables -->
    <v-row class="mb-6">
      <v-col cols="12" md="6">
        <SourcesTable :sources="statsStore.topSources" />
      </v-col>
      <v-col cols="12" md="6">
        <PagesTable :pages="statsStore.topPages" />
      </v-col>
    </v-row>

    <v-row class="mb-6">
      <v-col cols="12" md="6">
        <LocationsMap :locations="statsStore.locations" />
      </v-col>
      <v-col cols="12" md="6">
        <DevicesTable :devices="statsStore.devices" />
      </v-col>
    </v-row>

    <!-- Goals (single-site only) -->
    <GoalsTable v-if="singleSiteId" :goals="goalsStore.goals" />
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
import MetricCards from '@/components/dashboard/MetricCards.vue'
import TopChart from '@/components/dashboard/TopChart.vue'
import RealtimeBadge from '@/components/dashboard/RealtimeBadge.vue'
import SourcesTable from '@/components/dashboard/SourcesTable.vue'
import PagesTable from '@/components/dashboard/PagesTable.vue'
import LocationsMap from '@/components/dashboard/LocationsMap.vue'
import DevicesTable from '@/components/dashboard/DevicesTable.vue'
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
    // Single-site mode (legacy route)
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
