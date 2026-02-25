<template>
  <v-card>
    <v-card-title class="text-body-1 font-weight-bold pa-4 pb-2">
      {{ $t('dashboard.devices') }}
    </v-card-title>
    <v-list density="compact">
      <v-list-item v-for="item in devices" :key="item.value" class="px-4">
        <template v-slot:prepend>
          <v-icon size="16" color="primary" class="mr-2">{{ getIcon(item.value) }}</v-icon>
        </template>
        <v-list-item-title class="text-body-2 text-capitalize">{{ item.value || 'Unknown' }}</v-list-item-title>
        <template v-slot:append>
          <div class="d-flex align-center">
            <v-progress-linear
              :model-value="getPercent(item.metrics.visitors)"
              color="primary"
              rounded
              style="width: 60px"
              class="mr-2"
            />
            <span class="text-body-2 font-weight-medium" style="min-width: 35px; text-align: right">
              {{ getPercent(item.metrics.visitors) }}%
            </span>
          </div>
        </template>
      </v-list-item>
      <v-list-item v-if="devices.length === 0" class="px-4 text-medium-emphasis">
        No data yet
      </v-list-item>
    </v-list>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  devices: { value: string; metrics: Record<string, number> }[]
}>()

const total = computed(() => props.devices.reduce((sum, d) => sum + (d.metrics.visitors || 0), 0))

function getPercent(visitors: number): number {
  if (total.value === 0) return 0
  return Math.round((visitors / total.value) * 100)
}

function getIcon(device: string): string {
  switch (device.toLowerCase()) {
    case 'desktop': return 'mdi-monitor'
    case 'mobile': return 'mdi-cellphone'
    case 'tablet': return 'mdi-tablet'
    default: return 'mdi-devices'
  }
}
</script>
