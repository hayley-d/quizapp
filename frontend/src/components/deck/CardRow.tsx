import { useEffect, useRef, useState } from 'react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import {
  Archive as ArchiveIcon,
  ArchiveRestore,
  Eye,
  EyeOff,
  GripVertical,
  Pencil,
} from 'lucide-react'
import { toast } from 'sonner'
import { KIND_LABEL, type Card, type CardSummary } from '@/lib/api'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { CardBack } from '@/components/deck/CardBack'
import { useFlip } from '@/components/deck/useFlip'
import { cn } from '@/lib/utils'

type Props = {
  card: CardSummary
  /** Fetches the full card. Supplied by the page so the row owns no api import policy. */
  loadCard: (id: number, signal: AbortSignal) => Promise<Card>
  onEdit: () => void
  onArchiveToggle: () => void
}

/**
 * One card in the deck list.
 *
 * The answer is fetched here, per row, and kept for the row's lifetime. A
 * page-level cache would need an eviction rule; this needs none — the only
 * thing that invalidates an answer is an edit, and editing navigates to
 * `/cards/:id/edit`, which remounts the whole page on return. Archive and
 * unarchive change no answer content.
 */
export function CardRow({ card, loadCard, onEdit, onArchiveToggle }: Props) {
  const [full, setFull] = useState<Card | null>(null)
  const [loading, setLoading] = useState(false)
  const [revealed, setRevealed] = useState(false)
  const inFlight = useRef<AbortController | null>(null)

  const { face, flip, toFront, rotatorStyle, perspectiveStyle } = useFlip()

  // Driven by `face` rather than by a callback passed into useFlip: the fetch
  // needs `toFront` for its failure path, and useFlip returns that, so a
  // callback would have to close over a value declared after it.
  useEffect(() => {
    if (face !== 'back') {
      // Flipping away abandons a fetch still in the air — by the time it
      // landed the user would be looking at the question again.
      inFlight.current?.abort()
      inFlight.current = null
      setLoading(false)
      return
    }
    if (full || inFlight.current) return

    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    loadCard(card.id, controller.signal)
      .then(setFull)
      .catch((e: unknown) => {
        if ((e as Error)?.name === 'AbortError') return
        toast.error('Could not load the answer')
        toFront()
      })
      .finally(() => {
        if (inFlight.current === controller) {
          inFlight.current = null
          setLoading(false)
        }
      })
  }, [card.id, face, full, loadCard, toFront])

  // Unmount mid-flight: abort rather than resolve into a dead component.
  useEffect(() => () => inFlight.current?.abort(), [])

  // Revealing is per visit to the answer, not per card: flipping back to the
  // question and forward again should be a fresh self-test.
  useEffect(() => {
    if (face === 'front') setRevealed(false)
  }, [face])

  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: card.id })

  const showingAnswer = face === 'back'

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={cn('flex items-start gap-1', isDragging && 'relative z-10 opacity-80')}
    >
      <button
        type="button"
        ref={setActivatorNodeRef}
        {...attributes}
        {...listeners}
        aria-label={`Reorder ${card.prompt_md.slice(0, 40)}`}
        title="Drag to reorder"
        className={cn(
          'mt-4 shrink-0 cursor-grab touch-none rounded-md p-1 text-muted-foreground',
          'hover:bg-muted hover:text-foreground active:cursor-grabbing',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        )}
      >
        <GripVertical className="size-4" />
      </button>

      <div
        style={perspectiveStyle}
        className={cn('min-w-0 flex-1', card.archived && 'opacity-60')}
      >
        <div
          style={rotatorStyle}
          className="space-y-3 rounded-xl border bg-card p-3 shadow-sm"
        >
          {/* Header strip. Outside the flip target below, which keeps these
              controls out of the flip and avoids nesting buttons in a button. */}
          <div className="flex items-center gap-2">
            <Badge variant="outline">{KIND_LABEL[card.kind]}</Badge>
            {card.archived && <Badge variant="secondary">Archived</Badge>}

            <Button
              variant="secondary"
              size="icon-sm"
              className="rounded-full"
              aria-label={`Edit card ${card.id}`}
              title="Edit card"
              onClick={onEdit}
            >
              <Pencil />
            </Button>
            <Button
              variant="secondary"
              size="icon-sm"
              className="rounded-full"
              aria-label={`${card.archived ? 'Unarchive' : 'Archive'} card ${card.id}`}
              title={card.archived ? 'Unarchive card' : 'Archive card'}
              onClick={onArchiveToggle}
            >
              {card.archived ? <ArchiveRestore /> : <ArchiveIcon />}
            </Button>

            {card.kind === 'mc_single' && showingAnswer && full && (
              <Button
                variant="secondary"
                size="icon-sm"
                className="ml-auto rounded-full"
                aria-pressed={revealed}
                aria-label={revealed ? 'Hide answer' : 'Reveal answer'}
                title={revealed ? 'Hide answer' : 'Reveal answer'}
                onClick={() => setRevealed((r) => !r)}
              >
                {revealed ? <EyeOff /> : <Eye />}
              </Button>
            )}
          </div>

          {/* The flip target. A div rather than a button because the image
              thumbnail inside it is itself a button. */}
          <div
            role="button"
            tabIndex={0}
            aria-label={showingAnswer ? 'Show question' : 'Show answer'}
            onClick={flip}
            onKeyDown={(e) => {
              // Only keys pressed on the card body itself. A keydown from a
              // focusable descendant — the image thumbnail's button, or a link
              // in the markdown — must reach its own default action: Enter's
              // default action IS the button's click, so preventing it here
              // would make the lightbox unreachable by keyboard.
              if (e.target !== e.currentTarget) return
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                flip()
              }
            }}
            className="cursor-pointer rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {showingAnswer ? (
              loading || !full ? (
                <div className="space-y-2" aria-busy="true">
                  <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
                  <div className="h-4 w-1/3 animate-pulse rounded bg-muted" />
                </div>
              ) : (
                <CardBack card={full} revealed={revealed} />
              )
            ) : (
              <div className="flex items-start justify-between gap-3">
                {/* Unclamped on purpose: a truncated single line cannot render
                    markdown without a half-open `$…$` or a stray list marker
                    looking broken. Recorded trade-off from Part 2b. */}
                <Markdown className="min-w-0 flex-1">{card.prompt_md}</Markdown>
                {card.image_path !== null && (
                  // Keeps its lightbox, so it is a deliberate non-flipping
                  // region: the click must not reach the flip handler.
                  <div onClick={(e) => e.stopPropagation()}>
                    <CardImage path={card.image_path} alt={card.prompt_md} />
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </li>
  )
}
