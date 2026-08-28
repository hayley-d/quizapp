import type { RefObject } from 'react'

import { Markdown } from '@/components/Markdown'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { AnswerResult } from '@/lib/api'

type AnswerVerdictProps = {
  verdict: AnswerResult
  overridden: boolean
  overriding: boolean
  onOverride: () => void
  onNext: () => void
  nextButtonRef: RefObject<HTMLButtonElement | null>
}

export function AnswerVerdict({
  verdict,
  overridden,
  overriding,
  onOverride,
  onNext,
  nextButtonRef,
}: AnswerVerdictProps) {
  const correct = verdict.correct || overridden
  const [primaryExpected, ...alternateExpected] = verdict.expected

  return (
    <div className="space-y-4">
      <div
        role="status"
        className={cn(
          'rounded-lg px-4 py-3',
          correct
            ? 'bg-success text-success-foreground'
            : 'bg-destructive text-destructive-foreground',
        )}
      >
        <p className="font-display font-semibold">
          {correct ? (overridden ? 'Counted as correct' : 'Correct') : 'Not quite'}
        </p>
      </div>

      {primaryExpected !== undefined && (
        <div className="space-y-2">
          <p className="text-sm font-semibold text-muted-foreground">
            {correct ? 'The answer' : 'The answer was'}
          </p>
          <Markdown>{primaryExpected}</Markdown>
          {alternateExpected.length > 0 && (
            <p className="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
              <span>also accepted:</span>
              {alternateExpected.map((wording) => (
                <span key={wording} className="rounded bg-muted px-1.5 py-0.5">
                  {wording}
                </span>
              ))}
            </p>
          )}
        </div>
      )}

      {verdict.explanation_md && (
        <Markdown className="border-t pt-3 text-sm text-muted-foreground">
          {verdict.explanation_md}
        </Markdown>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button ref={nextButtonRef} variant="brand" className="h-10 px-6" onClick={onNext}>
          Next card
        </Button>
        {verdict.can_override && !overridden && (
          <Button variant="secondary" disabled={overriding} onClick={onOverride}>
            I was right
          </Button>
        )}
        <span className="text-sm text-muted-foreground">Enter for the next card</span>
      </div>
    </div>
  )
}
