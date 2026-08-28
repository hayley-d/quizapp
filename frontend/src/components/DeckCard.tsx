import { Pencil } from 'lucide-react'
import { Link } from 'react-router-dom'
import type { Deck } from '@/lib/api'

type DeckCardProps = {
  deck: Deck
  onEdit: () => void
  onFilterModule: (moduleId: number) => void
}

export function DeckCard({ deck, onEdit, onFilterModule }: DeckCardProps) {
  return (
    <article className="flex min-h-64 flex-col overflow-hidden rounded-xl bg-[var(--deck-card)]">
      <div className="flex min-h-16 items-center justify-between gap-3 bg-[var(--deck-card-header)] px-4">
        {deck.module_id === null ? (
          <span />
        ) : (
          <button
            type="button"
            onClick={() => onFilterModule(deck.module_id as number)}
            title={`Filter by ${deck.module_name}`}
            className="rounded-full bg-[var(--deck-card-chip)] px-5 py-1.5 text-sm font-medium text-deck-card-chip-foreground transition hover:brightness-125"
          >
            {deck.module_name}
          </button>
        )}
        <button
          type="button"
          onClick={onEdit}
          aria-label={`Edit ${deck.name}`}
          title="Edit deck"
          className="rounded-full bg-[var(--deck-card-chip)] px-4 py-2.5 text-deck-card-chip-foreground transition hover:brightness-125"
        >
          <Pencil className="size-5" />
        </button>
      </div>

      <Link
        to={`/decks/${deck.id}`}
        className="flex flex-1 flex-col gap-1 px-5 py-4"
      >
        <h2 className="font-display text-3xl leading-tight font-normal text-deck-card-foreground">
          {deck.name}
        </h2>
        {deck.description && <p className="text-deck-card-foreground/90">{deck.description}</p>}
        <span className="mt-auto pt-4 text-sm text-deck-card-foreground/80">
          {deck.card_count} card{deck.card_count === 1 ? '' : 's'}
        </span>
      </Link>
    </article>
  )
}
