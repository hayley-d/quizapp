import { useState } from 'react'

import { Markdown } from '@/components/Markdown'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { AnswerPointScore } from '@/lib/api'

type RecalledPointsCorrectionProps = {
  answerPoints: AnswerPointScore
  onCorrect: (newlyRecalledPointKeys: string[]) => void
  correcting: boolean
}

export function RecalledPointsCorrection({
  answerPoints,
  onCorrect,
  correcting,
}: RecalledPointsCorrectionProps) {
  const [newlyRecalledPointKeys, setNewlyRecalledPointKeys] = useState<string[]>([])

  function togglePoint(key: string) {
    setNewlyRecalledPointKeys((current) =>
      current.includes(key) ? current.filter((existing) => existing !== key) : [...current, key],
    )
  }

  return (
    <div className="space-y-3 border-t pt-3">
      <p className="text-sm font-semibold text-muted-foreground">
        Tick any point you did recall but the matcher missed
      </p>

      <ul className="space-y-2">
        {answerPoints.points.map((point) => {
          const alreadyCredited = point.recalled
          const ticked = alreadyCredited || newlyRecalledPointKeys.includes(point.key)
          return (
            <li key={point.key}>
              <button
                type="button"
                aria-pressed={ticked}
                disabled={alreadyCredited || correcting}
                onClick={() => togglePoint(point.key)}
                className={cn(
                  'flex w-full items-start gap-3 rounded-lg border px-3 py-2 text-left transition-colors',
                  ticked ? 'border-success/50 bg-success/10' : 'border-border bg-card',
                  alreadyCredited
                    ? 'cursor-default opacity-70'
                    : !correcting && 'hover:bg-muted',
                )}
              >
                <span
                  aria-hidden
                  className={cn(
                    'mt-0.5 flex size-5 shrink-0 items-center justify-center rounded border font-mono text-xs',
                    ticked
                      ? 'border-success bg-success text-success-foreground'
                      : 'border-muted-foreground/40 text-muted-foreground',
                  )}
                >
                  {ticked ? '✓' : ''}
                </span>
                <Markdown className="min-w-0 flex-1">{point.text_md}</Markdown>
                {alreadyCredited && (
                  <span className="mt-0.5 shrink-0 rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                    matched
                  </span>
                )}
              </button>
            </li>
          )
        })}
      </ul>

      <Button
        variant="secondary"
        className="h-8 px-3 text-sm"
        disabled={correcting || newlyRecalledPointKeys.length === 0}
        onClick={() => onCorrect(newlyRecalledPointKeys)}
      >
        {correcting ? 'Rescoring…' : 'I recalled these too'}
      </Button>
    </div>
  )
}
