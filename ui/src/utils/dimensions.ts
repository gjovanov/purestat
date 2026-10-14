// What the dashboard calls each report dimension and value. The grouping
// itself (channels, normalised campaign sources) is the server's: this only
// names what the API answers.

export const DIMENSION_LABELS: Record<string, string> = {
  path: 'Page',
  entry_page: 'Entry page',
  exit_page: 'Exit page',
  hostname: 'Hostname',
  channel: 'Channel',
  referrer_source: 'Source',
  referrer_host: 'Referrer',
  utm_source: 'UTM source',
  utm_medium: 'UTM medium',
  utm_campaign: 'UTM campaign',
  utm_content: 'UTM content',
  utm_term: 'UTM term',
  utm_source_group: 'UTM source group',
  utm_medium_group: 'UTM medium group',
  country: 'Country',
  device_type: 'Device',
  browser: 'Browser',
  os: 'Operating system',
}

/** The dimensions the filter control offers, in its order. */
export const FILTER_DIMENSIONS = [
  'path',
  'entry_page',
  'exit_page',
  'hostname',
  'channel',
  'referrer_source',
  'referrer_host',
  'utm_source',
  'utm_medium',
  'utm_campaign',
  'utm_content',
  'utm_term',
  'country',
  'device_type',
  'browser',
  'os',
] as const

export const OPERATORS: { value: string; label: string }[] = [
  { value: 'is', label: 'is' },
  { value: 'is_not', label: 'is not' },
  { value: 'contains', label: 'contains' },
  { value: 'starts_with', label: 'starts with' },
]

/** The channels of FR-102 §3b, as the API spells them, in report order. */
export const CHANNEL_LABELS: Record<string, string> = {
  search: 'Search',
  ai_assistant: 'AI assistants',
  social: 'Social',
  community: 'Community',
  code: 'Code',
  self: 'Own site',
  direct: 'Direct',
  other: 'Other',
}

export function dimensionLabel(dimension: string): string {
  return DIMENSION_LABELS[dimension] ?? dimension
}

export function operatorLabel(operator: string): string {
  return OPERATORS.find((o) => o.value === operator)?.label ?? operator
}

/** A value as a row shows it. An empty value is a visit without one. */
export function valueLabel(dimension: string, value: string): string {
  if (dimension === 'channel') return CHANNEL_LABELS[value] ?? value
  if (value === '') return '(none)'
  return value
}
