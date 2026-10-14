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
  /** The previous period, day by day, drawn against the same days. */
  previous?: { date: string; metrics: Record<string, number> }[]
}>()

const { buildLineChartData, defaultLineOptions, CHART_COLORS } = useCharts()

const chartData = computed(() => {
  const labels = props.timeseries.map((p) => {
    const d = new Date(`${p.date}T00:00:00Z`)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' })
  })
  const datasets: { label: string; data: number[]; color?: string; dashed?: boolean }[] = [
    { label: 'Visitors', data: props.timeseries.map((p) => p.metrics.visitors || 0) },
    { label: 'Pageviews', data: props.timeseries.map((p) => p.metrics.pageviews || 0) },
  ]
  if (props.previous?.length) {
    datasets.push({
      label: 'Visitors, previous period',
      data: props.previous.map((p) => p.metrics.visitors || 0),
      color: CHART_COLORS.text,
      dashed: true,
    })
  }
  return buildLineChartData(labels, datasets)
})

const chartOptions = defaultLineOptions
</script>
