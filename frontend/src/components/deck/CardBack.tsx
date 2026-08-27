import type { Card } from '@/lib/api'
import { Markdown } from '@/components/Markdown'
import { cn } from '@/lib/utils'

type Props = {
  card: Card
  /** Multiple choice only: whether the correct option is shown as correct. */
  revealed: boolean
}

const LETTERS = 'ABCDEFGHIJ'

/**
 * Unrevealed options are deliberately uniform: the back of a multiple-choice
 * card is a self-test, so nothing may hint at the answer until the eye button
 * is pressed. `--success` is the theme's designated correct-answer colour.
 */
function choiceClass(isCorrect: boolean, revealed: boolean): string {
  if (!revealed) return 'bg-accent/85 text-accent-foreground'
  return isCorrect
    ? 'bg-success text-success-foreground font-medium'
    : 'bg-muted text-muted-foreground'
}

/**
 * A card's answer. One component per kind would triple the file count for
 * three small branches that share their explanation footer, so they live
 * together here.
 */
export function CardBack({ card, revealed }: Props) {
  return (
    <div className="space-y-3">
      {card.kind === 'mc_single' && (
        <ul className="grid gap-2 sm:grid-cols-2">
          {card.choices.map((c, i) => (
            <li
              key={c.id}
              className={cn(
                'flex items-start gap-1.5 rounded-lg px-3 py-2 text-sm transition-colors',
                choiceClass(c.is_correct, revealed),
              )}
            >
              <span className="font-semibold">{LETTERS[i] ?? '•'}.</span>
              <Markdown className="min-w-0 flex-1">{c.text_md}</Markdown>
              {revealed && c.is_correct && <span className="sr-only">Correct answer</span>}
            </li>
          ))}
        </ul>
      )}

      {card.kind === 'short_answer' && <ShortAnswer card={card} />}

      {card.kind === 'flashcard' && <Markdown>{card.answer_md ?? ''}</Markdown>}

      {/* One field on `cards` with no per-kind rule behind it, so it shows for
          every kind rather than only flashcards. */}
      {card.explanation_md && (
        <Markdown className="border-t pt-2 text-sm text-muted-foreground">
          {card.explanation_md}
        </Markdown>
      )}
    </div>
  )
}

/** The API orders `accepted` by `is_primary DESC, id`, so the primary is first. */
function ShortAnswer({ card }: { card: Card }) {
  const [primary, ...alternates] = card.accepted
  return (
    <div className="space-y-2">
      <p className="font-medium">{primary?.text ?? '—'}</p>
      {alternates.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-muted-foreground">also accepted:</span>
          {alternates.map((a) => (
            <span
              key={a.id}
              className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground"
            >
              {a.text}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
