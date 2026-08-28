import { Sparkles } from 'lucide-react'

type StreakBadgeProps = {
  streak: number
}

export function StreakBadge({ streak }: StreakBadgeProps) {
  return (
    <span
      key={streak}
      className="wing-flutter inline-flex items-center gap-1 rounded-full bg-streak px-2.5 py-0.5 text-xs font-semibold text-accent-foreground"
    >
      <Sparkles className="size-3" />
      {streak} in a row
    </span>
  )
}
