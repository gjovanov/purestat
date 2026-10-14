<template>
  <div class="funnel-steps">
    <div v-for="(step, i) in steps" :key="i" class="mb-4">
      <div v-if="i > 0" class="d-flex align-center text-body-2 text-medium-emphasis mb-2 ml-9">
        <v-icon size="16" class="mr-1">mdi-arrow-down</v-icon>
        {{ $t('funnels.continued', { share: formatPercent(step.conversion_from_previous) }) }}
        <span v-if="step.dropoff" class="ml-2 text-error">
          · {{ $t('funnels.droppedOff', { count: step.dropoff }) }}
        </span>
      </div>
      <div class="d-flex align-center ga-3">
        <v-avatar size="28" color="primary" variant="tonal" class="text-caption font-weight-bold">
          {{ i + 1 }}
        </v-avatar>
        <div class="flex-grow-1" style="min-width: 0">
          <div class="d-flex align-center mb-1">
            <v-icon size="16" class="mr-1" :color="step.type === 'goal' ? 'secondary' : 'primary'">
              {{ step.type === 'goal' ? 'mdi-bullseye-arrow' : 'mdi-file-document-outline' }}
            </v-icon>
            <span class="text-body-2 font-weight-medium text-truncate">{{ step.label }}</span>
            <v-chip v-if="step.goal_missing" size="x-small" color="warning" variant="tonal" class="ml-2">
              {{ $t('funnels.goalMissing') }}
            </v-chip>
            <v-spacer />
            <span class="text-body-2 font-weight-bold ml-2">{{ step.visitors }}</span>
            <span class="text-body-2 text-medium-emphasis ml-1">{{ $t('funnels.visitors', step.visitors) }}</span>
            <span class="text-body-2 text-medium-emphasis ml-3" style="min-width: 52px; text-align: right">
              {{ formatPercent(step.conversion_from_start) }}
            </span>
          </div>
          <v-progress-linear
            :model-value="(step.conversion_from_start ?? 0) * 100"
            :color="step.type === 'goal' ? 'secondary' : 'primary'"
            bg-color="surface-variant"
            height="14"
            rounded
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { StepReport } from '@/stores/funnels'
import { formatPercent } from '@/utils/format'

defineProps<{ steps: StepReport[] }>()
</script>
