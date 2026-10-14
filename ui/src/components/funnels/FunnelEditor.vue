<template>
  <v-dialog v-model="open" max-width="720" scrollable>
    <v-card>
      <v-card-title class="text-h6 pa-6 pb-2">
        {{ funnel ? $t('funnels.edit') : $t('funnels.create') }}
      </v-card-title>
      <v-card-text class="px-6">
        <v-form ref="formRef" @submit.prevent="save">
          <v-text-field
            v-model="name"
            :label="$t('funnels.name')"
            :rules="[rules.required, rules.maxLength(100)]"
            placeholder="Blog to signup"
            class="mb-2"
          />
          <p class="text-body-2 text-medium-emphasis mb-4">{{ $t('funnels.stepsHint') }}</p>

          <div v-for="(step, i) in steps" :key="step.key" class="d-flex align-start ga-2 mb-2">
            <v-avatar size="28" color="primary" variant="tonal" class="mt-3 text-caption font-weight-bold">
              {{ i + 1 }}
            </v-avatar>
            <v-select
              v-model="step.type"
              :items="stepTypes"
              density="compact"
              hide-details="auto"
              style="max-width: 130px"
            />
            <v-text-field
              v-if="step.type === 'page'"
              v-model="step.pattern"
              :label="$t('funnels.pattern')"
              :rules="[rules.required, patternRule]"
              placeholder="/blog/*"
              density="compact"
              hide-details="auto"
              class="flex-grow-1"
            />
            <v-select
              v-else
              v-model="step.goal_id"
              :items="goalItems"
              :label="$t('funnels.goal')"
              :rules="[rules.required]"
              :no-data-text="$t('funnels.noGoals')"
              density="compact"
              hide-details="auto"
              class="flex-grow-1"
            />
            <v-text-field
              v-model="step.label"
              :label="$t('funnels.label')"
              :rules="[rules.maxLength(100)]"
              density="compact"
              hide-details="auto"
              style="max-width: 170px"
            />
            <div class="d-flex flex-column">
              <v-btn icon size="x-small" variant="text" :disabled="i === 0" @click="move(i, -1)">
                <v-icon>mdi-chevron-up</v-icon>
              </v-btn>
              <v-btn icon size="x-small" variant="text" :disabled="i === steps.length - 1" @click="move(i, 1)">
                <v-icon>mdi-chevron-down</v-icon>
              </v-btn>
            </div>
            <v-btn
              icon
              size="small"
              variant="text"
              color="error"
              class="mt-1"
              :disabled="steps.length <= MIN_STEPS"
              @click="steps.splice(i, 1)"
            >
              <v-icon>mdi-close</v-icon>
            </v-btn>
          </div>

          <v-btn
            variant="text"
            prepend-icon="mdi-plus"
            :disabled="steps.length >= MAX_STEPS"
            @click="addStep"
          >
            {{ $t('funnels.addStep') }}
          </v-btn>
        </v-form>
      </v-card-text>
      <v-card-actions class="px-6 pb-6">
        <v-spacer />
        <v-btn variant="text" @click="open = false">{{ $t('common.cancel') }}</v-btn>
        <v-btn color="primary" :loading="saving" @click="save">{{ $t('common.save') }}</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useFunnelsStore, type Funnel, type FunnelInput, type StepType } from '@/stores/funnels'
import { useSnackbar } from '@/composables/useSnackbar'
import { useValidation } from '@/composables/useValidation'

const MIN_STEPS = 2
const MAX_STEPS = 8

const props = defineProps<{
  orgId: string
  siteId: string
  funnel: Funnel | null
  goals: { id: string; name: string }[]
}>()

const emit = defineEmits<{ saved: [funnel: Funnel] }>()
const open = defineModel<boolean>({ default: false })

const { t } = useI18n()
const store = useFunnelsStore()
const { showSuccess, showError } = useSnackbar()
const { rules } = useValidation()

interface EditableStep {
  key: number
  type: StepType
  pattern: string
  goal_id: string | null
  label: string
}

let nextKey = 0
const formRef = ref()
const name = ref('')
const steps = ref<EditableStep[]>([])
const saving = ref(false)

const stepTypes = computed(() => [
  { title: t('funnels.page'), value: 'page' },
  { title: t('funnels.goal'), value: 'goal' },
])
const goalItems = computed(() => props.goals.map((g) => ({ title: g.name, value: g.id })))

const patternRule = (v: string) =>
  !v || v.trim().startsWith('/') || v.trim().startsWith('*') || t('funnels.patternStart')

function emptyStep(): EditableStep {
  return { key: nextKey++, type: 'page', pattern: '', goal_id: null, label: '' }
}

watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = props.funnel?.name ?? ''
  steps.value = props.funnel
    ? props.funnel.steps.map((s) => ({
        key: nextKey++,
        type: s.type,
        pattern: s.pattern ?? '',
        goal_id: s.goal_id,
        label: s.label ?? '',
      }))
    : [emptyStep(), emptyStep()]
})

function addStep() {
  if (steps.value.length < MAX_STEPS) steps.value.push(emptyStep())
}

function move(i: number, by: number) {
  const [step] = steps.value.splice(i, 1)
  steps.value.splice(i + by, 0, step)
}

function input(): FunnelInput {
  return {
    name: name.value.trim(),
    steps: steps.value.map((s) => {
      const label = s.label.trim() || undefined
      return s.type === 'page'
        ? { type: 'page', pattern: s.pattern.trim(), label }
        : { type: 'goal', goal_id: s.goal_id ?? '', label }
    }),
  }
}

async function save() {
  const { valid } = await formRef.value.validate()
  if (!valid) return
  saving.value = true
  try {
    const saved = props.funnel
      ? await store.updateFunnel(props.orgId, props.siteId, props.funnel.id, input())
      : await store.createFunnel(props.orgId, props.siteId, input())
    showSuccess(props.funnel ? t('funnels.updated') : t('funnels.created'))
    open.value = false
    emit('saved', saved)
  } catch (e) {
    showError(e instanceof Error ? e.message : t('funnels.saveFailed'))
  } finally {
    saving.value = false
  }
}
</script>
