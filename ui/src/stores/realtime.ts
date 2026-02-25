import { defineStore } from 'pinia'
import { ref, onUnmounted } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface RealtimePage {
  path: string
  visitors: number
}

export const useRealtimeStore = defineStore('realtime', () => {
  const currentVisitors = ref(0)
  const topPages = ref<RealtimePage[]>([])
  let pollTimer: ReturnType<typeof setInterval> | null = null

  async function fetch(orgId: string, siteId: string) {
    const { get } = useHttpClient()
    try {
      const data = await get<{ current_visitors: number; top_pages: RealtimePage[] }>(
        `/org/${orgId}/site/${siteId}/realtime`,
      )
      currentVisitors.value = data.current_visitors
      topPages.value = data.top_pages
    } catch {
      // Silently ignore realtime fetch errors
    }
  }

  function startPolling(orgId: string, siteId: string, intervalMs = 30000) {
    stopPolling()
    fetch(orgId, siteId)
    pollTimer = setInterval(() => fetch(orgId, siteId), intervalMs)
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  return { currentVisitors, topPages, fetch, startPolling, stopPolling }
})
