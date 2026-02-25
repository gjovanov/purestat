import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  Filler,
  Tooltip,
  Legend,
  type ChartData,
  type ChartOptions,
} from 'chart.js'

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  Filler,
  Tooltip,
  Legend,
)

const CHART_COLORS = {
  primary: '#6366F1',
  primaryBg: 'rgba(99, 102, 241, 0.1)',
  secondary: '#34D399',
  secondaryBg: 'rgba(52, 211, 153, 0.1)',
  tertiary: '#FBBF24',
  tertiaryBg: 'rgba(251, 191, 36, 0.1)',
  error: '#FB7185',
  grid: 'rgba(148, 163, 184, 0.1)',
  text: '#94A3B8',
}

export function useCharts() {
  function buildLineChartData(
    labels: string[],
    datasets: { label: string; data: number[]; color?: string }[],
  ): ChartData<'line'> {
    const colors = [CHART_COLORS.primary, CHART_COLORS.secondary, CHART_COLORS.tertiary]
    const bgs = [CHART_COLORS.primaryBg, CHART_COLORS.secondaryBg, CHART_COLORS.tertiaryBg]

    return {
      labels,
      datasets: datasets.map((ds, i) => ({
        label: ds.label,
        data: ds.data,
        borderColor: ds.color || colors[i % colors.length],
        backgroundColor: bgs[i % bgs.length],
        fill: true,
        tension: 0.4,
        pointRadius: 0,
        pointHoverRadius: 4,
        borderWidth: 2,
      })),
    }
  }

  const defaultLineOptions: ChartOptions<'line'> = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: 'index',
      intersect: false,
    },
    plugins: {
      legend: {
        display: false,
      },
      tooltip: {
        backgroundColor: '#1E293B',
        titleColor: '#E2E8F0',
        bodyColor: '#E2E8F0',
        padding: 12,
        cornerRadius: 8,
        displayColors: false,
      },
    },
    scales: {
      x: {
        grid: { display: false },
        ticks: { color: CHART_COLORS.text, maxRotation: 0, autoSkipPadding: 20 },
        border: { display: false },
      },
      y: {
        grid: { color: CHART_COLORS.grid },
        ticks: { color: CHART_COLORS.text, padding: 8 },
        border: { display: false },
        beginAtZero: true,
      },
    },
  }

  return { buildLineChartData, defaultLineOptions, CHART_COLORS }
}
