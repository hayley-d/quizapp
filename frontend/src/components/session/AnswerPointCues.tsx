import { Button } from '@/components/ui/button'
import type { NextAnswerPoints } from '@/lib/api'

type AnswerPointCuesProps = {
  answerPoints: NextAnswerPoints
  hintShown: boolean
  onShowHint: () => void
}

export function AnswerPointCues({ answerPoints, hintShown, onShowHint }: AnswerPointCuesProps) {
  const { total, full_total: fullTotal, focused, cues } = answerPoints
  const shown = hintShown && cues.behind_the_hint.length > 0 ? cues.behind_the_hint : cues.visible
  const hintAvailable = !hintShown && cues.behind_the_hint.length > 0

  return (
    <div className="space-y-3 rounded-xl border bg-card p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-sm font-semibold text-muted-foreground">
          {focused
            ? `You recalled ${fullTotal - total} of ${fullTotal} last time — name the ${total} you missed`
            : `Name all ${total}`}
        </p>
        {hintAvailable && (
          <Button variant="ghost" size="sm" onClick={onShowHint}>
            Need a hint
          </Button>
        )}
      </div>

      {shown.length > 0 ? (
        <ul className="flex flex-wrap gap-2" aria-label="Retrieval cues">
          {shown.map((cue, cueIndex) => (
            <li
              key={`${cue}-${cueIndex}`}
              className="rounded-md border bg-muted px-2.5 py-1 font-mono text-sm"
            >
              {cue}
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-sm text-muted-foreground">
          No cues at this level — recall them from the question alone.
        </p>
      )}
    </div>
  )
}
