import type { DeckStatsSummary } from '@/lib/api'
import { formatPercentage, formatRelativeTime } from '@/lib/format'

export function DeckStatsStrip({ summary }: { summary: DeckStatsSummary }) {
  const answeredCount = summary.card_count - summary.unseen_count
  const hasReviews =
    summary.mock_review_count > 0 || summary.practice_review_count > 0

  if (!hasReviews) {
    return <p className="text-sm text-muted-foreground">No sessions yet.</p>
  }

  const parts = [
    `${answeredCount} of ${summary.card_count} answered`,
    `Mock ${formatPercentage(summary.mock_accuracy)} (${summary.mock_review_count})`,
    `Practice ${formatPercentage(summary.practice_accuracy)} (${summary.practice_review_count})`,
  ]
  if (summary.last_answered_at !== null) {
    parts.push(`Last studied ${formatRelativeTime(summary.last_answered_at)}`)
  }

  return (
    <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground">
      {parts.map((part, partIndex) => (
        <span key={part}>
          {partIndex > 0 && <span className="mr-2 opacity-60">·</span>}
          {part}
        </span>
      ))}
    </p>
  )
}
