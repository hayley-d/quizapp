import { Link } from 'react-router-dom'

import { ResultRow } from '@/components/session/ResultRow'
import { SummaryTiles } from '@/components/session/SummaryTiles'
import { Button } from '@/components/ui/button'
import type { SessionResults } from '@/lib/api'

type MockResultsProps = {
  results: SessionResults
  onOverride: (reviewId: number) => void
  overridingReviewId: number | null
}

export function MockResults({ results, onOverride, overridingReviewId }: MockResultsProps) {
  const hasFlashcards = results.questions.some((question) => question.kind === 'flashcard')

  return (
    <div className="space-y-6">
      <h1 className="font-display text-2xl font-bold">Mock test finished</h1>
      <SummaryTiles summary={results.summary} />

      {results.summary.overridden_count > 0 && (
        <p className="text-sm text-muted-foreground">
          {results.summary.overridden_count} counted correct by you.
        </p>
      )}

      {hasFlashcards && (
        <p className="rounded-xl border bg-card px-4 py-3 text-sm text-muted-foreground shadow-sm">
          Flashcards are matched against the answer you wrote on the card, so a longer answer
          may be marked wrong even when you knew it. Mark the ones you got right.
        </p>
      )}

      {results.questions.length === 0 ? (
        <p className="text-sm text-muted-foreground">You did not answer any questions.</p>
      ) : (
        <ol className="space-y-4">
          {results.questions.map((question, questionIndex) => (
            <ResultRow
              key={question.review_id}
              question={question}
              position={questionIndex + 1}
              onOverride={onOverride}
              overriding={overridingReviewId === question.review_id}
            />
          ))}
        </ol>
      )}

      <div className="flex gap-3">
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    </div>
  )
}
