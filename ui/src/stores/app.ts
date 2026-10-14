import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useTheme } from 'vuetify'
import { SESSION_HINT_KEY } from '@/utils/oauth'

/** Whether a cookie session may exist (a hint, not a credential). */
function readSessionHint(): boolean {
  try {
    return localStorage.getItem(SESSION_HINT_KEY) === '1'
  } catch {
    return false
  }
}

function writeSessionHint(on: boolean) {
  try {
    if (on) localStorage.setItem(SESSION_HINT_KEY, '1')
    else localStorage.removeItem(SESSION_HINT_KEY)
  } catch {
    // Storage blocked: a reload asks for the sign-in again.
  }
}

interface User {
  id: string
  email: string
  username: string
  display_name: string
  avatar: string | null
  locale: string
}

export const useAppStore = defineStore('app', () => {
  const user = ref<User | null>(null)
  const accessToken = ref<string | null>(null)
  // Signed in with Google or GitHub: the session is an HttpOnly cookie the
  // SPA cannot read, sent with every request. This only says one may exist.
  const cookieSession = ref(false)
  const darkMode = ref(false)

  const isAuthenticated = computed(() => !!accessToken.value || cookieSession.value)

  function init() {
    const token = localStorage.getItem('access_token')
    if (token) {
      accessToken.value = token
      fetchMe()
    } else if (readSessionHint()) {
      cookieSession.value = true
      fetchMe()
    }
    const saved = localStorage.getItem('dark_mode')
    if (saved === 'true') {
      darkMode.value = true
    }
  }

  async function register(email: string, username: string, password: string, displayName?: string) {
    const resp = await fetch('/api/auth/register', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({ email, username, password, display_name: displayName || username }),
    })
    const data = await resp.json()
    if (!resp.ok) throw new Error(data.message || 'Registration failed')
    return data
  }

  async function login(email: string, password: string) {
    const resp = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({ email, password }),
    })
    const data = await resp.json()
    if (!resp.ok) throw new Error(data.message || 'Login failed')
    setAuth(data)
    return data
  }

  // The bearer token of a password login, if any. Without one the session
  // cookie authenticates; an `Authorization: Bearer null` would override it.
  function authHeaders(): Record<string, string> {
    return accessToken.value ? { Authorization: `Bearer ${accessToken.value}` } : {}
  }

  async function fetchMe() {
    try {
      const resp = await fetch('/api/me', {
        headers: authHeaders(),
        credentials: 'include',
      })
      if (resp.ok) {
        user.value = await resp.json()
      } else {
        logout()
      }
    } catch {
      logout()
    }
  }

  async function updateProfile(data: Partial<User>) {
    const resp = await fetch('/api/me', {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        ...authHeaders(),
      },
      credentials: 'include',
      body: JSON.stringify(data),
    })
    if (resp.ok) {
      user.value = await resp.json()
    }
  }

  /**
   * After a Google or GitHub sign-in: the server set the session cookie, so
   * load the user with it. Nothing is read from the address.
   */
  async function resumeCookieSession(): Promise<boolean> {
    // A token left from an earlier password login would be sent instead.
    accessToken.value = null
    localStorage.removeItem('access_token')
    try {
      const resp = await fetch('/api/me', { credentials: 'include' })
      if (!resp.ok) return false
      user.value = await resp.json()
      cookieSession.value = true
      writeSessionHint(true)
      return true
    } catch {
      return false
    }
  }

  function setAuth(data: { user: User; access_token: string }) {
    user.value = data.user
    accessToken.value = data.access_token
    localStorage.setItem('access_token', data.access_token)
  }

  function logout() {
    user.value = null
    accessToken.value = null
    cookieSession.value = false
    localStorage.removeItem('access_token')
    writeSessionHint(false)
    fetch('/api/auth/logout', { method: 'POST', credentials: 'include' })
  }

  function toggleDarkMode() {
    darkMode.value = !darkMode.value
    localStorage.setItem('dark_mode', String(darkMode.value))
  }

  return {
    user,
    accessToken,
    darkMode,
    isAuthenticated,
    init,
    register,
    login,
    fetchMe,
    updateProfile,
    resumeCookieSession,
    logout,
    toggleDarkMode,
  }
})
