import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { useTheme } from 'vuetify'

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
  const darkMode = ref(false)

  const isAuthenticated = computed(() => !!accessToken.value)

  function init() {
    const token = localStorage.getItem('access_token')
    if (token) {
      accessToken.value = token
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
    setAuth(data)
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

  async function fetchMe() {
    try {
      const resp = await fetch('/api/me', {
        headers: { Authorization: `Bearer ${accessToken.value}` },
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
        Authorization: `Bearer ${accessToken.value}`,
      },
      credentials: 'include',
      body: JSON.stringify(data),
    })
    if (resp.ok) {
      user.value = await resp.json()
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
    localStorage.removeItem('access_token')
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
    logout,
    toggleDarkMode,
  }
})
