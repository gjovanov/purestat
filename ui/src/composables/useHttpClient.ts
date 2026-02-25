import { ref } from 'vue'
import { useAppStore } from '@/stores/app'
import router from '@/router'

const BASE_URL = '/api'

interface RequestOptions {
  method?: string
  body?: unknown
  headers?: Record<string, string>
}

export function useHttpClient() {
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
    loading.value = true
    error.value = null

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...options.headers,
    }

    const appStore = useAppStore()
    if (appStore.accessToken) {
      headers['Authorization'] = `Bearer ${appStore.accessToken}`
    }

    try {
      const resp = await fetch(`${BASE_URL}${path}`, {
        method: options.method || 'GET',
        headers,
        body: options.body ? JSON.stringify(options.body) : undefined,
        credentials: 'include',
      })

      if (resp.status === 401) {
        appStore.logout()
        router.push({ name: 'login' })
        throw new Error('Unauthorized')
      }

      if (resp.status === 204) {
        return undefined as T
      }

      const data = await resp.json()

      if (!resp.ok) {
        const msg = data.message || data.error || 'Request failed'
        error.value = msg
        throw new Error(msg)
      }

      return data as T
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Unknown error'
      error.value = msg
      throw e
    } finally {
      loading.value = false
    }
  }

  const get = <T>(path: string) => request<T>(path)
  const post = <T>(path: string, body?: unknown) => request<T>(path, { method: 'POST', body })
  const put = <T>(path: string, body?: unknown) => request<T>(path, { method: 'PUT', body })
  const del = <T>(path: string) => request<T>(path, { method: 'DELETE' })

  return { loading, error, get, post, put, del }
}
