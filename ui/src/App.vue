<template>
  <v-app>
    <template v-if="route.meta.layout === 'blank'">
      <v-main>
        <router-view />
      </v-main>
    </template>
    <template v-else>
      <AppBar />
      <Sidebar />
      <v-main>
        <v-container fluid class="pa-6">
          <router-view />
        </v-container>
      </v-main>
    </template>
    <v-snackbar
      v-model="snackbar.state.show"
      :color="snackbar.state.color"
      :timeout="snackbar.state.timeout"
      location="bottom right"
    >
      {{ snackbar.state.text }}
      <template #actions>
        <v-btn variant="text" @click="snackbar.hideSnackbar()">Close</v-btn>
      </template>
    </v-snackbar>
  </v-app>
</template>

<script setup lang="ts">
import { useRoute } from 'vue-router'
import { onMounted, watch } from 'vue'
import { useTheme } from 'vuetify'
import { useAppStore } from '@/stores/app'
import { useSnackbar } from '@/composables/useSnackbar'
import AppBar from '@/components/layout/AppBar.vue'
import Sidebar from '@/components/layout/Sidebar.vue'

const snackbar = useSnackbar()

const route = useRoute()
const appStore = useAppStore()
const theme = useTheme()

watch(
  () => appStore.darkMode,
  (dark) => {
    theme.global.name.value = dark ? 'purestatDark' : 'purestatLight'
  },
  { immediate: true },
)

onMounted(() => {
  appStore.init()
})
</script>
