import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface Org {
  id: string
  name: string
  slug: string
  owner_id: string
  plan: string
  limits: { max_sites: number; max_members: number; max_pageviews_monthly: number }
  usage: { current_month_pageviews: number }
}

export const useOrgStore = defineStore('org', () => {
  const orgs = ref<Org[]>([])
  const currentOrg = ref<Org | null>(null)
  const loading = ref(false)

  async function fetchOrgs() {
    const { get } = useHttpClient()
    loading.value = true
    try {
      orgs.value = await get<Org[]>('/org')
    } finally {
      loading.value = false
    }
  }

  async function fetchOrg(orgId: string) {
    const { get } = useHttpClient()
    currentOrg.value = await get<Org>(`/org/${orgId}`)
    return currentOrg.value
  }

  async function createOrg(name: string, slug: string) {
    const { post } = useHttpClient()
    const org = await post<Org>('/org', { name, slug })
    orgs.value.push(org)
    return org
  }

  async function updateOrg(orgId: string, name: string) {
    const { put } = useHttpClient()
    const org = await put<Org>(`/org/${orgId}`, { name })
    const idx = orgs.value.findIndex((o) => o.id === orgId)
    if (idx >= 0) orgs.value[idx] = org
    if (currentOrg.value?.id === orgId) currentOrg.value = org
    return org
  }

  async function deleteOrg(orgId: string) {
    const { del } = useHttpClient()
    await del(`/org/${orgId}`)
    orgs.value = orgs.value.filter((o) => o.id !== orgId)
    if (currentOrg.value?.id === orgId) currentOrg.value = null
  }

  function selectOrg(orgId: string) {
    currentOrg.value = orgs.value.find((o) => o.id === orgId) || null
  }

  return { orgs, currentOrg, loading, fetchOrgs, fetchOrg, createOrg, updateOrg, deleteOrg, selectOrg }
})
