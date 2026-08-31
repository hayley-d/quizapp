import { MASTERY_LEVEL_LABELS, type MasteryMovement } from '@/lib/api'
import { plainTextPrompt } from '@/lib/format'

export function MasteryMovementList({ movements }: { movements: MasteryMovement[] }) {
  const moved = movements.filter((movement) => movement.direction !== 'unchanged')

  if (moved.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">No cards changed level this session.</p>
    )
  }

  return (
    <ul className="space-y-1.5 text-sm">
      {moved.map((movement) => (
        <li key={movement.card_id} className="flex items-baseline gap-2">
          <span
            aria-hidden="true"
            className={movement.direction === 'up' ? 'text-success' : 'text-destructive'}
          >
            {movement.direction === 'up' ? '↑' : '↓'}
          </span>
          <span className="line-clamp-1 flex-1">{plainTextPrompt(movement.prompt_md)}</span>
          <span className="shrink-0 text-muted-foreground">
            {MASTERY_LEVEL_LABELS[movement.level_before]} →{' '}
            {MASTERY_LEVEL_LABELS[movement.level_after]}
          </span>
        </li>
      ))}
    </ul>
  )
}
