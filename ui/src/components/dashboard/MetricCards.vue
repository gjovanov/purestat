<template>
  <v-row>
    <v-col cols="6" md="3" v-for="metric in metrics" :key="metric.key">
      <v-card class="pa-4 text-center">
        <div class="text-h4 font-weight-bold" :class="metric.color">
          {{ formatValue(metric.key, metric.value) }}
        </div>
        <div class="text-body-2 text-medium-emphasis mt-1">
          {{ metric.label }}
        </div>
      </v-card>
    </v-col>
  </v-row>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  visitors: number
  pageviews: number
  bounceRate: number
  visitDuration: number
}>()

const metrics = computed(() => [
  { key: 'visitors', label: 'Visitors', value: props.visitors, color: 'text-primary' },
  { key: 'pageviews', label: 'Pageviews', value: props.pageviews, color: 'text-primary' },
  { key: 'bounce_rate', label: 'Bounce Rate', value: props.bounceRate, color: 'text-warning' },
  { key: 'duration', label: 'Avg. Duration', value: props.visitDuration, color: 'text-secondary' },
])

function formatValue(key: string, value: number): string {
  if (key === 'bounce_rate') return `${value}%`
  if (key === 'duration') {
    const mins = Math.floor(value / 60)
    const secs = Math.floor(value % 60)
    return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`
  }
  return value >= 1000 ? `${(value / 1000).toFixed(1)}k` : String(value)
}
</script>
