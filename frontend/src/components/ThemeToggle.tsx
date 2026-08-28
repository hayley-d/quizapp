import type { ComponentType } from 'react'
import { Monitor } from 'lucide-react'

import { BibbleIcon } from '@/components/icons/BibbleIcon'
import { MakkaPakkaIcon } from '@/components/icons/MakkaPakkaIcon'
import { useTheme, type ThemePreference } from '@/hooks/useTheme'
import { cn } from '@/lib/utils'

type ThemeOption = {
  value: ThemePreference
  label: string
  Icon: ComponentType<{ className?: string }>
}

const THEME_OPTIONS: ThemeOption[] = [
  { value: 'light', label: 'Makka Pakka (light)', Icon: MakkaPakkaIcon },
  { value: 'dark', label: 'Bibble (dark)', Icon: BibbleIcon },
  { value: 'system', label: 'System', Icon: Monitor },
]

export function ThemeToggle() {
  const { preference, setPreference } = useTheme()

  return (
    <div
      role="group"
      aria-label="Colour theme"
      className="ml-auto flex items-center gap-0.5 rounded-lg bg-muted p-0.5"
    >
      {THEME_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={preference === option.value}
          aria-label={option.label}
          title={option.label}
          onClick={() => setPreference(option.value)}
          className={cn(
            'rounded-md p-1.5 transition-colors',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
            preference === option.value
              ? 'bg-background text-foreground shadow-sm'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          <option.Icon className="size-4" />
        </button>
      ))}
    </div>
  )
}
