<template>
  <div class="d-flex align-center flex-wrap ga-2">
    <v-btn
      size="small"
      variant="tonal"
      color="primary"
      prepend-icon="mdi-filter-variant"
      data-test="add-filter"
      @click="dialog = true"
    >
      Filter
    </v-btn>
    <v-chip
      v-for="(filter, i) in filters"
      :key="`${filter.dimension}-${i}`"
      closable
      size="small"
      color="primary"
      variant="tonal"
      data-test="filter-chip"
      @click:close="$emit('remove', i)"
    >
      {{ dimensionLabel(filter.dimension) }} {{ operatorLabel(filter.operator) }}
      <strong class="ml-1">{{ valueLabel(filter.dimension, filter.value) }}</strong>
    </v-chip>
    <v-btn v-if="filters.length > 1" size="x-small" variant="text" color="error" @click="$emit('clear')">
      Clear all
    </v-btn>
    <FilterDialog v-model="dialog" @apply="$emit('add', $event)" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import FilterDialog from '@/components/dashboard/FilterDialog.vue'
import type { StatsFilter } from '@/stores/stats'
import { dimensionLabel, operatorLabel, valueLabel } from '@/utils/dimensions'

defineProps<{
  filters: StatsFilter[]
}>()

defineEmits<{
  add: [filter: StatsFilter]
  remove: [index: number]
  clear: []
}>()

const dialog = ref(false)
</script>
