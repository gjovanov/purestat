<template>
  <div class="d-flex align-center flex-wrap ga-2" v-if="filters.length > 0">
    <v-chip
      v-for="(filter, i) in filters"
      :key="i"
      closable
      size="small"
      color="primary"
      variant="tonal"
      @click:close="$emit('remove', i)"
    >
      {{ filter.dimension }} {{ filter.operator }} {{ filter.value }}
    </v-chip>
    <v-btn size="x-small" variant="text" color="error" @click="$emit('clear')">
      Clear all
    </v-btn>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  filters: { dimension: string; operator: string; value: string }[]
}>()

defineEmits<{
  remove: [index: number]
  clear: []
}>()
</script>
