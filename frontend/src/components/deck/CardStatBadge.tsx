import { MASTERY_LEVEL_LABELS, type CardStats, type MasteryLevel } from '@/lib/api'
import { Badge } from '@/components/ui/badge'

const LEVEL_EMPHASIS: Record<MasteryLevel, string | undefined> = {
  unseen: undefined,
  shaky: 'bg-destructive text-destructive-foreground',
  learning: undefined,
  solid: undefined,
  mastered: 'bg-success text-success-foreground',
}

export function CardStatBadge({ stats }: { stats: CardStats | null }) {
  if (stats === null) {
    return <Badge variant="secondary">Unseen</Badge>
  }

  const missPercentage = Math.round(stats.miss_rate * 100)

  return (
    <Badge
      variant="secondary"
      className={LEVEL_EMPHASIS[stats.mastery_level]}
      title={`Missed ${missPercentage}% of the last ${stats.attempt_count} attempt${
        stats.attempt_count === 1 ? '' : 's'
      }`}
    >
      {MASTERY_LEVEL_LABELS[stats.mastery_level]} · {missPercentage}% missed · {stats.attempt_count}
    </Badge>
  )
}
