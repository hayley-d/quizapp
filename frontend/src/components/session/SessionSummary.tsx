import { Link } from 'react-router-dom'

import { SummaryTiles } from '@/components/session/SummaryTiles'
import { Button } from '@/components/ui/button'
import type { SessionSummary as SessionSummaryData } from '@/lib/api'

type SessionSummaryProps = {
  summary: SessionSummaryData
}

export function SessionSummary({ summary }: SessionSummaryProps) {
  return (
    <div className="max-w-xl space-y-6">
      <h1 className="font-display text-2xl font-bold">Session finished</h1>
      <SummaryTiles summary={summary} />
      {summary.overridden_count > 0 && (
        <p className="text-sm text-muted-foreground">
          {summary.overridden_count} counted correct by override.
        </p>
      )}
      <div className="flex gap-3">
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    </div>
  )
}
