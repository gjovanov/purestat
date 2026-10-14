<template>
  <v-row>
    <v-col v-for="card in cards" :key="card.key" cols="6" sm="4" md>
      <v-card class="pa-4 text-center" :data-test="`metric-${card.key}`">
        <div class="text-h5 font-weight-bold" :class="card.color">
          {{ card.value }}
        </div>
        <div class="text-body-2 text-medium-emphasis mt-1">
          {{ card.label }}
        </div>
        <div class="text-caption mt-1" :class="card.changeColor" :title="comparison">
          <v-icon v-if="card.icon" size="12">{{ card.icon }}</v-icon>
          {{ card.change }}
        </div>
      </v-card>
    </v-col>
  </v-row>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { pointChange, relativeChange } from '@/utils/period'
import {
  formatChange,
  formatCompact,
  formatDuration,
  formatPercent,
  formatPoints,
} from '@/utils/format'

const props = defineProps<{
  current: Record<string, number | null | undefined>
  previous: Record<string, number | null | undefined>
  /** What the change is against, for its tooltip ("vs the previous 30 days"). */
  comparison: string
}>()

type Kind = 'count' | 'rate' | 'duration'

const METRICS: { key: string; label: string; kind: Kind; color: string; lowerIsBetter?: boolean }[] = [
  { key: 'visitors', label: 'Unique visitors', kind: 'count', color: 'text-primary' },
  { key: 'visits', label: 'Visits', kind: 'count', color: 'text-primary' },
  { key: 'pageviews', label: 'Pageviews', kind: 'count', color: 'text-primary' },
  { key: 'bounce_rate', label: 'Bounce rate', kind: 'rate', color: 'text-warning', lowerIsBetter: true },
  { key: 'visit_duration', label: 'Visit duration', kind: 'duration', color: 'text-secondary' },
]

const cards = computed(() =>
  METRICS.map((m) => {
    const now = props.current[m.key] ?? null
    const before = props.previous[m.key] ?? null
    const delta = m.kind === 'rate' ? pointChange(now, before) : relativeChange(now, before)
    const value =
      m.kind === 'rate' ? formatPercent(now) : m.kind === 'duration' ? formatDuration(now) : formatCompact(now)
    const better = delta == null || delta === 0 ? null : (delta > 0) !== !!m.lowerIsBetter
    return {
      key: m.key,
      label: m.label,
      color: m.color,
      value,
      change: m.kind === 'rate' ? formatPoints(delta) : formatChange(delta),
      icon: delta == null || delta === 0 ? '' : delta > 0 ? 'mdi-arrow-up' : 'mdi-arrow-down',
      changeColor: better == null ? 'text-medium-emphasis' : better ? 'text-success' : 'text-error',
    }
  }),
)
</script>
