<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-3 mb-6">
      <h1 class="text-h5 font-weight-bold mr-auto">{{ $t('journeys.title') }}</h1>
      <DatePicker :model-value="period" @change="onPeriod" />
    </div>

    <v-card class="pa-4">
      <v-btn-toggle v-model="mode" mandatory density="compact" variant="outlined" divided color="primary" class="mb-4">
        <v-btn value="next" size="small" prepend-icon="mdi-arrow-right">{{ $t('journeys.fromEntry') }}</v-btn>
        <v-btn value="into" size="small" prepend-icon="mdi-bullseye-arrow">{{ $t('journeys.intoGoal') }}</v-btn>
      </v-btn-toggle>

      <div class="d-flex align-center flex-wrap ga-3 mb-2">
        <v-combobox
          v-if="mode === 'next'"
          v-model="entryPage"
          :items="entryPages"
          item-title="title"
          item-value="value"
          :return-object="false"
          :label="$t('journeys.entryPage')"
          :placeholder="$t('journeys.entryPagePlaceholder')"
          density="compact"
          hide-details
          clearable
          style="min-width: 280px; max-width: 420px"
        />
        <v-select
          v-else
          v-model="goalId"
          :items="goalItems"
          :label="$t('journeys.goal')"
          :no-data-text="$t('journeys.noGoals')"
          density="compact"
          hide-details
          style="min-width: 240px; max-width: 360px"
        />
        <v-select
          v-model="depth"
          :items="[1, 2, 3, 4, 5]"
          :label="$t('journeys.depth')"
          density="compact"
          hide-details
          style="max-width: 120px"
        />
        <v-select
          v-model="width"
          :items="[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]"
          :label="$t('journeys.width')"
          density="compact"
          hide-details
          style="max-width: 150px"
        />
      </div>

      <v-progress-linear v-if="loading" indeterminate color="primary" class="my-2" />

      <div v-if="!anchorChosen" class="pa-8 text-center text-medium-emphasis">
        {{ mode === 'next' ? $t('journeys.pickEntry') : $t('journeys.pickGoal') }}
      </div>
      <div v-else-if="journey && journey.nodes.length === 0" class="pa-8 text-center text-medium-emphasis">
        <v-icon size="48" color="secondary" class="mb-2">mdi-shield-account-outline</v-icon>
        <p>{{ $t('journeys.tooFew', { min: journey.meta.min_visitors }) }}</p>
      </div>
      <template v-else-if="journey">
        <div class="d-flex align-center flex-wrap ga-4 text-body-2 my-2">
          <span>
            <span class="font-weight-bold">{{ journey.nodes[0].visits }}</span>
            {{ mode === 'next' ? $t('journeys.visitsEntered') : $t('journeys.visitsConverted') }}
          </span>
          <span class="text-medium-emphasis">{{ $t('journeys.legend') }}</span>
        </div>
        <JourneySankey :journey="journey" />
      </template>

      <p class="text-caption text-medium-emphasis mt-4">
        {{ $t('journeys.explain') }}
      </p>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useJourneysStore, type Journey } from '@/stores/journeys'
import { useGoalsStore } from '@/stores/goals'
import { useOrgStore } from '@/stores/org'
import { useSiteStore } from '@/stores/site'
import { useSnackbar } from '@/composables/useSnackbar'
import { presetPeriod, utcToday } from '@/utils/period'
import DatePicker from '@/components/dashboard/DatePicker.vue'
import JourneySankey from '@/components/journeys/JourneySankey.vue'

const route = useRoute()
const { t } = useI18n()
const store = useJourneysStore()
const goalsStore = useGoalsStore()
const orgStore = useOrgStore()
const siteStore = useSiteStore()
const { showError } = useSnackbar()

const orgId = route.params.orgId as string
const siteId = route.params.siteId as string

const period = ref('30d')
const mode = ref<'next' | 'into'>('next')
const entryPage = ref<string | null>(null)
const goalId = ref<string | null>(null)
const depth = ref(3)
const width = ref(5)
const entryPages = ref<{ title: string; value: string }[]>([])
const journey = ref<Journey | null>(null)
const loading = ref(false)

const goalItems = computed(() => goalsStore.goals.map((g) => ({ title: g.name, value: g.id })))
const anchorChosen = computed(() =>
  mode.value === 'next' ? !!entryPage.value?.trim() : !!goalId.value,
)

/** The days the dashboard's period buttons mean, so both read the same days. */
const days = () => presetPeriod(period.value, utcToday())

onMounted(async () => {
  // The sidebar's site links need the org and the site, as on the dashboard.
  await Promise.all([
    goalsStore.fetchGoals(orgId, siteId),
    orgStore.currentOrg?.id === orgId ? Promise.resolve() : orgStore.fetchOrg(orgId),
    siteStore.currentSite?.id === siteId ? Promise.resolve() : siteStore.fetchSite(orgId, siteId),
    loadEntryPages(),
  ])
  if (!entryPage.value && entryPages.value.length > 0) entryPage.value = entryPages.value[0].value
  if (!goalId.value && goalItems.value.length > 0) goalId.value = goalItems.value[0].value
})

async function loadEntryPages() {
  try {
    const pages = await store.fetchEntryPages(orgId, siteId, days())
    entryPages.value = pages.map((p) => ({ title: p.path, value: p.path }))
  } catch {
    entryPages.value = []
  }
}

// Answers can arrive out of order; only the latest request's is shown.
let requestSeq = 0

async function load() {
  if (!anchorChosen.value) {
    journey.value = null
    return
  }
  const seq = ++requestSeq
  loading.value = true
  try {
    const answer = await store.fetchJourney(orgId, siteId, {
      date_from: days().from,
      date_to: days().to,
      ...(mode.value === 'next'
        ? { entry_page: entryPage.value!.trim() }
        : { goal_id: goalId.value! }),
      depth: depth.value,
      width: width.value,
    })
    if (seq === requestSeq) journey.value = answer
  } catch (e) {
    if (seq === requestSeq) {
      journey.value = null
      showError(e instanceof Error ? e.message : t('journeys.loadFailed'))
    }
  } finally {
    if (seq === requestSeq) loading.value = false
  }
}

watch([mode, entryPage, goalId, depth, width, period], load)

function onPeriod(range: string) {
  period.value = range
  loadEntryPages()
}
</script>
