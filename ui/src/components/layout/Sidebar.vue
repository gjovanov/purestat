<template>
  <v-navigation-drawer v-model="drawer" width="260" border="r">
    <!-- Org selector -->
    <v-list density="compact" class="pa-2">
      <v-select
        v-model="selectedOrgId"
        :items="orgStore.orgs"
        item-title="name"
        item-value="id"
        density="compact"
        variant="outlined"
        hide-details
        label="Organization"
        @update:model-value="onOrgChange"
      />
    </v-list>

    <v-divider />

    <v-list density="compact" nav>
      <template v-if="orgStore.currentOrg && siteStore.currentSite">
        <v-list-item
          prepend-icon="mdi-chart-line"
          title="Dashboard"
          :to="dashboardRoute"
        />
        <v-list-item
          prepend-icon="mdi-bullseye-arrow"
          title="Goals"
          :to="{ name: 'goals', params: routeParams }"
        />
        <v-list-item
          prepend-icon="mdi-key"
          title="API Keys"
          :to="{ name: 'api-keys', params: routeParams }"
        />
        <v-list-item
          prepend-icon="mdi-cog"
          title="Site Settings"
          :to="{ name: 'site-settings', params: routeParams }"
        />
      </template>

      <v-divider class="my-2" />

      <template v-if="orgStore.currentOrg">
        <v-list-item
          prepend-icon="mdi-web"
          title="Sites"
          :to="{ name: 'sites', params: { orgId: orgStore.currentOrg.id } }"
        />
        <v-list-item
          prepend-icon="mdi-account-group"
          title="Members"
          :to="{ name: 'members', params: { orgId: orgStore.currentOrg.id } }"
        />
        <v-list-item
          prepend-icon="mdi-credit-card"
          title="Billing"
          :to="{ name: 'billing', params: { orgId: orgStore.currentOrg.id } }"
        />
        <v-list-item
          prepend-icon="mdi-office-building-cog"
          title="Org Settings"
          :to="{ name: 'org-settings', params: { orgId: orgStore.currentOrg.id } }"
        />
      </template>
    </v-list>

    <template v-slot:append>
      <v-list density="compact" nav>
        <v-list-item
          prepend-icon="mdi-plus"
          title="New Organization"
          @click="$router.push({ name: 'orgs' })"
        />
      </v-list>
    </template>
  </v-navigation-drawer>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'

const orgStore = useOrgStore()
const siteStore = useSiteStore()
const router = useRouter()

const drawer = defineModel<boolean>({ default: true })

const selectedOrgId = ref<string | null>(null)

const routeParams = computed(() => ({
  orgId: orgStore.currentOrg?.id || '',
  siteId: siteStore.currentSite?.id || '',
}))

const dashboardRoute = computed(() => ({
  name: 'dashboard',
  params: routeParams.value,
}))

watch(
  () => orgStore.currentOrg,
  (org) => {
    selectedOrgId.value = org?.id || null
  },
  { immediate: true },
)

function onOrgChange(orgId: string) {
  orgStore.selectOrg(orgId)
  siteStore.fetchSites(orgId)
  router.push({ name: 'sites', params: { orgId } })
}
</script>
