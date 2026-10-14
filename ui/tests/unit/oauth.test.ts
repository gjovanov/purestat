import { describe, expect, test } from 'bun:test'
import {
  OAUTH_ERRORS,
  oauthError,
  parseProviders,
  safeNext,
  startPath,
} from '../../src/utils/oauth'

describe('parseProviders', () => {
  test('keeps the providers with a button, in button order', () => {
    expect(parseProviders(['google', 'github'])).toEqual(['google', 'github'])
    expect(parseProviders(['github', 'google'])).toEqual(['google', 'github'])
    expect(parseProviders(['github'])).toEqual(['github'])
  })

  test('anything else shows no button', () => {
    for (const body of [
      [],
      ['facebook', 'microsoft'],
      ['Google'],
      null,
      undefined,
      'google',
      { providers: ['google'] },
    ]) {
      expect(parseProviders(body)).toEqual([])
    }
  })
})

describe('startPath', () => {
  test('is the API route that starts the sign-in', () => {
    expect(startPath('google')).toBe('/api/oauth/google')
    expect(startPath('github')).toBe('/api/oauth/github')
  })
})

describe('oauthError', () => {
  test('every code the callback sends is known', () => {
    for (const code of OAUTH_ERRORS) {
      expect(oauthError(code)).toBe(code)
    }
  })

  test('an unknown code reads as a server error, and none as none', () => {
    expect(oauthError('something_new')).toBe('server')
    expect(oauthError(['account_exists', 'state'])).toBe('account_exists')
    expect(oauthError('')).toBeNull()
    expect(oauthError(undefined)).toBeNull()
    expect(oauthError(null)).toBeNull()
  })
})

describe('safeNext', () => {
  test('a path inside the app is kept', () => {
    expect(safeNext('/orgs')).toBe('/orgs')
    expect(safeNext('/invite/abc123')).toBe('/invite/abc123')
    expect(safeNext('/org/1/site/2?period=7d')).toBe('/org/1/site/2?period=7d')
  })

  test('another origin, the API or a sign-in page is not', () => {
    for (const next of [
      'https://evil.example/',
      '//evil.example/x',
      '/\\evil.example',
      '/x\\y',
      'javascript:alert(1)',
      'orgs',
      '/ /evil.example',
      '/api/oauth/google',
      '/api',
      '/login?redirect=/x',
      '/register',
      '/auth/oauth',
      '',
      null,
      undefined,
      ['/orgs'],
    ]) {
      expect(safeNext(next)).toBe('/orgs')
    }
  })

  test('the fallback is configurable', () => {
    expect(safeNext(null, '/settings')).toBe('/settings')
  })
})
