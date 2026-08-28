import { Link } from 'react-router-dom'

import { Button } from '@/components/ui/button'
import type { SessionSummary as SessionSummaryData } from '@/lib/api'

function formatDuration(totalMilliseconds: number): string {
  const totalSeconds = Math.round(totalMilliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes === 0 ? `${seconds}s` : `${minutes}m ${seconds}s`
}

type SessionSummaryProps = {
  summary: SessionSummaryData
}

export function SessionSummary({ summary }: SessionSummaryProps) {
  return (
    <div className="max-w-xl space-y-6">
      <h1 className="font-display text-2xl font-bold">Session finished</h1>
      <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div>
          <dt className="text-sm text-muted-foreground">Answered</dt>
          <dd className="font-display text-2xl font-bold">{summary.answered_count}</dd>
        </div>
        <div>
          <dt className="text-sm text-muted-foreground">Correct</dt>
          <dd className="font-display text-2xl font-bold">{summary.correct_count}</dd>
        </div>
        <div>
          <dt className="text-sm text-muted-foreground">Accuracy</dt>
          <dd className="font-display text-2xl font-bold">
            {summary.accuracy === null ? '—' : `${Math.round(summary.accuracy * 100)}%`}
          </dd>
        </div>
        <div>
          <dt className="text-sm text-muted-foreground">Time</dt>
          <dd className="font-display text-2xl font-bold">
            {formatDuration(summary.total_ms)}
          </dd>
        </div>
      </dl>
      {summary.overridden_count > 0 && (
        <p className="text-sm text-muted-foreground">
          {summary.overridden_count} counted correct by override.
        </p>
      )}
      <div className="flex gap-3">
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/study">Study again</Link>
        </Button>
        <Button variant="secondary" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    </div>
  )
}
