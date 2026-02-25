<template>
  <v-card class="pa-4">
    <div style="height: 300px">
      <Line :data="chartData" :options="chartOptions" />
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Line } from 'vue-chartjs'
import { useCharts } from '@/composables/useCharts'

const props = defineProps<{
  timeseries: { date: string; metrics: Record<string, number> }[]
}>()

const { buildLineChartData, defaultLineOptions } = useCharts()

const chartData = computed(() => {
  const labels = props.timeseries.map((p) => {
    const d = new Date(p.date)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  })
  return buildLineChartData(labels, [
    { label: 'Visitors', data: props.timeseries.map((p) => p.metrics.visitors || 0) },
    { label: 'Pageviews', data: props.timeseries.map((p) => p.metrics.pageviews || 0) },
  ])
})

const chartOptions = defaultLineOptions
</script>
