<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">{{ $t('nav.apiKeys') }}</h1>
      <v-spacer />
      <v-btn color="primary" prepend-icon="mdi-plus" @click="showDialog = true">
        Create API Key
      </v-btn>
    </div>

    <v-alert v-if="newKey" type="success" class="mb-6" closable @click:close="newKey = null">
      <p class="font-weight-bold mb-1">Your new API key (copy it now, it won't be shown again):</p>
      <v-sheet color="surface-variant" rounded="lg" class="pa-3 d-flex align-center">
        <code class="flex-grow-1">{{ newKey }}</code>
        <v-btn icon size="small" variant="text" @click="copyKey">
          <v-icon>{{ copied ? 'mdi-check' : 'mdi-content-copy' }}</v-icon>
        </v-btn>
      </v-sheet>
    </v-alert>

    <v-card v-if="apiKeys.length > 0">
      <v-list>
        <v-list-item v-for="key in apiKeys" :key="key.id">
          <template v-slot:prepend>
            <v-icon color="primary">mdi-key-variant</v-icon>
          </template>
          <v-list-item-title>{{ key.name }}</v-list-item-title>
          <v-list-item-subtitle>
            {{ key.key_prefix }}... &middot; {{ key.scopes.join(', ') }}
            <span v-if="key.last_used_at"> &middot; Last used {{ formatDate(key.last_used_at) }}</span>
          </v-list-item-subtitle>
          <template v-slot:append>
            <v-btn
              icon
              size="small"
              color="error"
              variant="text"
              :disabled="!!key.revoked_at"
              @click="handleRevoke(key.id)"
            >
              <v-icon>mdi-close-circle-outline</v-icon>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>
    </v-card>

    <v-card v-else class="pa-8 text-center">
      <v-icon size="64" color="secondary" class="mb-4">mdi-key-variant</v-icon>
      <p class="text-body-1 text-medium-emphasis">No API keys yet</p>
    </v-card>

    <!-- Create API Key Dialog -->
    <v-dialog v-model="showDialog" max-width="500">
      <v-card class="pa-6">
        <v-card-title class="text-h6 pa-0 mb-4">Create API Key</v-card-title>
        <v-form ref="formRef" @submit.prevent="handleCreate">
          <v-text-field v-model="keyName" label="Key name" placeholder="My integration" :rules="[rules.required]" class="mb-2" />
          <v-select
            v-model="keyScopes"
            :items="scopeOptions"
            label="Scopes"
            multiple
            chips
            class="mb-4"
          />
          <div class="d-flex justify-end ga-2">
            <v-btn variant="text" @click="showDialog = false">{{ $t('common.cancel') }}</v-btn>
            <v-btn type="submit" color="primary" :loading="creating">{{ $t('common.create') }}</v-btn>
          </div>
        </v-form>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useHttpClient } from '@/composables/useHttpClient'
import { useSnackbar } from '@/composables/useSnackbar'
import { useValidation } from '@/composables/useValidation'

interface ApiKey {
  id: string
  name: string
  key_prefix: string
  scopes: string[]
  last_used_at: string | null
  revoked_at: string | null
}

const route = useRoute()
const orgId = route.params.orgId as string
const siteId = route.params.siteId as string

const { showSuccess, showError } = useSnackbar()
const { rules } = useValidation()

const formRef = ref()
const apiKeys = ref<ApiKey[]>([])
const showDialog = ref(false)
const keyName = ref('')
const keyScopes = ref(['stats:read'])
const creating = ref(false)
const newKey = ref<string | null>(null)
const copied = ref(false)

const scopeOptions = ['stats:read', 'events:write']

onMounted(fetchKeys)

async function fetchKeys() {
  const { get } = useHttpClient()
  apiKeys.value = await get<ApiKey[]>(`/org/${orgId}/site/${siteId}/api-key`)
}

async function handleCreate() {
  const { valid } = await formRef.value.validate()
  if (!valid) return
  creating.value = true
  try {
    const { post } = useHttpClient()
    const result = await post<{ key: string; api_key: ApiKey }>(`/org/${orgId}/site/${siteId}/api-key`, {
      name: keyName.value,
      scopes: keyScopes.value,
    })
    newKey.value = result.key
    apiKeys.value.push(result.api_key)
    showDialog.value = false
    keyName.value = ''
    keyScopes.value = ['stats:read']
    showSuccess('API key created')
  } catch (e) {
    showError(e instanceof Error ? e.message : 'Failed to create API key')
  } finally {
    creating.value = false
  }
}

async function handleRevoke(keyId: string) {
  if (confirm('Revoke this API key? This cannot be undone.')) {
    try {
      const { del } = useHttpClient()
      await del(`/org/${orgId}/site/${siteId}/api-key/${keyId}`)
      apiKeys.value = apiKeys.value.filter((k) => k.id !== keyId)
      showSuccess('API key revoked')
    } catch (e) {
      showError(e instanceof Error ? e.message : 'Failed to revoke API key')
    }
  }
}

function copyKey() {
  if (newKey.value) {
    navigator.clipboard.writeText(newKey.value)
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  }
}

function formatDate(dateStr: string) {
  return new Date(dateStr).toLocaleDateString()
}
</script>
