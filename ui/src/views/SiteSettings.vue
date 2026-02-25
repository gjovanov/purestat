<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">{{ $t('nav.settings') }}</h1>
    </div>

    <v-card v-if="site" class="pa-6 mb-6">
      <v-card-title class="text-h6 pa-0 mb-4">{{ $t('site.name') }}</v-card-title>
      <v-form @submit.prevent="handleSave">
        <v-text-field v-model="siteName" :label="$t('site.name')" class="mb-2" />
        <v-text-field v-model="siteDomain" :label="$t('site.domain')" disabled class="mb-2" />
        <v-text-field v-model="siteTimezone" :label="$t('site.timezone')" class="mb-2" />
        <v-switch v-model="isPublic" label="Public dashboard" color="primary" class="mb-2" />
        <v-textarea
          v-model="allowedHostnames"
          label="Allowed hostnames (one per line)"
          hint="Leave empty to allow all"
          rows="3"
          class="mb-4"
        />
        <v-btn type="submit" color="primary" :loading="saving">{{ $t('common.save') }}</v-btn>
      </v-form>
    </v-card>

    <v-card class="pa-6 mb-6">
      <v-card-title class="text-h6 pa-0 mb-2">{{ $t('site.trackingCode') }}</v-card-title>
      <p class="text-body-2 text-medium-emphasis mb-4">{{ $t('site.trackingInstructions') }}</p>
      <v-sheet color="surface-variant" rounded="lg" class="pa-4">
        <code class="text-body-2">{{ trackingSnippet }}</code>
      </v-sheet>
      <v-btn variant="text" size="small" class="mt-2" @click="copySnippet">
        <v-icon start>mdi-content-copy</v-icon>
        {{ copied ? $t('common.copied') : $t('common.copy') }}
      </v-btn>
    </v-card>

    <v-card class="pa-6">
      <v-card-title class="text-h6 pa-0 mb-2 text-error">Danger Zone</v-card-title>
      <p class="text-body-2 text-medium-emphasis mb-4">Deleting a site removes all analytics data permanently.</p>
      <v-btn color="error" variant="outlined" @click="handleDelete">
        <v-icon start>mdi-delete-outline</v-icon>
        Delete Site
      </v-btn>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useSiteStore } from '@/stores/site'

const route = useRoute()
const router = useRouter()
const siteStore = useSiteStore()

const orgId = route.params.orgId as string
const siteId = route.params.siteId as string

const siteName = ref('')
const siteDomain = ref('')
const siteTimezone = ref('UTC')
const isPublic = ref(false)
const allowedHostnames = ref('')
const saving = ref(false)
const copied = ref(false)

const site = computed(() => siteStore.currentSite)

const trackingSnippet = computed(
  () =>
    `<script defer data-domain="${siteDomain.value}" src="https://purestat.ai/js/purestat.js"><\/script>`,
)

onMounted(async () => {
  const s = await siteStore.fetchSite(orgId, siteId)
  if (s) {
    siteName.value = s.name
    siteDomain.value = s.domain
    siteTimezone.value = s.timezone
    isPublic.value = s.is_public
    allowedHostnames.value = s.allowed_hostnames.join('\n')
  }
})

async function handleSave() {
  saving.value = true
  try {
    await siteStore.updateSite(orgId, siteId, {
      name: siteName.value,
      timezone: siteTimezone.value,
      is_public: isPublic.value,
      allowed_hostnames: allowedHostnames.value
        .split('\n')
        .map((h) => h.trim())
        .filter(Boolean),
    })
  } finally {
    saving.value = false
  }
}

function copySnippet() {
  navigator.clipboard.writeText(trackingSnippet.value)
  copied.value = true
  setTimeout(() => (copied.value = false), 2000)
}

async function handleDelete() {
  if (confirm('Are you sure? This will permanently delete all data for this site.')) {
    await siteStore.deleteSite(orgId, siteId)
    router.push({ name: 'sites', params: { orgId } })
  }
}
</script>
