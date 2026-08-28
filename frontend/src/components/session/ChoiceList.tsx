import { Markdown } from '@/components/Markdown'
import { cn } from '@/lib/utils'
import type { NextChoice } from '@/lib/api'

const LETTERS = 'ABCDEFGHI'

type ChoiceListProps = {
  choices: NextChoice[]
  selectedChoiceId: number | null
  onSelect: (choiceId: number) => void
  disabled: boolean
}

export function ChoiceList({
  choices,
  selectedChoiceId,
  onSelect,
  disabled,
}: ChoiceListProps) {
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {choices.map((choice, choiceIndex) => {
        const selected = choice.id === selectedChoiceId
        return (
          <li key={choice.id}>
            <button
              type="button"
              disabled={disabled}
              aria-pressed={selected}
              onClick={() => onSelect(choice.id)}
              className={cn(
                'flex w-full items-start gap-2 rounded-lg px-3 py-2 text-left text-sm transition-colors',
                'focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-ring/50',
                'disabled:cursor-not-allowed',
                selected
                  ? 'bg-primary text-primary-foreground font-medium'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
              )}
            >
              <span className="font-semibold tabular-nums">
                {LETTERS[choiceIndex] ?? '•'}.
              </span>
              <Markdown className="min-w-0 flex-1">{choice.text_md}</Markdown>
              <span className="shrink-0 text-xs opacity-60">{choiceIndex + 1}</span>
            </button>
          </li>
        )
      })}
    </ul>
  )
}
