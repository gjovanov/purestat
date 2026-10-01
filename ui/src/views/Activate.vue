<template>
  <v-container class="fill-height" fluid>
    <v-row justify="center" align="center">
      <v-col cols="12" sm="8" md="5" lg="4">
        <v-card class="pa-8 text-center">
          <template v-if="loading">
            <v-progress-circular indeterminate color="primary" size="48" class="mb-4" />
            <p class="text-body-1">Activating your account...</p>
          </template>

          <template v-else-if="success">
            <v-icon size="64" color="success" class="mb-4">mdi-check-circle-outline</v-icon>
            <p class="text-h6 mb-2">Account Activated!</p>
            <p class="text-body-2 text-medium-emphasis mb-6">
              Your account is now active. You can sign in.
            </p>
            <v-btn color="primary" block to="/login">Sign In</v-btn>
          </template>

          <template v-else>
            <v-icon size="64" color="error" class="mb-4">mdi-alert-circle-outline</v-icon>
            <p class="text-h6 mb-2">Activation Failed</p>
            <p class="text-body-2 text-medium-emphasis mb-6">{{ error }}</p>
            <v-btn color="primary" block to="/login">Back to Login</v-btn>
          </template>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

const loading = ref(true)
const success = ref(false)
const error = ref('')

onMounted(async () => {
  const userId = route.query.userId as string
  const token = route.query.token as string

  if (!userId || !token) {
    error.value = 'Invalid activation link.'
    loading.value = false
    return
  }

  try {
    const resp = await fetch('/api/auth/activate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ user_id: userId, token }),
    })
    const data = await resp.json()
    if (!resp.ok) throw new Error(data.message || 'Activation failed')
    success.value = true
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Activation failed. The link may have expired.'
  } finally {
    loading.value = false
  }
})
</script>
