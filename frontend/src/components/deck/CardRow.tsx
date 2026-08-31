import { useEffect, useRef, useState } from 'react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Eye, EyeOff, GripVertical, Pencil, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { KIND_LABEL, type Card, type CardStats, type CardSummary } from '@/lib/api'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { CardBack } from '@/components/deck/CardBack'
import { CardStatBadge } from '@/components/deck/CardStatBadge'
import { useFlip } from '@/components/deck/useFlip'
import { plainTextPrompt } from '@/lib/format'
import { cn } from '@/lib/utils'

type CardRowProps = {
  card: CardSummary
  cardStats: CardStats | null | undefined
  loadCard: (id: number, signal: AbortSignal) => Promise<Card>
  onEdit: () => void
  onDelete: () => void
}

export function CardRow({
  card,
  cardStats,
  loadCard,
  onEdit,
  onDelete,
}: CardRowProps) {
  const [fullCard, setFullCard] = useState<Card | null>(null)
  const [loading, setLoading] = useState(false)
  const [revealed, setRevealed] = useState(false)
  const inFlight = useRef<AbortController | null>(null)

  const { face, flip, toFront, rotatorStyle, perspectiveStyle } = useFlip()

  useEffect(() => {
    if (face !== 'back') {
      inFlight.current?.abort()
      inFlight.current = null
      setLoading(false)
      return
    }
    if (fullCard || inFlight.current) return

    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    loadCard(card.id, controller.signal)
      .then(setFullCard)
      .catch((error: unknown) => {
        if ((error as Error)?.name === 'AbortError') return
        toast.error('Could not load the answer')
        toFront()
      })
      .finally(() => {
        if (inFlight.current === controller) {
          inFlight.current = null
          setLoading(false)
        }
      })
  }, [card.id, face, fullCard, loadCard, toFront])

  useEffect(() => () => inFlight.current?.abort(), [])

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

  const promptLabel = plainTextPrompt(card.prompt_md)

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
        aria-label={`Reorder ${promptLabel}`}
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
          <div className="flex items-center gap-2">
            <Badge>{KIND_LABEL[card.kind]}</Badge>
            {card.archived && <Badge variant="secondary">Archived</Badge>}
            {!card.archived && cardStats !== undefined && (
              <CardStatBadge stats={cardStats} />
            )}

            <Button
              variant="brand"
              size="icon-sm"
              className="rounded-full"
              aria-label={`Edit card ${card.id}`}
              title="Edit card"
              onClick={onEdit}
            >
              <Pencil />
            </Button>
            <Button
              variant="brand"
              size="icon-sm"
              className="rounded-full"
              aria-label={`Delete card ${card.id}`}
              title="Delete card"
              onClick={onDelete}
            >
              <Trash2 />
            </Button>

            {card.kind === 'mc_single' && showingAnswer && fullCard && (
              <Button
                variant="brand"
                size="icon-sm"
                className="ml-auto rounded-full"
                aria-pressed={revealed}
                aria-label={revealed ? 'Hide answer' : 'Reveal answer'}
                title={revealed ? 'Hide answer' : 'Reveal answer'}
                onClick={() => setRevealed((wasRevealed) => !wasRevealed)}
              >
                {revealed ? <EyeOff /> : <Eye />}
              </Button>
            )}
          </div>

          <div
            role="button"
            tabIndex={0}
            aria-label={`${showingAnswer ? 'Show question' : 'Show answer'}: ${promptLabel}`}
            onClick={(clickEvent) => {
              if ((clickEvent.target as HTMLElement).closest('a,button')) return
              flip()
            }}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) return
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                flip()
              }
            }}
            className="cursor-pointer rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {showingAnswer ? (
              loading || !fullCard ? (
                <div className="space-y-2" aria-busy="true">
                  <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
                  <div className="h-4 w-1/3 animate-pulse rounded bg-muted" />
                </div>
              ) : (
                <CardBack card={fullCard} revealed={revealed} />
              )
            ) : (
              <div className="flex items-start justify-between gap-3">
                <Markdown className="min-w-0 flex-1">{card.prompt_md}</Markdown>
                {card.image_path !== null && (
                  <div onClick={(event) => event.stopPropagation()}>
                    <CardImage path={card.image_path} altText={card.prompt_md} />
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
