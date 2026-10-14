<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-3 mb-6">
      <h1 class="text-h5 font-weight-bold mr-auto">{{ $t('funnels.title') }}</h1>
      <DatePicker :model-value="period" @change="onPeriod" />
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openEditor(null)">
        {{ $t('funnels.create') }}
      </v-btn>
    </div>

    <v-card v-if="!store.loading && store.funnels.length === 0" class="pa-8 text-center">
      <v-icon size="64" color="secondary" class="mb-4">mdi-filter-outline</v-icon>
      <p class="text-body-1 text-medium-emphasis">{{ $t('funnels.empty') }}</p>
    </v-card>

    <v-row v-else>
      <v-col cols="12" md="4">
        <v-card>
          <v-list density="compact" nav>
            <v-list-item
              v-for="f in store.funnels"
              :key="f.id"
              :active="f.id === selectedId"
              color="primary"
              @click="select(f.id)"
            >
              <v-list-item-title>{{ f.name }}</v-list-item-title>
              <v-list-item-subtitle>{{ $t('funnels.stepCount', { count: f.steps.length }) }}</v-list-item-subtitle>
              <template #append>
                <v-btn icon size="small" variant="text" :title="$t('common.edit')" @click.stop="openEditor(f)">
                  <v-icon>mdi-pencil-outline</v-icon>
                </v-btn>
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="error"
                  :title="$t('common.delete')"
                  @click.stop="remove(f)"
                >
                  <v-icon>mdi-delete-outline</v-icon>
                </v-btn>
              </template>
            </v-list-item>
          </v-list>
        </v-card>
      </v-col>

      <v-col cols="12" md="8">
        <v-card v-if="selected" class="pa-4">
          <div class="d-flex align-center flex-wrap ga-3 mb-4">
            <h2 class="text-h6 mr-auto">{{ selected.name }}</h2>
            <v-select
              v-model="breakdown"
              :items="VISIT_DIMENSIONS"
              :label="$t('funnels.breakdown')"
              density="compact"
              hide-details
              clearable
              style="max-width: 220px"
            />
          </div>

          <div class="d-flex align-center flex-wrap ga-2 mb-2">
            <v-select
              v-model="newFilter.dimension"
              :items="VISIT_DIMENSIONS"
              :label="$t('funnels.filterBy')"
              density="compact"
              hide-details
              style="max-width: 180px"
            />
            <v-select
              v-model="newFilter.operator"
              :items="operators"
              density="compact"
              hide-details
              style="max-width: 150px"
            />
            <v-text-field
              v-model="newFilter.value"
              :label="$t('funnels.value')"
              density="compact"
              hide-details
              style="max-width: 200px"
              @keyup.enter="addFilter"
            />
            <v-btn variant="tonal" size="small" :disabled="!newFilter.dimension" @click="addFilter">
              {{ $t('funnels.addFilter') }}
            </v-btn>
          </div>
          <div class="d-flex align-center flex-wrap ga-2 mb-4">
            <v-chip
              v-for="(filter, i) in filters"
              :key="`${filter.dimension}-${i}`"
              closable
              size="small"
              color="primary"
              variant="tonal"
              data-test="funnel-filter"
              @click:close="filters.splice(i, 1)"
            >
              {{ dimensionLabel(filter.dimension) }} {{ operatorLabel(filter.operator) }}
              <strong class="ml-1">{{ valueLabel(filter.dimension, filter.value) }}</strong>
            </v-chip>
            <v-btn v-if="filters.length > 1" size="x-small" variant="text" color="error" @click="filters = []">
              {{ $t('funnels.clearFilters') }}
            </v-btn>
          </div>

          <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-4" />
          <template v-if="report">
            <FunnelSteps :steps="report.steps" />
            <template v-if="report.breakdown">
              <v-divider class="my-4" />
              <FunnelBreakdown
                :dimension="report.breakdown.dimension"
                :labels="report.steps.map((s) => s.label)"
                :rows="report.breakdown.rows"
              />
              <p v-if="report.breakdown.has_more" class="text-caption text-medium-emphasis mt-2">
                {{ $t('funnels.moreRows', { count: report.breakdown.rows.length }) }}
              </p>
            </template>
            <p class="text-caption text-medium-emphasis mt-4">
              {{ $t('funnels.explain') }}
            </p>
          </template>
        </v-card>
        <v-card v-else class="pa-8 text-center text-medium-emphasis">
          {{ $t('funnels.pick') }}
        </v-card>
      </v-col>
    </v-row>

    <FunnelEditor
      v-model="editorOpen"
      :org-id="orgId"
      :site-id="siteId"
      :funnel="editing"
      :goals="goalsStore.goals"
      @saved="onSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  useFunnelsStore,
  VISIT_DIMENSIONS,
  type Funnel,
  type FunnelFilter,
  type FunnelReport,
} from '@/stores/funnels'
import { useGoalsStore } from '@/stores/goals'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'
import { useSnackbar } from '@/composables/useSnackbar'
import { presetPeriod, utcToday } from '@/utils/period'
import { OPERATORS, dimensionLabel, operatorLabel, valueLabel } from '@/utils/dimensions'
import DatePicker from '@/components/dashboard/DatePicker.vue'
import FunnelEditor from '@/components/funnels/FunnelEditor.vue'
import FunnelSteps from '@/components/funnels/FunnelSteps.vue'
import FunnelBreakdown from '@/components/funnels/FunnelBreakdown.vue'

