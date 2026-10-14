import { defineStore } from 'pinia'
import { useHttpClient } from '@/composables/useHttpClient'
import type { ReportAnswer } from '@/stores/stats'
import type { Period } from '@/utils/period'

export type NodeKind = 'anchor' | 'page' | 'exit' | 'entrance' | 'other'

export interface JourneyNode {
  id: number
  level: number
  kind: NodeKind
  label: string
  visits: number
  visitors: number | null
}

export interface JourneyLink {
  source: number
  target: number
  visits: number
}

export interface Journey {
  anchor: { type: 'entry_page' | 'goal'; label: string }
  nodes: JourneyNode[]
  links: JourneyLink[]
  meta: {
    date_from: string
    date_to: string
    depth: number
    width: number
    min_visitors: number
    hostnames: string[]
  }
}

export interface JourneyRequest {
  date_from: string
  date_to: string
  entry_page?: string
  goal_id?: string
  depth?: number
  width?: number
}

export const useJourneysStore = defineStore('journeys', () => {
  async function fetchJourney(orgId: string, siteId: string, request: JourneyRequest) {
    const { post } = useHttpClient()
    return post<Journey>(`/org/${orgId}/site/${siteId}/journey/stats`, request)
  }

  /** The site's most common entry pages in a period, to pick from: the dashboard's own report. */
  async function fetchEntryPages(orgId: string, siteId: string, period: Period) {
    const { post } = useHttpClient()
    const answer = await post<ReportAnswer>(`/org/${orgId}/site/${siteId}/query`, {
      date_from: period.from,
      date_to: period.to,
      dimensions: ['entry_page'],
      metrics: ['visits'],
      limit: 25,
    })
    return answer.rows.map((r) => ({ path: r.dimensions.entry_page, visits: r.metrics.visits ?? 0 }))
  }

  return { fetchJourney, fetchEntryPages }
})
