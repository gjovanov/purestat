<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">Goals</h1>
      <v-spacer />
      <v-btn color="primary" prepend-icon="mdi-plus" @click="showDialog = true">
        {{ $t('goals.create') }}
      </v-btn>
    </div>

    <v-card v-if="goalsStore.goals.length > 0">
      <v-list>
        <v-list-item v-for="goal in goalsStore.goals" :key="goal.id">
          <template v-slot:prepend>
            <v-icon :color="goal.goal_type === 'custom_event' ? 'primary' : 'secondary'">
              {{ goal.goal_type === 'custom_event' ? 'mdi-flash' : 'mdi-file-document-outline' }}
            </v-icon>
          </template>
          <v-list-item-title>{{ goal.name }}</v-list-item-title>
          <v-list-item-subtitle>
            {{ goal.goal_type === 'custom_event' ? `Event: ${goal.event_name}` : `Page: ${goal.page_path}` }}
          </v-list-item-subtitle>
          <template v-slot:append>
            <v-btn icon size="small" color="error" variant="text" @click="handleDelete(goal.id)">
              <v-icon>mdi-delete-outline</v-icon>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>
    </v-card>

    <v-card v-else class="pa-8 text-center">
      <v-icon size="64" color="secondary" class="mb-4">mdi-bullseye-arrow</v-icon>
      <p class="text-body-1 text-medium-emphasis">No goals configured yet</p>
    </v-card>

    <!-- Create Goal Dialog -->
    <v-dialog v-model="showDialog" max-width="500">
      <v-card class="pa-6">
        <v-card-title class="text-h6 pa-0 mb-4">{{ $t('goals.create') }}</v-card-title>
        <v-form @submit.prevent="handleCreate">
          <v-text-field v-model="goalName" :label="$t('goals.name')" required class="mb-2" />
          <v-select
            v-model="goalType"
            :items="goalTypes"
            item-title="label"
            item-value="value"
            :label="$t('goals.type')"
            class="mb-2"
          />
          <v-text-field
            v-if="goalType === 'custom_event'"
            v-model="eventName"
            :label="$t('goals.eventName')"
            placeholder="signup"
            class="mb-2"
          />
          <v-text-field
            v-if="goalType === 'pageview'"
            v-model="pagePath"
            :label="$t('goals.pagePath')"
            placeholder="/thank-you"
            class="mb-4"
          />
          <div class="d-flex justify-end ga-2">
            <v-btn variant="text" @click="showDialog = false">{{ $t('common.cancel') }}</v-btn>
            <v-btn type="submit" color="primary" :loading="creating">{{ $t('common.create') }}</v-btn>
          </div>
        </v-form>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useGoalsStore } from '@/stores/goals'

const route = useRoute()
const goalsStore = useGoalsStore()

const orgId = route.params.orgId as string
const siteId = route.params.siteId as string

const showDialog = ref(false)
const goalName = ref('')
const goalType = ref('custom_event')
const eventName = ref('')
const pagePath = ref('')
const creating = ref(false)

const goalTypes = [
  { label: 'Custom Event', value: 'custom_event' },
  { label: 'Pageview', value: 'pageview' },
]

onMounted(() => goalsStore.fetchGoals(orgId, siteId))

async function handleCreate() {
  creating.value = true
  try {
    await goalsStore.createGoal(orgId, siteId, {
      goal_type: goalType.value,
      name: goalName.value,
      event_name: goalType.value === 'custom_event' ? eventName.value : undefined,
      page_path: goalType.value === 'pageview' ? pagePath.value : undefined,
    })
    showDialog.value = false
    goalName.value = ''
    eventName.value = ''
    pagePath.value = ''
  } finally {
    creating.value = false
  }
}

async function handleDelete(goalId: string) {
  if (confirm('Delete this goal?')) {
    await goalsStore.deleteGoal(orgId, siteId, goalId)
  }
}
</script>
