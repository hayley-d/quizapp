import {
  MASTERY_LEVELS,
  MASTERY_LEVEL_LABELS,
  type MasteryCounts,
  type MasteryLevel,
} from '@/lib/api'

const LEVEL_FILL: Record<MasteryLevel, string> = {
  unseen: 'bg-muted',
  shaky: 'bg-destructive',
  learning: 'bg-brand/40',
  solid: 'bg-brand/70',
  mastered: 'bg-success',
}

const MASTERED_EXPLANATION =
  'Three correct answers in a row, spread over at least twelve hours — a streak inside one sitting is not mastery.'

export function MasteryLadderBar({ counts }: { counts: MasteryCounts }) {
  const total = MASTERY_LEVELS.reduce((running, level) => running + counts[level], 0)
  if (total === 0) {
    return null
  }

  const spokenMix = MASTERY_LEVELS.filter((level) => counts[level] > 0)
    .map((level) => `${counts[level]} ${MASTERY_LEVEL_LABELS[level].toLowerCase()}`)
    .join(', ')

  return (
    <div className="space-y-2">
      <div
        className="flex h-2.5 w-full overflow-hidden rounded-full bg-muted"
        role="img"
        aria-label={`Mastery across ${total} cards: ${spokenMix}`}
      >
        {MASTERY_LEVELS.filter((level) => counts[level] > 0).map((level) => (
          <div
            key={level}
            className={LEVEL_FILL[level]}
            style={{ width: `${(counts[level] / total) * 100}%` }}
          />
        ))}
      </div>
      <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
        {MASTERY_LEVELS.map((level) => (
          <li
            key={level}
            className={`flex items-center gap-1.5 ${counts[level] === 0 ? 'opacity-50' : ''}`}
            title={level === 'mastered' ? MASTERED_EXPLANATION : undefined}
          >
            <span className={`size-2 shrink-0 rounded-full ${LEVEL_FILL[level]}`} />
            {counts[level]} {MASTERY_LEVEL_LABELS[level].toLowerCase()}
          </li>
        ))}
      </ul>
    </div>
  )
}
