<template>
  <v-card>
    <v-card-title class="text-body-1 font-weight-bold pa-4 pb-2">
      {{ $t('dashboard.goals') }}
    </v-card-title>
    <v-list density="compact">
      <v-list-item v-for="goal in goals" :key="goal.id" class="px-4">
        <template v-slot:prepend>
          <v-icon size="16" color="secondary" class="mr-2">mdi-bullseye-arrow</v-icon>
        </template>
        <v-list-item-title class="text-body-2">{{ goal.name }}</v-list-item-title>
        <template v-slot:append>
          <v-chip size="x-small" color="secondary" variant="tonal">
            {{ goal.goal_type === 'custom_event' ? goal.event_name : goal.page_path }}
          </v-chip>
        </template>
      </v-list-item>
      <v-list-item v-if="goals.length === 0" class="px-4 text-medium-emphasis">
        No goals configured
      </v-list-item>
    </v-list>
  </v-card>
</template>

<script setup lang="ts">
defineProps<{
  goals: { id: string; name: string; goal_type: string; event_name: string | null; page_path: string | null }[]
}>()
</script>
