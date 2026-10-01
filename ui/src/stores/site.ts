import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

export interface Site {
  id: string
  org_id: string
  domain: string
  name: string
  timezone: string
  is_public: boolean
  allowed_hostnames: string[]
}

export const useSiteStore = defineStore('site', () => {
  const sites = ref<Site[]>([])
  const currentSite = ref<Site | null>(null)
  const selectedSiteIds = ref<string[]>([])
  const loading = ref(false)

  const siteDomainsById = computed(() => {
    const map: Record<string, string> = {}
    for (const s of sites.value) {
      map[s.id] = s.domain
    }
    return map
  })

  async function fetchSites(orgId: string) {
    const { get } = useHttpClient()
    loading.value = true
    try {
      sites.value = await get<Site[]>(`/org/${orgId}/site`)
    } finally {
      loading.value = false
    }
  }

  async function fetchSite(orgId: string, siteId: string) {
    const { get } = useHttpClient()
    currentSite.value = await get<Site>(`/org/${orgId}/site/${siteId}`)
    return currentSite.value
  }

  async function createSite(orgId: string, domain: string, name: string, timezone?: string) {
    const { post } = useHttpClient()
    const site = await post<Site>(`/org/${orgId}/site`, { domain, name, timezone })
    sites.value.push(site)
    return site
  }

  async function updateSite(
    orgId: string,
    siteId: string,
    data: { name?: string; timezone?: string; is_public?: boolean; allowed_hostnames?: string[] },
  ) {
    const { put } = useHttpClient()
    const site = await put<Site>(`/org/${orgId}/site/${siteId}`, data)
    const idx = sites.value.findIndex((s) => s.id === siteId)
    if (idx >= 0) sites.value[idx] = site
    if (currentSite.value?.id === siteId) currentSite.value = site
    return site
  }

  async function deleteSite(orgId: string, siteId: string) {
    const { del } = useHttpClient()
    await del(`/org/${orgId}/site/${siteId}`)
    sites.value = sites.value.filter((s) => s.id !== siteId)
    if (currentSite.value?.id === siteId) currentSite.value = null
  }

  function selectSite(siteId: string) {
    currentSite.value = sites.value.find((s) => s.id === siteId) || null
  }

  function setSelectedSiteIds(ids: string[]) {
    selectedSiteIds.value = ids
  }

  function selectAllSites() {
    selectedSiteIds.value = sites.value.map((s) => s.id)
  }

  function clearSelection() {
    selectedSiteIds.value = []
  }

  return {
    sites, currentSite, selectedSiteIds, loading, siteDomainsById,
    fetchSites, fetchSite, createSite, updateSite, deleteSite, selectSite,
    setSelectedSiteIds, selectAllSites, clearSelection,
  }
})
