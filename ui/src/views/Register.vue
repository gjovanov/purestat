<template>
  <v-container class="fill-height" fluid>
    <v-row justify="center" align="center">
      <v-col cols="12" sm="8" md="5" lg="4">
        <div class="text-center mb-8">
          <img src="@/assets/logo.svg" alt="Purestat" height="48" class="mb-4" />
          <h1 class="text-h5 font-weight-bold">{{ $t('auth.registerTitle') }}</h1>
          <p class="text-body-2 text-medium-emphasis">{{ $t('auth.registerSubtitle') }}</p>
        </div>

        <v-card class="pa-6">
          <v-form @submit.prevent="handleRegister">
            <v-text-field
              v-model="email"
              :label="$t('auth.email')"
              type="email"
              prepend-inner-icon="mdi-email-outline"
              required
              autofocus
              class="mb-2"
            />
            <v-text-field
              v-model="username"
              :label="$t('auth.username')"
              prepend-inner-icon="mdi-account-outline"
              required
              class="mb-2"
            />
            <v-text-field
              v-model="displayName"
              :label="$t('auth.displayName')"
              prepend-inner-icon="mdi-badge-account-outline"
              class="mb-2"
            />
            <v-text-field
              v-model="password"
              :label="$t('auth.password')"
              :type="showPassword ? 'text' : 'password'"
              prepend-inner-icon="mdi-lock-outline"
              :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
              @click:append-inner="showPassword = !showPassword"
              required
              class="mb-4"
            />

            <v-alert v-if="error" type="error" density="compact" class="mb-4">
              {{ error }}
            </v-alert>

            <v-btn
              type="submit"
              color="primary"
              block
              size="large"
              :loading="loading"
            >
              Create Account
            </v-btn>
          </v-form>

          <div class="text-center mt-4">
            <span class="text-body-2 text-medium-emphasis">{{ $t('auth.hasAccount') }}</span>
            <router-link to="/login" class="text-primary ml-1">{{ $t('nav.login') }}</router-link>
          </div>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'

const appStore = useAppStore()
const router = useRouter()

const email = ref('')
const username = ref('')
const displayName = ref('')
const password = ref('')
const showPassword = ref(false)
const loading = ref(false)
const error = ref('')

async function handleRegister() {
  loading.value = true
  error.value = ''
  try {
    await appStore.register(email.value, username.value, password.value, displayName.value || undefined)
    router.push('/orgs')
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Registration failed'
  } finally {
    loading.value = false
  }
}
</script>
