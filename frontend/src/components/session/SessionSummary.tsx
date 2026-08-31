import { Link } from 'react-router-dom'

import { MasteryMovementList } from '@/components/session/MasteryMovementList'
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
      <div className="space-y-2">
        <h2 className="font-display text-base font-semibold">
          {summary.mastery_goal === null
            ? 'What moved'
            : `Goal: ${summary.mastery_goal} · moved up: ${summary.mastery_moved_up_count}`}
        </h2>
        <MasteryMovementList movements={summary.mastery_movements} />
      </div>
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
