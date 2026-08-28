import { useCallback, useEffect, useState } from 'react'

export type ThemePreference = 'light' | 'dark' | 'system'

export const THEME_STORAGE_KEY = 'quizapp-theme'

const LIGHT_QUERY = '(prefers-color-scheme: light)'

function readStoredPreference(): ThemePreference {
  let stored: string | null = null
  try {
    stored = window.localStorage.getItem(THEME_STORAGE_KEY)
  } catch {
    stored = null
  }
  return stored === 'light' || stored === 'dark' ? stored : 'system'
}

function applyPreference(preference: ThemePreference) {
  const isLight =
    preference === 'light' ||
    (preference === 'system' && window.matchMedia(LIGHT_QUERY).matches)
  document.documentElement.classList.toggle('dark', !isLight)
}

export function useTheme() {
  const [preference, setPreferenceState] = useState<ThemePreference>(readStoredPreference)

  useEffect(() => {
    applyPreference(preference)
    if (preference !== 'system') return
    const mediaQuery = window.matchMedia(LIGHT_QUERY)
    function handleChange() {
      applyPreference('system')
    }
    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [preference])

  const setPreference = useCallback((next: ThemePreference) => {
    try {
      if (next === 'system') {
        window.localStorage.removeItem(THEME_STORAGE_KEY)
      } else {
        window.localStorage.setItem(THEME_STORAGE_KEY, next)
      }
    } catch {}
    setPreferenceState(next)
  }, [])

  return { preference, setPreference }
}
