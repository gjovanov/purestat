<template>
  <div v-if="providers.length" class="mb-2">
    <v-btn
      v-for="provider in providers"
      :key="provider"
      :href="startPath(provider)"
      :prepend-icon="icons[provider]"
      variant="outlined"
      size="large"
      block
      class="mb-3 text-none"
      @click="rememberNext"
    >
      {{ $t(labels[provider]) }}
    </v-btn>
    <div class="d-flex align-center mb-4">
      <v-divider />
      <span class="mx-3 text-caption text-medium-emphasis text-no-wrap">
        {{ $t('auth.oauth.orEmail') }}
      </span>
      <v-divider />
    </div>
  </div>
</template>

<script setup lang="ts">
// "Continue with Google / GitHub", only for the providers the server says
// work here: a self-hosted install without OAuth shows no buttons at all.
import { onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { NEXT_KEY, parseProviders, safeNext, startPath, type OAuthProvider } from '@/utils/oauth'

const route = useRoute()
const providers = ref<OAuthProvider[]>([])

const icons: Record<OAuthProvider, string> = {
  google: 'mdi-google',
  github: 'mdi-github',
}
const labels: Record<OAuthProvider, string> = {
  google: 'auth.oauth.continueWithGoogle',
  github: 'auth.oauth.continueWithGithub',
}

onMounted(async () => {
  try {
    const resp = await fetch('/api/auth/providers', { credentials: 'same-origin' })
    if (resp.ok) providers.value = parseProviders(await resp.json())
  } catch {
    // No buttons; the email form still works.
  }
})

// The page to return to after the round trip through the provider. The
// browser leaves the SPA, so it is kept for this tab only.
function rememberNext() {
  try {
    sessionStorage.setItem(NEXT_KEY, safeNext(route.query.redirect))
  } catch {
    // Storage blocked: the sign-in lands on the default page.
  }
}
</script>
