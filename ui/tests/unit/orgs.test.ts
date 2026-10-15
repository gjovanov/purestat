import { describe, expect, test } from 'bun:test'
import { orgOptions } from '../../src/utils/orgs'

const a = { id: 'a1', name: 'Alpha' }
const b = { id: 'b2', name: 'Beta' }

describe('the org picker items', () => {
  test('a page opened directly: the list is empty but the current org shows by name', () => {
    expect(orgOptions([], a)).toEqual([a])
  })

  test('the current org already in the list is not added twice', () => {
    expect(orgOptions([a, b], b)).toEqual([a, b])
  })

  test('a current org missing from a loaded list is kept, first', () => {
    expect(orgOptions([b], a)).toEqual([a, b])
  })

  test('no current org: the list as it is, as a copy', () => {
    const list = [a, b]
    const out = orgOptions(list, null)
    expect(out).toEqual(list)
    expect(out).not.toBe(list)
  })
})
