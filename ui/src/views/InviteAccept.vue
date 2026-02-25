<template>
  <div class="d-flex justify-center align-center" style="min-height: 60vh">
    <v-card class="pa-8 text-center" max-width="450" width="100%">
      <template v-if="loading">
        <v-progress-circular indeterminate color="primary" size="48" class="mb-4" />
        <p class="text-body-1">{{ $t('common.loading') }}</p>
      </template>

      <template v-else-if="error">
        <v-icon size="64" color="error" class="mb-4">mdi-alert-circle-outline</v-icon>
        <p class="text-h6 mb-2">Invalid Invite</p>
        <p class="text-body-2 text-medium-emphasis mb-6">{{ error }}</p>
        <v-btn color="primary" :to="{ name: 'orgs' }">Go to Dashboard</v-btn>
      </template>

      <template v-else-if="invite">
        <v-icon size="64" color="primary" class="mb-4">mdi-account-group</v-icon>
        <p class="text-h6 mb-2">You've been invited!</p>
        <p class="text-body-2 text-medium-emphasis mb-6">
          Join <strong>{{ invite.org_name }}</strong> as a member.
        </p>
        <v-btn color="primary" block :loading="accepting" @click="handleAccept">
          Accept Invite
        </v-btn>
      </template>

      <template v-else-if="accepted">
        <v-icon size="64" color="secondary" class="mb-4">mdi-check-circle-outline</v-icon>
        <p class="text-h6 mb-2">Welcome aboard!</p>
        <p class="text-body-2 text-medium-emphasis mb-6">
          You've joined the organization successfully.
        </p>
        <v-btn color="primary" :to="{ name: 'orgs' }">Go to Dashboard</v-btn>
      </template>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useHttpClient } from '@/composables/useHttpClient'

interface InviteInfo {
  org_name: string
  inviter_name: string
  role: string
}

const route = useRoute()
const code = route.params.code as string

const invite = ref<InviteInfo | null>(null)
const loading = ref(true)
const error = ref('')
const accepting = ref(false)
const accepted = ref(false)

onMounted(async () => {
  try {
    const { get } = useHttpClient()
    invite.value = await get<InviteInfo>(`/invite/${code}`)
  } catch (e: any) {
    error.value = e.message || 'This invite is invalid or has expired.'
  } finally {
    loading.value = false
  }
})

async function handleAccept() {
  accepting.value = true
  try {
    const { post } = useHttpClient()
    await post(`/invite/${code}/accept`, {})
    invite.value = null
    accepted.value = true
  } catch (e: any) {
    error.value = e.message || 'Failed to accept invite.'
  } finally {
    accepting.value = false
  }
}
</script>
