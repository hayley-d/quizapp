import type { CardStats } from '@/lib/api'
import { Badge } from '@/components/ui/badge'

const EMPHASIS_THRESHOLD = 0.4

export function CardStatBadge({ stats }: { stats: CardStats | null }) {
  if (stats === null) {
    return <Badge variant="secondary">Unseen</Badge>
  }

  const missPercentage = Math.round(stats.miss_rate * 100)
  const emphasised = stats.miss_rate >= EMPHASIS_THRESHOLD

  return (
    <Badge
      variant="secondary"
      className={
        emphasised ? 'bg-destructive text-destructive-foreground' : undefined
      }
      title={`Missed ${missPercentage}% of the last ${stats.attempt_count} attempt${
        stats.attempt_count === 1 ? '' : 's'
      }`}
    >
      {missPercentage}% missed · {stats.attempt_count}
    </Badge>
  )
}
