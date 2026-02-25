import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useHttpClient } from '@/composables/useHttpClient'

interface PlanInfo {
  name: string
  price: string
  pageviews: string
  sites: string
  members: string
}

export const useBillingStore = defineStore('billing', () => {
  const plans = ref<PlanInfo[]>([])
  const loading = ref(false)

  async function fetchPlans() {
    const { get } = useHttpClient()
    loading.value = true
    try {
      const data = await get<{ plans: PlanInfo[] }>('/stripe/plans')
      plans.value = data.plans
    } finally {
      loading.value = false
    }
  }

  async function createCheckout(orgId: string, plan: string) {
    const { post } = useHttpClient()
    const data = await post<{ url: string }>('/stripe/checkout', {
      org_id: orgId,
      plan,
      success_url: `${window.location.origin}/org/${orgId}/billing?success=true`,
      cancel_url: `${window.location.origin}/org/${orgId}/billing?canceled=true`,
    })
    window.location.href = data.url
  }

  async function openPortal(orgId: string) {
    const { post } = useHttpClient()
    const data = await post<{ url: string }>('/stripe/portal', {
      org_id: orgId,
      return_url: `${window.location.origin}/org/${orgId}/billing`,
    })
    window.location.href = data.url
  }

  return { plans, loading, fetchPlans, createCheckout, openPortal }
})
