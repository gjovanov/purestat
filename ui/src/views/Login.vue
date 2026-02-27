<template>
  <v-container class="fill-height" fluid>
    <v-row justify="center" align="center">
      <v-col cols="12" sm="8" md="5" lg="4">
        <div class="text-center mb-8">
          <img src="@/assets/logo.svg" alt="Purestat" height="48" class="mb-4" />
          <h1 class="text-h5 font-weight-bold">{{ $t('auth.loginTitle') }}</h1>
          <p class="text-body-2 text-medium-emphasis">{{ $t('auth.loginSubtitle') }}</p>
        </div>

        <v-card class="pa-6">
          <v-form ref="formRef" @submit.prevent="handleLogin">
            <v-text-field
              v-model="email"
              :label="$t('auth.email')"
              type="email"
              prepend-inner-icon="mdi-email-outline"
              :rules="[rules.required, rules.email]"
              autofocus
              class="mb-2"
            />
            <v-text-field
              v-model="password"
              :label="$t('auth.password')"
              :type="showPassword ? 'text' : 'password'"
              prepend-inner-icon="mdi-lock-outline"
              :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
              @click:append-inner="showPassword = !showPassword"
              :rules="[rules.required]"
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
              Sign In
            </v-btn>
          </v-form>

          <div class="text-center mt-4">
            <span class="text-body-2 text-medium-emphasis">{{ $t('auth.noAccount') }}</span>
            <router-link to="/register" class="text-primary ml-1">{{ $t('nav.register') }}</router-link>
          </div>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useValidation } from '@/composables/useValidation'

const appStore = useAppStore()
const router = useRouter()
const route = useRoute()
const { rules } = useValidation()

const formRef = ref()
const email = ref('')
const password = ref('')
const showPassword = ref(false)
const loading = ref(false)
const error = ref('')

async function handleLogin() {
  const { valid } = await formRef.value.validate()
  if (!valid) return
  loading.value = true
  error.value = ''
  try {
    await appStore.login(email.value, password.value)
    const redirect = route.query.redirect as string
    router.push(redirect || '/orgs')
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Login failed'
  } finally {
    loading.value = false
  }
}
</script>
