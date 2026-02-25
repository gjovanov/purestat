<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">{{ $t('nav.billing') }}</h1>
    </div>

    <!-- Current Plan -->
    <v-card class="pa-6 mb-6" v-if="orgStore.currentOrg">
      <div class="d-flex align-center mb-4">
        <div>
          <p class="text-body-2 text-medium-emphasis">{{ $t('billing.currentPlan') }}</p>
          <p class="text-h5 font-weight-bold text-primary text-capitalize">{{ orgStore.currentOrg.plan }}</p>
        </div>
        <v-spacer />
        <v-btn
          v-if="orgStore.currentOrg.plan !== 'free'"
          variant="outlined"
          @click="billingStore.openPortal(orgId)"
        >
          {{ $t('billing.manageBilling') }}
        </v-btn>
      </div>
      <v-divider class="mb-4" />
      <div class="d-flex ga-8">
        <div>
          <p class="text-body-2 text-medium-emphasis">Pageviews this month</p>
          <p class="text-h6">
            {{ orgStore.currentOrg.usage.current_month_pageviews.toLocaleString() }}
            <span class="text-body-2 text-medium-emphasis">
              / {{ orgStore.currentOrg.limits.max_pageviews_monthly.toLocaleString() }}
            </span>
          </p>
        </div>
        <div>
          <p class="text-body-2 text-medium-emphasis">Sites</p>
          <p class="text-h6">{{ orgStore.currentOrg.limits.max_sites }}</p>
        </div>
        <div>
          <p class="text-body-2 text-medium-emphasis">Team members</p>
          <p class="text-h6">{{ orgStore.currentOrg.limits.max_members }}</p>
        </div>
      </div>
    </v-card>

    <!-- Plans -->
    <v-row>
      <v-col v-for="plan in plans" :key="plan.name" cols="12" md="4">
        <v-card
          class="pa-6 h-100 d-flex flex-column"
          :class="{ 'border-primary': plan.name.toLowerCase() === orgStore.currentOrg?.plan }"
        >
          <p class="text-h6 font-weight-bold mb-1 text-capitalize">{{ plan.name }}</p>
          <p class="text-h4 font-weight-bold mb-4">
            {{ plan.price }}
            <span v-if="plan.price !== '$0'" class="text-body-2 text-medium-emphasis">/mo</span>
          </p>
          <v-list density="compact" class="flex-grow-1 mb-4">
            <v-list-item>
              <template v-slot:prepend>
                <v-icon color="secondary" size="small">mdi-check</v-icon>
              </template>
              {{ plan.pageviews }} pageviews/mo
            </v-list-item>
            <v-list-item>
              <template v-slot:prepend>
                <v-icon color="secondary" size="small">mdi-check</v-icon>
              </template>
              {{ plan.sites }} sites
            </v-list-item>
            <v-list-item>
              <template v-slot:prepend>
                <v-icon color="secondary" size="small">mdi-check</v-icon>
              </template>
              {{ plan.members }} team members
            </v-list-item>
          </v-list>
          <v-btn
            v-if="plan.name.toLowerCase() !== orgStore.currentOrg?.plan"
            color="primary"
            block
            @click="billingStore.createCheckout(orgId, plan.name.toLowerCase())"
          >
            {{ $t('billing.upgrade') }}
          </v-btn>
          <v-btn v-else color="primary" variant="outlined" block disabled>
            Current plan
          </v-btn>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useOrgStore } from '@/stores/org'
import { useBillingStore } from '@/stores/billing'

const route = useRoute()
const orgStore = useOrgStore()
const billingStore = useBillingStore()

const orgId = route.params.orgId as string

const plans = computed(() =>
  billingStore.plans.length
    ? billingStore.plans
    : [
        { name: 'Free', price: '$0', pageviews: '10,000', sites: '1', members: '1' },
        { name: 'Pro', price: '$9', pageviews: '100,000', sites: '5', members: '5' },
        { name: 'Business', price: '$29', pageviews: '1,000,000', sites: 'Unlimited', members: 'Unlimited' },
      ],
)

onMounted(() => {
  orgStore.fetchOrg(orgId)
  billingStore.fetchPlans()
})
</script>
