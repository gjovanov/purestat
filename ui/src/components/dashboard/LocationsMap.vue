<template>
  <v-card>
    <v-card-title class="text-body-1 font-weight-bold pa-4 pb-2">
      {{ $t('dashboard.locations') }}
    </v-card-title>
    <v-list density="compact">
      <v-list-item v-for="item in locations" :key="item.value" class="px-4">
        <template v-slot:prepend>
          <v-icon size="16" color="primary" class="mr-2">mdi-map-marker</v-icon>
        </template>
        <v-list-item-title class="text-body-2">{{ item.value || 'Unknown' }}</v-list-item-title>
        <template v-slot:append>
          <span class="text-body-2 font-weight-medium">{{ item.metrics.visitors }}</span>
        </template>
      </v-list-item>
      <v-list-item v-if="locations.length === 0" class="px-4 text-medium-emphasis">
        No data yet
      </v-list-item>
    </v-list>
  </v-card>
</template>

<script setup lang="ts">
defineProps<{
  locations: { value: string; metrics: Record<string, number> }[]
}>()
</script>
