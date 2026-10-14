<template>
  <div class="journey-sankey" :style="{ height: `${height}px` }">
    <Sankey :data="chartData" :options="options" :aria-label="ariaLabel" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Chart as ChartJS,
  LinearScale,
  Tooltip,
  type ChartData,
  type ChartOptions,
  type TooltipItem,
} from 'chart.js'
import { SankeyController, Flow, type SankeyScriptableContext } from 'chartjs-chart-sankey'
import { createTypedChart } from 'vue-chartjs'
import type { Journey, JourneyNode } from '@/stores/journeys'

// The Sankey lays its nodes out on two hidden linear scales. The dashboard
// registers that scale too, but this page can be the first one loaded.
ChartJS.register(SankeyController, Flow, LinearScale, Tooltip)
const Sankey = createTypedChart('sankey', SankeyController)

const props = defineProps<{ journey: Journey }>()
const { t } = useI18n()

const COLORS: Record<JourneyNode['kind'], string> = {
  anchor: '#6366F1',
  page: '#818CF8',
  exit: '#94A3B8',
  entrance: '#94A3B8',
  other: '#CBD5E1',
}
const MAX_LABEL = 32

const key = (id: number) => `n${id}`
const byKey = computed(() => new Map(props.journey.nodes.map((n) => [key(n.id), n])))
const into = computed(() => props.journey.anchor.type === 'goal')
const deepest = computed(() => Math.max(0, ...props.journey.nodes.map((n) => n.level)))

function fullLabel(node: JourneyNode): string {
  switch (node.kind) {
    case 'exit':
      return t('journeys.exit')
    case 'entrance':
      return t('journeys.entrance')
    case 'other':
      return t('journeys.other')
    default:
      return node.label
  }
}

function shortLabel(node: JourneyNode): string {
  const label = fullLabel(node)
  return label.length > MAX_LABEL ? `${label.slice(0, MAX_LABEL - 1)}…` : label
}

/** Among siblings: pages by visits, then the exit or entrance, then "other". */
function rank(node: JourneyNode): number {
  const kindRank = node.kind === 'other' ? 2 : node.kind === 'exit' || node.kind === 'entrance' ? 1 : 0
  return kindRank * 1e12 - node.visits
}

/**
 * Top to bottom within each column: the tree walked depth first, siblings
 * in `rank` order, so a branch stays together and its flows do not cross.
 */
const order = computed(() => {
  const nodes = props.journey.nodes
  const children = new Map<number, JourneyNode[]>()
  for (const l of props.journey.links) {
    const [a, b] = [nodes[l.source], nodes[l.target]]
    const [parent, child] = a.level < b.level ? [a, b] : [b, a]
    children.set(parent.id, [...(children.get(parent.id) ?? []), child])
  }
  const walked: number[] = []
  const walk = (node: JourneyNode) => {
    walked.push(node.id)
    for (const child of [...(children.get(node.id) ?? [])].sort((x, y) => rank(x) - rank(y))) walk(child)
  }
  if (nodes.length > 0) walk(nodes[0])
  return new Map(walked.map((id, i) => [id, i]))
})

const colorOf = (nodeKey: string) => COLORS[byKey.value.get(nodeKey)?.kind ?? 'page']

const chartData = computed<ChartData<'sankey'>>(() => {
  const nodes = props.journey.nodes
  return {
    datasets: [
      {
        label: t('journeys.visits'),
        data: props.journey.links.map((l) => ({ from: key(l.source), to: key(l.target), flow: l.visits })),
        labels: Object.fromEntries(nodes.map((n) => [key(n.id), shortLabel(n)])),
        // Going forward the anchor is the first column; into a goal, the last.
        column: Object.fromEntries(
          nodes.map((n) => [key(n.id), into.value ? deepest.value - n.level : n.level]),
        ),
        priority: Object.fromEntries(nodes.map((n) => [key(n.id), order.value.get(n.id) ?? n.id])),
        colorFrom: (c: SankeyScriptableContext) => colorOf(c.raw.from),
        colorTo: (c: SankeyScriptableContext) => colorOf(c.raw.to),
        colorMode: 'gradient',
        alpha: 0.6,
        size: 'max',
        nodeWidth: 12,
        nodePadding: 16,
        color: '#64748B',
        font: { size: 12 },
      },
    ],
  }
})

const options = computed<ChartOptions<'sankey'>>(() => ({
  responsive: true,
  maintainAspectRatio: false,
  animation: false,
  plugins: {
    legend: { display: false },
    tooltip: {
      callbacks: {
        title: () => '',
        label: (item: TooltipItem<'sankey'>) => {
          const raw = item.raw as { from: string; to: string; flow: number }
          const from = byKey.value.get(raw.from)
          const to = byKey.value.get(raw.to)
          const names = `${from ? fullLabel(from) : ''} → ${to ? fullLabel(to) : ''}`
          return `${names}: ${t('journeys.visitCount', raw.flow)}`
        },
      },
    },
  },
}))

/** Tall enough for the fullest column. */
const height = computed(() => {
  const perColumn = new Map<number, number>()
  for (const n of props.journey.nodes) perColumn.set(n.level, (perColumn.get(n.level) ?? 0) + 1)
  const fullest = Math.max(1, ...perColumn.values())
  return Math.max(280, 80 + fullest * 52)
})

const ariaLabel = computed(() =>
  t('journeys.chartLabel', { anchor: props.journey.anchor.label, nodes: props.journey.nodes.length }),
)
</script>
