<template>
  <v-app-bar flat border="b" density="comfortable">
    <v-app-bar-nav-icon @click="drawer = !drawer" />

    <v-toolbar-title class="d-flex align-center">
      <img src="@/assets/logo.svg" alt="Purestat" height="28" class="mr-2" />
      <span class="font-weight-bold text-primary">Purestat</span>
    </v-toolbar-title>

    <v-spacer />

    <!-- Site selector -->
    <v-select
      v-if="siteStore.sites.length > 0 && orgStore.currentOrg"
      v-model="selectedSiteId"
      :items="siteStore.sites"
      item-title="domain"
      item-value="id"
      density="compact"
      variant="outlined"
      hide-details
      style="max-width: 250px"
      class="mr-3"
      @update:model-value="onSiteChange"
    />

    <v-btn icon @click="appStore.toggleDarkMode()">
      <v-icon>{{ appStore.darkMode ? 'mdi-weather-sunny' : 'mdi-weather-night' }}</v-icon>
    </v-btn>

    <v-menu>
      <template v-slot:activator="{ props }">
        <v-btn icon v-bind="props">
          <v-avatar size="32" color="primary">
            <span class="text-white text-body-2">
              {{ appStore.user?.display_name?.[0]?.toUpperCase() || '?' }}
            </span>
          </v-avatar>
        </v-btn>
      </template>
      <v-list density="compact" min-width="200">
        <v-list-item>
          <v-list-item-title class="font-weight-medium">{{ appStore.user?.display_name }}</v-list-item-title>
          <v-list-item-subtitle>{{ appStore.user?.email }}</v-list-item-subtitle>
        </v-list-item>
        <v-divider />
        <v-list-item prepend-icon="mdi-cog" to="/settings">Settings</v-list-item>
        <v-list-item prepend-icon="mdi-logout" @click="handleLogout">Logout</v-list-item>
      </v-list>
    </v-menu>
  </v-app-bar>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'

const appStore = useAppStore()
const orgStore = useOrgStore()
const siteStore = useSiteStore()
const router = useRouter()
const route = useRoute()

const drawer = defineModel<boolean>('drawer', { default: true })
const selectedSiteId = ref<string | null>(null)

watch(
  () => siteStore.currentSite,
  (site) => {
    selectedSiteId.value = site?.id || null
  },
  { immediate: true },
)

function onSiteChange(siteId: string) {
  if (orgStore.currentOrg) {
    router.push({ name: 'dashboard', params: { orgId: orgStore.currentOrg.id, siteId } })
  }
}

function handleLogout() {
  appStore.logout()
  router.push({ name: 'login' })
}
</script>
