/**
 * Sign-in with Google and GitHub: the parts with no Vue in them, tested by
 * tests/unit/oauth.test.ts.
 *
 * The server does the whole OAuth exchange. The SPA only:
 * - asks GET /api/auth/providers which providers work here, and shows a
 *   button for each (none when OAuth is off);
 * - sends the browser to /api/oauth/{provider}, a full page navigation;
 * - lands on /auth/oauth with the session in an HttpOnly cookie it cannot
 *   read, and loads the user from GET /api/me;
 * - or lands on /login?oauth_error=<code> and says what the code means.
 *
 * No credential ever passes through an address, so none is read from one.
 */

export type OAuthProvider = 'google' | 'github'

/** The providers this SPA has a button for, in the order they show. */
export const OAUTH_PROVIDERS: readonly OAuthProvider[] = ['google', 'github']

/** The codes a refused sign-in comes back with, on /login?oauth_error=. */
export const OAUTH_ERRORS = [
  'state',
  'cancelled',
  'provider',
  'email_unverified',
  'account_exists',
  'unavailable',
  'server',
] as const

export type OAuthError = (typeof OAUTH_ERRORS)[number]

/** sessionStorage: where the person was going before the round trip. */
export const NEXT_KEY = 'purestat_oauth_next'

/**
 * localStorage: that a cookie session may exist, so a reload tries GET
 * /api/me instead of showing the sign-in page. Not a credential.
 */
export const SESSION_HINT_KEY = 'purestat_session'

/** The providers in a GET /api/auth/providers answer that have a button. */
export function parseProviders(body: unknown): OAuthProvider[] {
  if (!Array.isArray(body)) return []
  return OAUTH_PROVIDERS.filter((p) => body.includes(p))
}

/** Where a provider's button sends the browser. */
export function startPath(provider: OAuthProvider): string {
  return `/api/oauth/${provider}`
}

/** The refusal code in a query value; an unknown one reads as `server`. */
export function oauthError(value: unknown): OAuthError | null {
  const code = Array.isArray(value) ? value[0] : value
  if (typeof code !== 'string' || code === '') return null
  return (OAUTH_ERRORS as readonly string[]).includes(code) ? (code as OAuthError) : 'server'
}

/**
 * Where to go once signed in: a path inside this app, else `fallback`.
 * Never another origin (`//host`, `/\host`, `https://host`), never the API,
 * never back to a sign-in page.
 */
export function safeNext(value: unknown, fallback = '/orgs'): string {
  if (typeof value !== 'string' || !value.startsWith('/')) return fallback
  if (value.startsWith('//') || value.includes('\\')) return fallback
  if (/[\s\u0000-\u001f\u007f]/.test(value)) return fallback
  const path = value.split(/[?#]/, 1)[0]
  const loops = ['/api', '/login', '/register', '/auth']
  if (loops.some((p) => path === p || path.startsWith(`${p}/`))) return fallback
  return value
}
