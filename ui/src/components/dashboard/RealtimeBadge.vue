<template>
  <v-chip color="secondary" variant="tonal" size="small" class="mr-2">
    <v-icon start size="8" class="pulse-dot">mdi-circle</v-icon>
    <strong>{{ count }}</strong>
    <span class="ml-1 text-medium-emphasis">{{ $t('dashboard.currentVisitors') }}</span>
  </v-chip>
</template>

<script setup lang="ts">
defineProps<{ count: number }>()
</script>

<style scoped>
.pulse-dot {
  animation: pulse 2s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
</style>
