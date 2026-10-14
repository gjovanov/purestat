<template>
  <v-card :data-test="`breakdown-${title.toLowerCase()}`">
    <div class="d-flex align-center flex-wrap ga-2 px-4 pt-3">
      <span class="text-body-1 font-weight-bold">{{ title }}</span>
      <v-spacer />
      <v-select
        v-if="tab.goals && goals && goals.length"
        v-model="goal"
        :items="goalItems"
        density="compact"
        variant="outlined"
        hide-details
        label="Goal"
        style="max-width: 190px"
        data-test="breakdown-goal"
      />
    </div>
    <v-tabs v-if="tabs.length > 1" v-model="tabKey" density="compact" color="primary" class="px-2">
      <v-tab v-for="t in tabs" :key="t.key" :value="t.key" class="text-none" size="small">
        {{ t.label }}
      </v-tab>
    </v-tabs>
    <v-table density="compact" class="breakdown-table">
      <thead>
        <tr>
          <th class="text-left">{{ tab.label }}</th>
          <th v-for="col in columns" :key="col.key" class="text-right text-no-wrap" :title="col.title">
            {{ col.label }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="row in rows"
          :key="row.value"
          class="breakdown-row"
          :style="barStyle(row)"
          :title="`Filter the dashboard by ${dimensionLabel(tab.dimension)}: ${valueLabel(tab.dimension, row.value)}`"
          @click="drill(row.value)"
        >
          <td class="text-truncate" style="max-width: 260px">
            {{ valueLabel(tab.dimension, row.value) }}
          </td>
          <td v-for="col in columns" :key="col.key" class="text-right text-no-wrap">
            {{ cell(col, row) }}
          </td>
        </tr>
        <tr v-if="!loading && rows.length === 0">
          <td :colspan="columns.length + 1" class="text-medium-emphasis">No data yet</td>
        </tr>
      </tbody>
    </v-table>
    <v-progress-linear v-if="loading" indeterminate color="primary" height="2" />
    <div v-if="hasMore" class="pa-2 text-center">
      <v-btn variant="text" size="small" :loading="loadingMore" data-test="show-more" @click="showMore">
        Show more
      </v-btn>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useStatsStore, type ReportRow, type StatsFilter } from '@/stores/stats'
import type { BreakdownColumn, BreakdownTab } from '@/components/dashboard/breakdown'
import { dimensionLabel, valueLabel } from '@/utils/dimensions'
import { formatCount, formatDuration, formatPercent } from '@/utils/format'

const props = defineProps<{
  title: string
  tabs: BreakdownTab[]
  /** The site's goal names, for a tab that can show conversions. */
  goals?: string[]
}>()

const emit = defineEmits<{
  filter: [filter: StatsFilter]
}>()

const FIRST_PAGE = 10
const MORE = 25

const statsStore = useStatsStore()
const tabKey = ref(props.tabs[0].key)
const tab = computed(() => props.tabs.find((t) => t.key === tabKey.value) ?? props.tabs[0])
const goal = ref<string | null>(null)
const goalItems = computed(() => [
  { title: 'No goal', value: null },
  ...(props.goals ?? []).map((g) => ({ title: g, value: g })),
])

interface Row {
  value: string
  row: ReportRow
}

const rows = ref<Row[]>([])
const hasMore = ref(false)
const loading = ref(false)
const loadingMore = ref(false)
let requestId = 0

const activeGoal = computed(() => (tab.value.goals ? goal.value : null))

const columns = computed<BreakdownColumn[]>(() =>
  activeGoal.value
    ? [
        ...tab.value.columns,
        { key: 'conversions', label: 'Conv.', title: `Visitors who completed ${activeGoal.value}` },
        { key: 'conversion_rate', label: 'CR', title: 'Conversions / visitors', format: 'percent' },
      ]
    : tab.value.columns,
)

const maxVisitors = computed(() =>
  Math.max(1, ...rows.value.map((r) => Number(r.row.metrics.visitors ?? 0))),
)

async function fetch(reset: boolean) {
  if (!statsStore.orgId) return
  const id = ++requestId
  const t = tab.value
  const g = activeGoal.value
  if (reset) loading.value = true
  else loadingMore.value = true
  try {
    const answer = await statsStore.runReport({
      dimensions: [t.dimension],
      metrics: t.metrics,
      goals: g ? [g] : undefined,
      filters: t.filters,
      limit: reset ? FIRST_PAGE : MORE,
      offset: reset ? 0 : rows.value.length,
    })
    if (id !== requestId) return
    const page = answer.rows.map((row) => ({ value: row.dimensions[t.dimension] ?? '', row }))
    rows.value = reset ? page : [...rows.value, ...page]
    hasMore.value = answer.meta.has_more
  } catch {
    if (id === requestId && reset) {
      rows.value = []
      hasMore.value = false
    }
  } finally {
    if (id === requestId) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

function showMore() {
  fetch(false)
}

function drill(value: string) {
  emit('filter', { dimension: tab.value.dimension, operator: 'is', value })
  if (tab.value.next) tabKey.value = tab.value.next
}

function cell(col: BreakdownColumn, { row }: Row): string {
  let value: number | null | undefined
  if (col.key === 'conversions') value = activeGoal.value ? row.goals[activeGoal.value] : null
  else if (col.key === 'conversion_rate')
    value = activeGoal.value ? row.conversion_rates?.[activeGoal.value] : null
  else value = row.metrics[col.key]
  switch (col.format) {
    case 'percent':
      return formatPercent(value)
    case 'duration':
      return formatDuration(value)
    case 'scroll':
      return value == null ? formatPercent(null) : `${Math.round(value)}%`
    default:
      return formatCount(value)
  }
}

/** A bar behind the row, as long as its share of the top row's visitors. */
function barStyle({ row }: Row) {
  const share = (Number(row.metrics.visitors ?? 0) / maxVisitors.value) * 100
  return {
    background: `linear-gradient(90deg, rgba(var(--v-theme-primary), 0.08) ${share}%, transparent ${share}%)`,
  }
}

watch(
  () => statsStore.version,
  () => fetch(true),
)
watch([tabKey, activeGoal], () => fetch(true))
onMounted(() => fetch(true))
</script>

<style scoped>
.breakdown-row {
  cursor: pointer;
}
.breakdown-row:hover td {
  background: rgba(var(--v-theme-primary), 0.06);
}
.breakdown-table :deep(td),
.breakdown-table :deep(th) {
  background: transparent;
}
</style>
