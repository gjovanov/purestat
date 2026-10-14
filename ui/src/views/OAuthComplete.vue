<template>
  <v-container class="fill-height" fluid>
    <v-row justify="center" align="center">
      <v-col cols="12" sm="8" md="5" lg="4">
        <v-card class="pa-8 text-center">
          <template v-if="failed">
            <v-icon size="64" color="error" class="mb-4">mdi-alert-circle-outline</v-icon>
            <p class="text-h6 mb-2">{{ $t('auth.oauth.failedTitle') }}</p>
            <p class="text-body-2 text-medium-emphasis mb-6">{{ $t('auth.oauth.failedBody') }}</p>
            <v-btn color="primary" block to="/login">{{ $t('auth.oauth.backToLogin') }}</v-btn>
          </template>
          <template v-else>
            <v-progress-circular indeterminate color="primary" size="48" class="mb-4" />
            <p class="text-body-1">{{ $t('auth.oauth.signingIn') }}</p>
          </template>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
// Where a Google or GitHub sign-in lands (/auth/oauth). The server has set
// the session as an HttpOnly cookie; this page loads the user with it. The
// address carries nothing: there is nothing here to read from it.
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { NEXT_KEY, safeNext } from '@/utils/oauth'

const appStore = useAppStore()
const router = useRouter()
const failed = ref(false)

function takeNext(): string {
  try {
    const next = sessionStorage.getItem(NEXT_KEY)
    sessionStorage.removeItem(NEXT_KEY)
    return safeNext(next)
  } catch {
    return safeNext(null)
  }
}

onMounted(async () => {
  if (await appStore.resumeCookieSession()) {
    router.replace(takeNext())
  } else {
    failed.value = true
  }
})
</script>
