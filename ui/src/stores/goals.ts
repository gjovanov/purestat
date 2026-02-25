import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface Goal {
  id: string
  site_id: string
  goal_type: string
  name: string
  event_name: string | null
  page_path: string | null
}

export const useGoalsStore = defineStore('goals', () => {
  const goals = ref<Goal[]>([])
  const loading = ref(false)

  async function fetchGoals(orgId: string, siteId: string) {
    const { get } = useHttpClient()
    loading.value = true
    try {
      goals.value = await get<Goal[]>(`/org/${orgId}/site/${siteId}/goal`)
    } finally {
      loading.value = false
    }
  }

  async function createGoal(
    orgId: string,
    siteId: string,
    data: { goal_type: string; name: string; event_name?: string; page_path?: string },
  ) {
    const { post } = useHttpClient()
    const goal = await post<Goal>(`/org/${orgId}/site/${siteId}/goal`, data)
    goals.value.push(goal)
    return goal
  }

  async function deleteGoal(orgId: string, siteId: string, goalId: string) {
    const { del } = useHttpClient()
    await del(`/org/${orgId}/site/${siteId}/goal/${goalId}`)
    goals.value = goals.value.filter((g) => g.id !== goalId)
  }

  return { goals, loading, fetchGoals, createGoal, deleteGoal }
})
