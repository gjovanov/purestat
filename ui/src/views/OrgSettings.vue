<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">Organization Settings</h1>
    </div>

    <v-card class="pa-6 mb-6" v-if="orgStore.currentOrg">
      <v-form @submit.prevent="handleSave">
        <v-text-field v-model="orgName" :label="$t('org.name')" class="mb-2" />
        <v-text-field
          :model-value="orgStore.currentOrg.slug"
          :label="$t('org.slug')"
          disabled
          class="mb-2"
        />
        <v-text-field
          :model-value="orgStore.currentOrg.plan"
          :label="$t('org.plan')"
          disabled
          class="mb-4"
        />
        <v-btn type="submit" color="primary" :loading="saving">{{ $t('common.save') }}</v-btn>
      </v-form>
    </v-card>

    <v-card class="pa-6">
      <v-card-title class="text-h6 pa-0 mb-2 text-error">Danger Zone</v-card-title>
      <p class="text-body-2 text-medium-emphasis mb-4">
        Deleting an organization removes all sites, analytics data, and members permanently.
      </p>
      <v-btn color="error" variant="outlined" @click="handleDelete">
        <v-icon start>mdi-delete-outline</v-icon>
        Delete Organization
      </v-btn>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useOrgStore } from '@/stores/org'

const route = useRoute()
const router = useRouter()
const orgStore = useOrgStore()

const orgId = route.params.orgId as string
const orgName = ref('')
const saving = ref(false)

onMounted(async () => {
  await orgStore.fetchOrg(orgId)
  if (orgStore.currentOrg) {
    orgName.value = orgStore.currentOrg.name
  }
})

async function handleSave() {
  saving.value = true
  try {
    await orgStore.updateOrg(orgId, orgName.value)
  } finally {
    saving.value = false
  }
}

async function handleDelete() {
  if (confirm('Are you sure? This will permanently delete the organization and all its data.')) {
    await orgStore.deleteOrg(orgId)
    router.push({ name: 'orgs' })
  }
}
</script>
