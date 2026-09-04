import { Markdown } from '@/components/Markdown'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { RevealAnswerPoints } from '@/lib/api'

type AnswerPointsChecklistProps = {
  answerPoints: RevealAnswerPoints
  recalledKeys: string[]
  onToggle: (key: string) => void
  onSubmit: () => void
  disabled: boolean
}

export function AnswerPointsChecklist({
  answerPoints,
  recalledKeys,
  onToggle,
  onSubmit,
  disabled,
}: AnswerPointsChecklistProps) {
  const { points, notes } = answerPoints

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <p className="text-sm font-semibold text-muted-foreground">
          Tick every point you actually recalled
        </p>
        <p className="font-display text-lg font-semibold">
          {recalledKeys.length} of {points.length}
        </p>
      </div>

      <ul className="space-y-2">
        {points.map((point, pointIndex) => {
          const recalled = recalledKeys.includes(point.key)
          return (
            <li key={point.key}>
              <button
                type="button"
                aria-pressed={recalled}
                onClick={() => onToggle(point.key)}
                className={cn(
                  'flex w-full items-start gap-3 rounded-lg border px-3 py-2 text-left transition-colors',
                  recalled
                    ? 'border-success/50 bg-success/10'
                    : 'border-border bg-card hover:bg-muted',
                )}
              >
                <span
                  aria-hidden
                  className={cn(
                    'mt-0.5 flex size-5 shrink-0 items-center justify-center rounded border font-mono text-xs',
                    recalled
                      ? 'border-success bg-success text-success-foreground'
                      : 'border-muted-foreground/40 text-muted-foreground',
                  )}
                >
                  {recalled ? '✓' : pointIndex + 1}
                </span>
                <Markdown className="min-w-0 flex-1">{point.text_md}</Markdown>
                {point.matched_what_you_typed && (
                  <span className="mt-0.5 shrink-0 rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                    matched
                  </span>
                )}
              </button>
            </li>
          )
        })}
      </ul>

      {notes.length > 0 && (
        <div className="space-y-1 border-t pt-3">
          {notes.map((note) => (
            <Markdown key={note} className="text-sm text-muted-foreground">
              {note}
            </Markdown>
          ))}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button variant="brand" className="h-10 px-6" disabled={disabled} onClick={onSubmit}>
          Score it
        </Button>
        <span className="text-sm text-muted-foreground">
          1–9 to toggle a point · Enter to score
        </span>
      </div>
    </div>
  )
}
