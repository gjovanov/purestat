<template>
  <v-table density="compact">
    <thead>
      <tr>
        <th>{{ dimensionLabel(dimension) }}</th>
        <th v-for="(label, i) in labels" :key="i" class="text-right">
          {{ i + 1 }}. {{ label }}
        </th>
        <th class="text-right">{{ $t('funnels.conversion') }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="row in rows" :key="row.value">
        <td class="text-truncate" style="max-width: 220px">{{ valueLabel(dimension, row.value) }}</td>
        <td v-for="(step, i) in row.steps" :key="i" class="text-right">
          {{ formatCount(step.visitors) }}
          <span v-if="i > 0" class="text-medium-emphasis text-caption">
            ({{ formatPercent(step.conversion_from_previous) }})
          </span>
        </td>
        <td class="text-right font-weight-medium">
          {{ formatPercent(row.steps[row.steps.length - 1]?.conversion_from_start) }}
        </td>
      </tr>
      <tr v-if="rows.length === 0">
        <td :colspan="labels.length + 2" class="text-medium-emphasis">{{ $t('funnels.noData') }}</td>
      </tr>
    </tbody>
  </v-table>
</template>

<script setup lang="ts">
import type { StepNumbers } from '@/stores/funnels'
import { dimensionLabel, valueLabel } from '@/utils/dimensions'
import { formatCount, formatPercent } from '@/utils/format'

defineProps<{
  /** The breakdown's dimension, as the API names it. */
  dimension: string
  labels: string[]
  rows: { value: string; steps: StepNumbers[] }[]
}>()
</script>
