<template>
  <v-btn-toggle v-model="selected" density="compact" variant="outlined" divided mandatory>
    <v-btn value="1d" size="small">Today</v-btn>
    <v-btn value="7d" size="small">7D</v-btn>
    <v-btn value="30d" size="small">30D</v-btn>
    <v-btn value="6mo" size="small">6M</v-btn>
    <v-btn value="12mo" size="small">12M</v-btn>
  </v-btn-toggle>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

const emit = defineEmits<{ change: [range: string] }>()
const props = defineProps<{ modelValue: string }>()

const selected = ref(props.modelValue)

watch(selected, (val) => {
  emit('change', val)
})
</script>
