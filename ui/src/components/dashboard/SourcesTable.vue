<template>
  <v-card>
    <v-card-title class="text-body-1 font-weight-bold pa-4 pb-2">
      {{ $t('dashboard.topSources') }}
    </v-card-title>
    <v-list density="compact">
      <v-list-item v-for="item in sources" :key="item.value" class="px-4">
        <template v-slot:prepend>
          <v-icon size="16" color="primary" class="mr-2">mdi-arrow-top-right</v-icon>
        </template>
        <v-list-item-title class="text-body-2">{{ item.value || 'Direct' }}</v-list-item-title>
        <template v-slot:append>
          <span class="text-body-2 font-weight-medium">
            {{ item.metrics.visitors }}
          </span>
        </template>
      </v-list-item>
      <v-list-item v-if="sources.length === 0" class="px-4 text-medium-emphasis">
        No data yet
      </v-list-item>
    </v-list>
  </v-card>
</template>

<script setup lang="ts">
defineProps<{
  sources: { value: string; metrics: Record<string, number> }[]
}>()
</script>
