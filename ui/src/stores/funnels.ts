import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'
import { dimensionLabel } from '@/utils/dimensions'

export type StepType = 'page' | 'goal'

export interface FunnelStep {
  type: StepType
  pattern: string | null
  goal_id: string | null
  label: string | null
}

export interface Funnel {
  id: string
  site_id: string
  name: string
  steps: FunnelStep[]
}

export interface FunnelInput {
  name: string
  steps: { type: StepType; pattern?: string; goal_id?: string; label?: string }[]
}

export interface StepNumbers {
  visitors: number
  conversion_from_previous: number | null
  conversion_from_start: number | null
  dropoff: number | null
}

export interface StepReport extends StepNumbers {
  label: string
  type: StepType
  goal_missing?: boolean
}

export interface FunnelFilter {
  dimension: string
  operator: string
  value: string
}

export interface FunnelReport {
  funnel: Funnel
  steps: StepReport[]
  breakdown: {
    dimension: string
    rows: { value: string; steps: StepNumbers[] }[]
    has_more: boolean
  } | null
  meta: { date_from: string; date_to: string; hostnames: string[] }
}

export interface ReportRequest {
  date_from: string
  date_to: string
  breakdown?: string
  filters?: FunnelFilter[]
  limit?: number
}

/** The visit attributes a funnel is broken down and filtered by. */
export const VISIT_DIMENSIONS = [
  'channel',
  'referrer_source',
  'referrer_host',
  'utm_source',
  'utm_medium',
  'utm_campaign',
  'utm_content',
  'utm_term',
  'utm_source_group',
  'utm_medium_group',
].map((value) => ({ value, title: dimensionLabel(value) }))

export const useFunnelsStore = defineStore('funnels', () => {
  const funnels = ref<Funnel[]>([])
  const loading = ref(false)

  const base = (orgId: string, siteId: string) => `/org/${orgId}/site/${siteId}/funnel`

  async function fetchFunnels(orgId: string, siteId: string) {
    const { get } = useHttpClient()
    loading.value = true
    try {
      funnels.value = await get<Funnel[]>(base(orgId, siteId))
    } finally {
      loading.value = false
    }
  }

  async function createFunnel(orgId: string, siteId: string, input: FunnelInput) {
    const { post } = useHttpClient()
    const funnel = await post<Funnel>(base(orgId, siteId), input)
    funnels.value.unshift(funnel)
    return funnel
  }

  async function updateFunnel(orgId: string, siteId: string, funnelId: string, input: FunnelInput) {
    const { put } = useHttpClient()
    const funnel = await put<Funnel>(`${base(orgId, siteId)}/${funnelId}`, input)
    const i = funnels.value.findIndex((f) => f.id === funnelId)
    if (i >= 0) funnels.value[i] = funnel
    return funnel
  }

  async function deleteFunnel(orgId: string, siteId: string, funnelId: string) {
    const { del } = useHttpClient()
    await del(`${base(orgId, siteId)}/${funnelId}`)
    funnels.value = funnels.value.filter((f) => f.id !== funnelId)
  }

  async function fetchReport(orgId: string, siteId: string, funnelId: string, request: ReportRequest) {
    const { post } = useHttpClient()
    return post<FunnelReport>(`${base(orgId, siteId)}/${funnelId}/stats`, request)
  }

  return { funnels, loading, fetchFunnels, createFunnel, updateFunnel, deleteFunnel, fetchReport }
})
