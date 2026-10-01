<template>
  <v-app-bar flat border="b" density="comfortable">
    <v-app-bar-nav-icon @click="drawer = !drawer" />

    <v-toolbar-title class="d-flex align-center">
      <img src="@/assets/logo.svg" alt="Purestat" height="28" class="mr-2" />
      <span class="font-weight-bold text-primary">Purestat</span>
    </v-toolbar-title>

    <v-spacer />

    <!-- Multi-site selector -->
    <v-select
      v-if="siteStore.sites.length > 0 && orgStore.currentOrg"
      v-model="localSelectedIds"
      :items="siteStore.sites"
      item-title="domain"
      item-value="id"
      density="compact"
      variant="outlined"
      hide-details
      multiple
      closable-chips
      style="max-width: 350px"
      class="mr-3"
      @update:model-value="onSitesChange"
    >
      <template v-slot:prepend-item>
        <v-list-item title="Select All" @click="toggleSelectAll">
          <template v-slot:prepend>
            <v-checkbox-btn
              :model-value="allSelected"
              :indeterminate="someSelected && !allSelected"
            />
          </template>
        </v-list-item>
        <v-divider />
      </template>
      <template v-slot:selection="{ item, index }">
        <v-chip v-if="index < 2" size="small" closable @click:close="removeSite(item.value)">
          {{ item.title }}
        </v-chip>
        <span v-if="index === 2" class="text-caption ml-1">
          +{{ localSelectedIds.length - 2 }} more
        </span>
      </template>
    </v-select>

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
import { ref, computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'

const appStore = useAppStore()
const orgStore = useOrgStore()
const siteStore = useSiteStore()
const router = useRouter()

const drawer = defineModel<boolean>('drawer', { default: true })
const localSelectedIds = ref<string[]>([])

const allSelected = computed(() =>
  localSelectedIds.value.length === siteStore.sites.length && siteStore.sites.length > 0,
)
const someSelected = computed(() => localSelectedIds.value.length > 0)

watch(
  () => siteStore.selectedSiteIds,
  (ids) => {
    localSelectedIds.value = [...ids]
  },
  { immediate: true },
)

function onSitesChange(ids: string[]) {
  siteStore.setSelectedSiteIds(ids)
  if (orgStore.currentOrg) {
    router.push({ name: 'org-dashboard', params: { orgId: orgStore.currentOrg.id } })
  }
}

function toggleSelectAll() {
  if (allSelected.value) {
    localSelectedIds.value = []
  } else {
    localSelectedIds.value = siteStore.sites.map((s) => s.id)
  }
  onSitesChange(localSelectedIds.value)
}

function removeSite(siteId: string) {
  localSelectedIds.value = localSelectedIds.value.filter((id) => id !== siteId)
  onSitesChange(localSelectedIds.value)
}

function handleLogout() {
  appStore.logout()
  router.push({ name: 'login' })
}
</script>
