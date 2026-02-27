<template>
  <div>
    <div class="d-flex align-center mb-6">
      <h1 class="text-h5 font-weight-bold">Account Settings</h1>
    </div>

    <v-card class="pa-6 mb-6" v-if="appStore.user">
      <v-card-title class="text-h6 pa-0 mb-4">Profile</v-card-title>
      <v-form @submit.prevent="handleSave">
        <v-text-field
          :model-value="appStore.user.email"
          label="Email"
          disabled
          class="mb-2"
        />
        <v-text-field
          :model-value="appStore.user.username"
          label="Username"
          disabled
          class="mb-2"
        />
        <v-text-field v-model="displayName" label="Display name" class="mb-4" />
        <v-btn type="submit" color="primary" :loading="saving">{{ $t('common.save') }}</v-btn>
      </v-form>
    </v-card>

    <v-card class="pa-6">
      <v-card-title class="text-h6 pa-0 mb-4">Preferences</v-card-title>
      <v-switch
        :model-value="appStore.darkMode"
        label="Dark mode"
        color="primary"
        @update:model-value="appStore.toggleDarkMode()"
      />
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useAppStore } from '@/stores/app'
import { useSnackbar } from '@/composables/useSnackbar'

const appStore = useAppStore()
const { showSuccess, showError } = useSnackbar()

const displayName = ref('')
const saving = ref(false)

onMounted(() => {
  if (appStore.user) {
    displayName.value = appStore.user.display_name
  }
})

async function handleSave() {
  saving.value = true
  try {
    await appStore.updateProfile({ display_name: displayName.value })
    showSuccess('Profile updated')
  } catch (e) {
    showError(e instanceof Error ? e.message : 'Failed to update profile')
  } finally {
    saving.value = false
  }
}
</script>
