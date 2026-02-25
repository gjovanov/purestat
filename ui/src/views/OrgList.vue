<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">Organizations</h1>
      <v-spacer />
      <v-btn color="primary" prepend-icon="mdi-plus" @click="showCreateDialog = true">
        {{ $t('org.create') }}
      </v-btn>
    </div>

    <v-row v-if="orgStore.orgs.length > 0">
      <v-col cols="12" sm="6" md="4" v-for="org in orgStore.orgs" :key="org.id">
        <v-card
          class="pa-4 cursor-pointer"
          @click="navigateToOrg(org.id)"
          hover
        >
          <div class="d-flex align-center mb-3">
            <v-avatar color="primary" size="40" class="mr-3">
              <span class="text-white font-weight-bold">{{ org.name[0]?.toUpperCase() }}</span>
            </v-avatar>
            <div>
              <div class="font-weight-bold">{{ org.name }}</div>
              <div class="text-body-2 text-medium-emphasis">{{ org.slug }}</div>
            </div>
          </div>
          <v-chip size="x-small" :color="planColor(org.plan)" class="mr-2">
            {{ org.plan }}
          </v-chip>
          <span class="text-body-2 text-medium-emphasis">
            {{ org.usage.current_month_pageviews.toLocaleString() }} / {{ org.limits.max_pageviews_monthly.toLocaleString() }} pageviews
          </span>
        </v-card>
      </v-col>
    </v-row>

    <v-card v-else class="pa-8 text-center">
      <v-icon size="64" color="primary" class="mb-4">mdi-office-building-plus</v-icon>
      <p class="text-body-1 text-medium-emphasis">{{ $t('org.noOrgs') }}</p>
      <v-btn color="primary" class="mt-4" @click="showCreateDialog = true">
        {{ $t('org.create') }}
      </v-btn>
    </v-card>

    <!-- Create Dialog -->
    <v-dialog v-model="showCreateDialog" max-width="500">
      <v-card class="pa-6">
        <v-card-title class="text-h6 pa-0 mb-4">{{ $t('org.create') }}</v-card-title>
        <v-form @submit.prevent="handleCreate">
          <v-text-field v-model="newOrgName" :label="$t('org.name')" required class="mb-2" />
          <v-text-field v-model="newOrgSlug" :label="$t('org.slug')" required class="mb-4" />
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
import { ref, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useOrgStore } from '@/stores/org'

const orgStore = useOrgStore()
const router = useRouter()

const showCreateDialog = ref(false)
const newOrgName = ref('')
const newOrgSlug = ref('')
const creating = ref(false)
const createError = ref('')

onMounted(() => orgStore.fetchOrgs())

watch(newOrgName, (val) => {
  newOrgSlug.value = val.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
})

function navigateToOrg(orgId: string) {
  orgStore.selectOrg(orgId)
  router.push({ name: 'sites', params: { orgId } })
}

async function handleCreate() {
  creating.value = true
  createError.value = ''
  try {
    const org = await orgStore.createOrg(newOrgName.value, newOrgSlug.value)
    showCreateDialog.value = false
    newOrgName.value = ''
    newOrgSlug.value = ''
    router.push({ name: 'sites', params: { orgId: org.id } })
  } catch (e) {
    createError.value = e instanceof Error ? e.message : 'Failed to create organization'
  } finally {
    creating.value = false
  }
}

function planColor(plan: string): string {
  switch (plan) {
    case 'pro': return 'primary'
    case 'business': return 'secondary'
    default: return 'default'
  }
}
</script>
