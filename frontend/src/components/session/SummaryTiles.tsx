import type { SessionSummary } from '@/lib/api'
import { formatDuration } from '@/lib/format'

type SummaryTilesProps = {
  summary: SessionSummary
}

export function SummaryTiles({ summary }: SummaryTilesProps) {
  return (
    <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
      <div className="rounded-xl border bg-card px-4 py-3 shadow-sm">
        <dt className="text-sm text-muted-foreground">Answered</dt>
        <dd className="font-display text-2xl font-bold">{summary.answered_count}</dd>
      </div>
      <div className="rounded-xl border bg-card px-4 py-3 shadow-sm">
        <dt className="text-sm text-muted-foreground">Correct</dt>
        <dd className="font-display text-2xl font-bold">{summary.correct_count}</dd>
      </div>
      <div className="rounded-xl border bg-card px-4 py-3 shadow-sm">
        <dt className="text-sm text-muted-foreground">Accuracy</dt>
        <dd className="font-display text-2xl font-bold">
          {summary.accuracy === null ? '—' : `${Math.round(summary.accuracy * 100)}%`}
        </dd>
      </div>
      <div className="rounded-xl border bg-card px-4 py-3 shadow-sm">
        <dt className="text-sm text-muted-foreground">Time</dt>
        <dd className="font-display text-2xl font-bold">
          {formatDuration(summary.total_ms)}
        </dd>
      </div>
    </dl>
  )
}
