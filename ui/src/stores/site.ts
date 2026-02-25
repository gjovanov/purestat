import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface Site {
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
  const loading = ref(false)

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

  return { sites, currentSite, loading, fetchSites, fetchSite, createSite, updateSite, deleteSite, selectSite }
})
