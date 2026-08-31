import type { Card } from '@/lib/api'
import { Markdown } from '@/components/Markdown'
import { cn } from '@/lib/utils'

type CardBackProps = {
  card: Card
  revealed: boolean
}

const LETTERS = 'ABCDEFGHIJ'

function choiceClass(isCorrect: boolean, revealed: boolean): string {
  if (!revealed) return 'bg-secondary text-secondary-foreground [&_strong]:font-normal'
  return isCorrect
    ? 'bg-primary text-primary-foreground font-medium'
    : 'bg-muted text-muted-foreground'
}

export function CardBack({ card, revealed }: CardBackProps) {
  return (
    <div className="space-y-3">
      {card.kind === 'mc_single' && (
        <ul className="grid gap-2 sm:grid-cols-2">
          {card.choices.map((choice, choiceIndex) => (
            <li
              key={choice.id}
              className={cn(
                'flex items-start gap-1.5 rounded-lg px-3 py-2 text-sm transition-colors',
                choiceClass(choice.is_correct, revealed),
              )}
            >
              <span className="font-semibold">{LETTERS[choiceIndex] ?? '•'}.</span>
              <Markdown className="min-w-0 flex-1">{choice.text_md}</Markdown>
              {revealed && choice.is_correct && (
                <span className="sr-only">Correct answer</span>
              )}
            </li>
          ))}
        </ul>
      )}

      {card.kind === 'text_answer' && <TextAnswer card={card} />}

      {card.kind === 'flashcard' && <Markdown>{card.answer_md ?? ''}</Markdown>}

      {card.explanation_md && (
        <Markdown className="border-t pt-2 text-sm text-muted-foreground">
          {card.explanation_md}
        </Markdown>
      )}
    </div>
  )
}

function TextAnswer({ card }: { card: Card }) {
  const [primary, ...alternates] = card.accepted
  return (
    <div className="space-y-2">
      <p className="font-medium">{primary?.text ?? '—'}</p>
      {alternates.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-muted-foreground">also accepted:</span>
          {alternates.map((alternate) => (
            <span
              key={alternate.id}
              className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground"
            >
              {alternate.text}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
