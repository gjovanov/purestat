export function useValidation() {
  const rules = {
    required: (v: unknown) => !!v || v === 0 || 'This field is required',
    email: (v: string) => !v || /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v) || 'Invalid email address',
    minLength: (n: number) => (v: string) => !v || v.length >= n || `Must be at least ${n} characters`,
    maxLength: (n: number) => (v: string) => !v || v.length <= n || `Must be at most ${n} characters`,
    positiveNumber: (v: unknown) => (!v && v !== 0) || (Number(v) > 0) || 'Must be a positive number',
    slug: (v: string) => !v || /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(v) || 'Only lowercase letters, numbers and hyphens',
  }

  return { rules }
}
