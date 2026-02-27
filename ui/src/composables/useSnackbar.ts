import { reactive } from 'vue'

const state = reactive({
  show: false,
  text: '',
  color: 'error',
  timeout: 5000,
})

export function useSnackbar() {
  function showSnackbar(text: string, color = 'error', timeout = 5000) {
    state.text = text
    state.color = color
    state.timeout = timeout
    state.show = true
  }

  function showError(text: string) {
    showSnackbar(text, 'error', 5000)
  }

  function showSuccess(text: string) {
    showSnackbar(text, 'success', 3000)
  }

  function hideSnackbar() {
    state.show = false
  }

  return { state, showSnackbar, showError, showSuccess, hideSnackbar }
}
