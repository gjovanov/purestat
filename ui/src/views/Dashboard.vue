<template>
  <div>
    <!-- Header -->
    <div class="d-flex align-center flex-wrap ga-3 mb-4">
      <h1 class="text-h5 font-weight-bold mr-auto">
        {{ siteStore.currentSite?.domain }}
      </h1>
      <RealtimeBadge :count="realtimeStore.currentVisitors" />
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

    <!-- Goals -->
    <GoalsTable :goals="goalsStore.goals" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'
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
const siteId = route.params.siteId as string

onMounted(async () => {
  await Promise.all([
    orgStore.fetchOrg(orgId),
    siteStore.fetchSite(orgId, siteId),
    goalsStore.fetchGoals(orgId, siteId),
  ])
  loadDashboard()
  realtimeStore.startPolling(orgId, siteId)
})

onUnmounted(() => {
  realtimeStore.stopPolling()
})

function loadDashboard() {
  statsStore.fetchDashboard(orgId, siteId)
}

function onDateRangeChange(range: string) {
  statsStore.setDateRange(range)
  loadDashboard()
}

watch(() => statsStore.filters, loadDashboard, { deep: true })
</script>
