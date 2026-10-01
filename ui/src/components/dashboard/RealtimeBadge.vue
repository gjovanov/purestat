<template>
  <v-menu :close-on-content-click="false" max-width="420">
    <template v-slot:activator="{ props: menuProps }">
      <v-chip
        v-bind="menuProps"
        color="secondary"
        variant="tonal"
        size="small"
        class="mr-2 cursor-pointer"
        data-test="realtime-badge"
      >
        <v-icon start size="8" class="pulse-dot">mdi-circle</v-icon>
        <strong>{{ count }}</strong>
        <span class="ml-1 text-medium-emphasis">current visitors</span>
      </v-chip>
    </template>
    <v-card>
      <v-card-title class="text-subtitle-2">Active Visitors</v-card-title>
      <v-list v-if="visitors && visitors.length > 0" density="compact" max-height="300" class="overflow-y-auto">
        <v-list-item v-for="v in visitors" :key="v.visitor_hash" class="px-4">
          <template v-slot:prepend>
            <span class="text-body-2 mr-2">{{ countryFlag(v.country) }}</span>
          </template>
          <v-list-item-title class="text-body-2">{{ v.path }}</v-list-item-title>
          <v-list-item-subtitle class="text-caption">
            {{ v.hostname }}
          </v-list-item-subtitle>
        </v-list-item>
      </v-list>
      <v-card-text v-else class="text-body-2 text-medium-emphasis">
        No active visitors right now
      </v-card-text>
    </v-card>
  </v-menu>
</template>

<script setup lang="ts">
import type { RealtimeVisitor } from '@/stores/realtime'

defineProps<{
  count: number
  visitors?: RealtimeVisitor[]
}>()

function countryFlag(countryCode: string): string {
  if (!countryCode || countryCode.length !== 2) return '\u{1F310}'
  const offset = 127397
  return String.fromCodePoint(
    ...countryCode
      .toUpperCase()
      .split('')
      .map((c) => c.charCodeAt(0) + offset),
  )
}
</script>

<style scoped>
.pulse-dot {
  animation: pulse 2s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
.cursor-pointer {
  cursor: pointer;
}
</style>
