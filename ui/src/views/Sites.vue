<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">Sites</h1>
      <v-spacer />
      <v-btn color="primary" prepend-icon="mdi-plus" @click="showCreateDialog = true">
        {{ $t('site.add') }}
      </v-btn>
    </div>

    <v-row v-if="siteStore.sites.length > 0">
      <v-col cols="12" sm="6" md="4" v-for="site in siteStore.sites" :key="site.id">
        <v-card class="pa-4 cursor-pointer" @click="navigateToSite(site.id)" hover>
          <div class="d-flex align-center mb-2">
            <v-icon color="primary" class="mr-2">mdi-web</v-icon>
            <div class="font-weight-bold">{{ site.domain }}</div>
          </div>
          <div class="text-body-2 text-medium-emphasis">{{ site.name }}</div>
          <div class="d-flex align-center mt-3">
            <v-chip v-if="site.is_public" size="x-small" color="secondary" variant="tonal" class="mr-2">Public</v-chip>
            <span class="text-caption text-medium-emphasis">{{ site.timezone }}</span>
          </div>
        </v-card>
      </v-col>
    </v-row>

    <v-card v-else class="pa-8 text-center">
      <v-icon size="64" color="primary" class="mb-4">mdi-web-plus</v-icon>
      <p class="text-body-1 text-medium-emphasis">{{ $t('site.noSites') }}</p>
      <v-btn color="primary" class="mt-4" @click="showCreateDialog = true">{{ $t('site.add') }}</v-btn>
    </v-card>

    <!-- Create Site Dialog -->
    <v-dialog v-model="showCreateDialog" max-width="500">
      <v-card class="pa-6">
        <v-card-title class="text-h6 pa-0 mb-4">{{ $t('site.add') }}</v-card-title>
        <v-form @submit.prevent="handleCreate">
          <v-text-field v-model="newDomain" :label="$t('site.domain')" placeholder="example.com" required class="mb-2" />
          <v-text-field v-model="newName" :label="$t('site.name')" required class="mb-4" />
          <v-alert v-if="createError" type="error" density="compact" class="mb-4">{{ createError }}</v-alert>
          <div class="d-flex justify-end ga-2">
            <v-btn variant="text" @click="showCreateDialog = false">{{ $t('common.cancel') }}</v-btn>
            <v-btn type="submit" color="primary" :loading="creating">{{ $t('common.create') }}</v-btn>
          </div>
        </v-form>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'

const orgStore = useOrgStore()
const siteStore = useSiteStore()
const router = useRouter()
const route = useRoute()

const showCreateDialog = ref(false)
const newDomain = ref('')
const newName = ref('')
const creating = ref(false)
const createError = ref('')

const orgId = route.params.orgId as string

onMounted(async () => {
  await orgStore.fetchOrg(orgId)
  await siteStore.fetchSites(orgId)
})

function navigateToSite(siteId: string) {
  siteStore.selectSite(siteId)
  router.push({ name: 'dashboard', params: { orgId, siteId } })
}

async function handleCreate() {
  creating.value = true
  createError.value = ''
  try {
    const site = await siteStore.createSite(orgId, newDomain.value, newName.value)
    showCreateDialog.value = false
    newDomain.value = ''
    newName.value = ''
    router.push({ name: 'dashboard', params: { orgId, siteId: site.id } })
  } catch (e) {
    createError.value = e instanceof Error ? e.message : 'Failed to create site'
  } finally {
    creating.value = false
  }
}
</script>
