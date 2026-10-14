<template>
  <v-dialog v-model="open" max-width="460">
    <v-card class="pa-6" data-test="filter-dialog">
      <v-card-title class="text-h6 pa-0 mb-4">Filter the dashboard</v-card-title>
      <v-select
        v-model="dimension"
        :items="dimensionItems"
        label="Dimension"
        class="mb-2"
        data-test="filter-dimension"
      />
      <v-select v-model="operator" :items="OPERATORS" item-title="label" item-value="value" label="Operator" class="mb-2" />
      <v-combobox
        v-model="value"
        :items="suggestions"
        :loading="loadingSuggestions"
        item-title="title"
        item-value="value"
        :return-object="false"
        label="Value"
        hint="Pick a value seen in this period, or type one. Values are matched exactly, case included."
        persistent-hint
        class="mb-4"
        data-test="filter-value"
      />
      <div class="d-flex justify-end ga-2">
        <v-btn variant="text" @click="open = false">{{ $t('common.cancel') }}</v-btn>
        <v-btn color="primary" :disabled="value == null" data-test="filter-apply" @click="apply">Apply</v-btn>
      </div>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useStatsStore, type StatsFilter } from '@/stores/stats'
import { FILTER_DIMENSIONS, OPERATORS, dimensionLabel, valueLabel } from '@/utils/dimensions'

const open = defineModel<boolean>({ default: false })
const emit = defineEmits<{ apply: [filter: StatsFilter] }>()

const statsStore = useStatsStore()
const dimension = ref<string>('path')
const operator = ref('is')
const value = ref<string | null>(null)
const suggestions = ref<{ title: string; value: string }[]>([])
const loadingSuggestions = ref(false)

const dimensionItems = computed(() =>
  FILTER_DIMENSIONS.map((d) => ({ title: dimensionLabel(d), value: d })),
)

/** The values of the dimension in this period, most visitors first. */
async function loadSuggestions() {
  if (!statsStore.orgId) return
  const d = dimension.value
  loadingSuggestions.value = true
  try {
    const answer = await statsStore.runReport({ dimensions: [d], metrics: ['visitors'], limit: 25 })
    if (d !== dimension.value) return
    suggestions.value = answer.rows.map((r) => ({
      title: valueLabel(d, r.dimensions[d] ?? ''),
      value: r.dimensions[d] ?? '',
    }))
  } catch {
    suggestions.value = []
  } finally {
    loadingSuggestions.value = false
  }
}

watch(dimension, () => {
  value.value = null
  loadSuggestions()
})
watch(open, (isOpen) => {
  if (isOpen) loadSuggestions()
})

function apply() {
  if (value.value == null) return
  emit('apply', { dimension: dimension.value, operator: operator.value, value: value.value })
  open.value = false
  value.value = null
}
</script>
