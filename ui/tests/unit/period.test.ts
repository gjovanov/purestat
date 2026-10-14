import { describe, expect, test } from 'bun:test'
import {
  addDays,
  daysOf,
  lengthInDays,
  pointChange,
  presetPeriod,
  previousPeriod,
  relativeChange,
  utcToday,
} from '../../src/utils/period'
import { valueLabel } from '../../src/utils/dimensions'
import {
  formatChange,
  formatCompact,
  formatCount,
  formatDuration,
  formatPercent,
  formatPoints,
} from '../../src/utils/format'

describe('formats', () => {
  test('counts, compact and in full', () => {
    expect(formatCompact(999)).toBe('999')
    expect(formatCompact(12_345)).toBe('12.3k')
    expect(formatCompact(2_500_000)).toBe('2.5M')
    expect(formatCount(12_345)).toBe('12,345')
    expect(formatCount(null)).toBe('—')
  })

  test('rates as percentages, null as a dash', () => {
    expect(formatPercent(0.253)).toBe('25%')
    expect(formatPercent(0.0525)).toBe('5.3%')
    expect(formatPercent(0)).toBe('0%')
    expect(formatPercent(1)).toBe('100%')
    expect(formatPercent(null)).toBe('—')
  })

  test('durations', () => {
    expect(formatDuration(45)).toBe('45s')
    expect(formatDuration(245)).toBe('4m 05s')
    expect(formatDuration(3_725)).toBe('1h 2m')
    expect(formatDuration(null)).toBe('—')
  })

  test('changes, relative and in points', () => {
    expect(formatChange(0.25)).toBe('+25%')
    expect(formatChange(-0.104)).toBe('-10%')
    expect(formatChange(0)).toBe('0%')
    expect(formatChange(null)).toBe('—')
    expect(formatPoints(0.025)).toBe('+2.5 pp')
    expect(formatPoints(-0.1)).toBe('-10 pp')
  })
})

describe('periods', () => {
  test('today is the UTC day', () => {
    expect(utcToday(new Date('2026-10-14T23:30:00-02:00'))).toBe('2026-10-15')
    expect(utcToday(new Date('2026-10-15T00:30:00+02:00'))).toBe('2026-10-14')
  })

  test('a preset ends today and covers its days, both ends included', () => {
    expect(presetPeriod('1d', '2026-10-15')).toEqual({ from: '2026-10-15', to: '2026-10-15' })
    expect(presetPeriod('7d', '2026-10-15')).toEqual({ from: '2026-10-09', to: '2026-10-15' })
    expect(presetPeriod('30d', '2026-03-01')).toEqual({ from: '2026-01-31', to: '2026-03-01' })
    expect(lengthInDays(presetPeriod('12mo', '2026-10-15'))).toBe(365)
    expect(lengthInDays(presetPeriod('6mo', '2026-10-15'))).toBe(182)
    expect(presetPeriod('nonsense', '2026-10-15')).toEqual(presetPeriod('30d', '2026-10-15'))
  })

  test('the previous period has the same length and ends the day before', () => {
    expect(previousPeriod({ from: '2026-10-15', to: '2026-10-15' })).toEqual({
      from: '2026-10-14',
      to: '2026-10-14',
    })
    expect(previousPeriod({ from: '2026-10-09', to: '2026-10-15' })).toEqual({
      from: '2026-10-02',
      to: '2026-10-08',
    })
    // Across a month end and a leap day.
    expect(previousPeriod({ from: '2028-03-01', to: '2028-03-02' })).toEqual({
      from: '2028-02-28',
      to: '2028-02-29',
    })
    // The longest preset and its previous period both fit the API's 366-day span.
    const year = presetPeriod('12mo', '2026-10-15')
    expect(lengthInDays(previousPeriod(year))).toBe(365)
  })

  test('the days of a period, oldest first', () => {
    expect(daysOf({ from: '2026-12-30', to: '2027-01-02' })).toEqual([
      '2026-12-30',
      '2026-12-31',
      '2027-01-01',
      '2027-01-02',
    ])
    expect(addDays('2026-03-29', 1)).toBe('2026-03-30')
  })
})

describe('changes', () => {
  test('a relative change needs a previous value above 0', () => {
    expect(relativeChange(15, 10)).toBe(0.5)
    expect(relativeChange(5, 10)).toBe(-0.5)
    expect(relativeChange(10, 0)).toBeNull()
    expect(relativeChange(null, 10)).toBeNull()
    expect(relativeChange(10, undefined)).toBeNull()
  })

  test('a change of rates is in points', () => {
    expect(pointChange(0.25, 0.2)).toBeCloseTo(0.05)
    expect(pointChange(0, 0)).toBe(0)
    expect(pointChange(null, 0.2)).toBeNull()
  })
})

describe('value labels', () => {
  test('channels by name, a missing value as (none)', () => {
    expect(valueLabel('channel', 'ai_assistant')).toBe('AI assistants')
    expect(valueLabel('channel', 'self')).toBe('Own site')
    expect(valueLabel('channel', 'something_new')).toBe('something_new')
    expect(valueLabel('utm_source', '')).toBe('(none)')
    expect(valueLabel('utm_source', 'youtube')).toBe('youtube')
  })
})
