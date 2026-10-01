import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface RealtimePage {
  path: string
  visitors: number
}

export interface RealtimeVisitor {
  visitor_hash: string
  country: string
  site_id: number
  hostname: string
  path: string
}

interface MultiSiteRealtimeData {
  current_visitors: number
  visitors: RealtimeVisitor[]
  top_pages: RealtimePage[]
}

export const useRealtimeStore = defineStore('realtime', () => {
  const currentVisitors = ref(0)
  const topPages = ref<RealtimePage[]>([])
  const visitors = ref<RealtimeVisitor[]>([])
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

  async function fetchMulti(orgId: string, siteIds: string[]) {
    const { get } = useHttpClient()
    try {
      const data = await get<MultiSiteRealtimeData>(
        `/org/${orgId}/analytics/realtime?site_ids=${siteIds.join(',')}`,
      )
      currentVisitors.value = data.current_visitors
      visitors.value = data.visitors
      topPages.value = data.top_pages
    } catch {
      // Silently ignore realtime fetch errors
    }
  }

  function startPollingMulti(orgId: string, siteIds: string[], intervalMs = 30000) {
    stopPolling()
    fetchMulti(orgId, siteIds)
    pollTimer = setInterval(() => fetchMulti(orgId, siteIds), intervalMs)
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  return { currentVisitors, topPages, visitors, fetch, startPolling, fetchMulti, startPollingMulti, stopPolling }
})