const route = useRoute()
const { t } = useI18n()
const store = useFunnelsStore()
const goalsStore = useGoalsStore()
const orgStore = useOrgStore()
const siteStore = useSiteStore()
const { showSuccess, showError } = useSnackbar()

const orgId = route.params.orgId as string
const siteId = route.params.siteId as string

const period = ref('30d')
const selectedId = ref<string | null>(null)
const breakdown = ref<string | null>(null)
const filters = ref<FunnelFilter[]>([])
const newFilter = reactive({ dimension: 'utm_source', operator: 'is', value: '' })
const report = ref<FunnelReport | null>(null)
const loading = ref(false)
const editorOpen = ref(false)
const editing = ref<Funnel | null>(null)

const operators = OPERATORS.map((o) => ({ title: o.label, value: o.value }))

const selected = computed(() => store.funnels.find((f) => f.id === selectedId.value) ?? null)

onMounted(async () => {
  // The sidebar's site links need the org and the site, as on the dashboard.
  await Promise.all([
    store.fetchFunnels(orgId, siteId),
    goalsStore.fetchGoals(orgId, siteId),
    orgStore.currentOrg?.id === orgId ? Promise.resolve() : orgStore.fetchOrg(orgId),
    siteStore.currentSite?.id === siteId ? Promise.resolve() : siteStore.fetchSite(orgId, siteId),
  ])
  if (store.funnels.length > 0) selectedId.value = store.funnels[0].id
})

// Answers can arrive out of order; only the latest request's is shown.
let requestSeq = 0

async function loadReport() {
  const id = selectedId.value
  if (!id) {
    report.value = null
    return
  }
  const seq = ++requestSeq
  loading.value = true
  try {
    // The days the dashboard's period buttons mean, so both read the same days.
    const days = presetPeriod(period.value, utcToday())
    const answer = await store.fetchReport(orgId, siteId, id, {
      date_from: days.from,
      date_to: days.to,
      breakdown: breakdown.value ?? undefined,
      filters: filters.value.length ? filters.value : undefined,
    })
    if (seq === requestSeq) report.value = answer
  } catch (e) {
    if (seq === requestSeq) {
      report.value = null
      showError(e instanceof Error ? e.message : t('funnels.loadFailed'))
    }
  } finally {
    if (seq === requestSeq) loading.value = false
  }
}

watch([selectedId, period, breakdown, filters], loadReport, { deep: true })

function select(id: string) {
  selectedId.value = id
}

function onPeriod(range: string) {
  period.value = range
}

function addFilter() {
  if (!newFilter.dimension) return
  filters.value.push({ ...newFilter })
  newFilter.value = ''
}

function openEditor(funnel: Funnel | null) {
  editing.value = funnel
  editorOpen.value = true
}

function onSaved(funnel: Funnel) {
  if (selectedId.value === funnel.id) {
    loadReport()
  } else {
    selectedId.value = funnel.id
  }
}

async function remove(funnel: Funnel) {
  if (!confirm(t('funnels.confirmDelete', { name: funnel.name }))) return
  try {
    await store.deleteFunnel(orgId, siteId, funnel.id)
    if (selectedId.value === funnel.id) {
      selectedId.value = store.funnels[0]?.id ?? null
    }
    showSuccess(t('funnels.deleted'))
  } catch (e) {
    showError(e instanceof Error ? e.message : t('funnels.deleteFailed'))
  }
}
</script>
