<template>
  <v-card data-test="goals-card">
    <div class="d-flex align-center px-4 pt-3 pb-1">
      <span class="text-body-1 font-weight-bold">{{ $t('dashboard.goals') }}</span>
      <v-spacer />
      <span class="text-caption text-medium-emphasis">{{ comparison }}</span>
    </div>
    <v-table density="compact">
      <thead>
        <tr>
          <th class="text-left">Goal</th>
          <th class="text-right" title="Unique visitors who completed the goal">Conversions</th>
          <th class="text-right" title="Conversions / unique visitors">CR</th>
          <th class="text-right" :title="comparison">Change</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.name" :data-test="`goal-${row.name}`">
          <td>
            <v-icon size="16" color="secondary" class="mr-1">{{ row.icon }}</v-icon>
            {{ row.name }}
            <span class="text-caption text-medium-emphasis ml-1">{{ row.condition }}</span>
          </td>
          <td class="text-right">{{ formatCount(row.visitors) }}</td>
          <td class="text-right">{{ formatPercent(row.rate) }}</td>
          <td class="text-right text-no-wrap" :class="row.changeColor">
            {{ row.change }}
            <span class="text-caption text-medium-emphasis">{{ row.rateChange }}</span>
          </td>
        </tr>
        <tr v-if="rows.length === 0">
          <td colspan="4" class="text-medium-emphasis">No goals configured</td>
        </tr>
      </tbody>
    </v-table>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { GoalResult } from '@/stores/stats'
import { pointChange, relativeChange } from '@/utils/period'
import { formatChange, formatCount, formatPercent, formatPoints } from '@/utils/format'

const props = defineProps<{
  goals: { id: string; name: string; goal_type: string; event_name: string | null; page_path: string | null }[]
  results: Record<string, GoalResult>
  previous: Record<string, GoalResult>
  comparison: string
}>()

// Goals of one name are one goal: completing any of them completes it.
const rows = computed(() => {
  const byName = new Map<string, typeof props.goals>()
  for (const g of props.goals) byName.set(g.name, [...(byName.get(g.name) ?? []), g])
  return [...byName.entries()].map(([name, defs]) => {
    const now = props.results[name]
    const before = props.previous[name]
    const change = relativeChange(now?.visitors, before?.visitors)
    return {
      name,
      icon: defs[0].goal_type === 'custom_event' ? 'mdi-flash' : 'mdi-file-document-outline',
      condition: defs
        .map((g) => (g.goal_type === 'custom_event' ? g.event_name : g.page_path))
        .join(', '),
      visitors: now?.visitors ?? null,
      rate: now?.rate ?? null,
      change: formatChange(change),
      rateChange: formatPoints(pointChange(now?.rate, before?.rate)),
      changeColor:
        change == null || change === 0 ? '' : change > 0 ? 'text-success' : 'text-error',
    }
  })
})
</script>
