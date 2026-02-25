import 'vuetify/styles'
import '@mdi/font/css/materialdesignicons.css'
import { createVuetify } from 'vuetify'

const purestatLight = {
  dark: false,
  colors: {
    primary: '#4F46E5',
    'primary-lighten-1': '#6366F1',
    'primary-lighten-4': '#E0E7FF',
    secondary: '#059669',
    accent: '#F59E0B',
    error: '#E11D48',
    warning: '#F59E0B',
    info: '#4F46E5',
    success: '#059669',
    background: '#F8FAFC',
    surface: '#FFFFFF',
    'on-background': '#334155',
    'on-surface': '#334155',
    'surface-variant': '#F1F5F9',
  },
}

const purestatDark = {
  dark: true,
  colors: {
    primary: '#6366F1',
    'primary-lighten-1': '#818CF8',
    'primary-lighten-4': '#312E81',
    secondary: '#34D399',
    accent: '#FBBF24',
    error: '#FB7185',
    warning: '#FBBF24',
    info: '#818CF8',
    success: '#34D399',
    background: '#0F172A',
    surface: '#1E293B',
    'on-background': '#E2E8F0',
    'on-surface': '#E2E8F0',
    'surface-variant': '#334155',
  },
}

export default createVuetify({
  theme: {
    defaultTheme: 'purestatLight',
    themes: {
      purestatLight,
      purestatDark,
    },
  },
  defaults: {
    VBtn: {
      variant: 'flat',
      rounded: 'lg',
    },
    VCard: {
      rounded: 'lg',
      elevation: 0,
      border: true,
    },
    VTextField: {
      variant: 'outlined',
      density: 'comfortable',
    },
    VSelect: {
      variant: 'outlined',
      density: 'comfortable',
    },
    VDataTable: {
      density: 'comfortable',
    },
  },
})
