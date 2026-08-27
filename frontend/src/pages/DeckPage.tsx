import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, Plus } from 'lucide-react'
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core'
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { toast } from 'sonner'
import { api, type CardSummary, type Deck } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { CardRow } from '@/components/deck/CardRow'

export function DeckPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()

  const deckId = id !== undefined && /^\d+$/.test(id) ? Number(id) : null

  const [deck, setDeck] = useState<Deck | null>(null)
  const [cards, setCards] = useState<CardSummary[]>([])
  const [showArchived, setShowArchived] = useState(false)
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)

  const loadDeck = useCallback(
    async (signal: AbortSignal) => {
      if (deckId === null) return
      try {
        const fetchedDeck = await api.getDeck(deckId, signal)
        setDeck(fetchedDeck)
        setNotFound(false)
      } catch (error) {
        if ((error as Error)?.name === 'AbortError') return
        setNotFound(true)
      }
    },
    [deckId],
  )

  const inFlight = useRef<AbortController | null>(null)
  const deckInFlight = useRef<AbortController | null>(null)

  const loadCards = useCallback(async () => {
    if (deckId === null) return
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    try {
      const rows = await api.listCards(
        { deckId, archived: showArchived ? 'all' : 'false' },
        controller.signal,
      )
      if (inFlight.current !== controller) return
      setCards(rows)
    } catch (error) {
      if ((error as Error)?.name === 'AbortError') return
      toast.error('Could not load cards')
    } finally {
      if (inFlight.current === controller) {
        inFlight.current = null
        setLoading(false)
      }
    }
  }, [deckId, showArchived])

  useEffect(() => {
    if (deckId === null) {
      setNotFound(true)
      setDeck(null)
      return
    }
    setNotFound(false)
    setDeck(null)
    const controller = new AbortController()
    void loadDeck(controller.signal)
    return () => controller.abort()
  }, [deckId, loadDeck])

  useEffect(() => { void loadCards() }, [loadCards])

  useEffect(() => () => {
    inFlight.current?.abort()
    deckInFlight.current?.abort()
  }, [])

  function reloadDeck() {
    deckInFlight.current?.abort()
    const controller = new AbortController()
    deckInFlight.current = controller
    void loadDeck(controller.signal)
  }

  async function archive(card: CardSummary) {
    try {
      await api.archiveCard(card.id)
      void loadCards()
      reloadDeck()
    } catch {
      toast.error('Could not archive card')
    }
  }

  async function unarchive(card: CardSummary) {
    try {
      await api.unarchiveCard(card.id)
      void loadCards()
      reloadDeck()
    } catch {
      toast.error('Could not unarchive card')
    }
  }

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event
    if (!over || active.id === over.id) return

    const fromIndex = cards.findIndex((card) => card.id === active.id)
    const toIndex = cards.findIndex((card) => card.id === over.id)
    if (fromIndex === -1 || toIndex === -1) return

    const droppedFetch = inFlight.current !== null
    inFlight.current?.abort()
    inFlight.current = null
    setLoading(false)

    const previousCards = cards
    const reorderedCards = arrayMove(cards, fromIndex, toIndex)
    setCards(reorderedCards)

    const landedIndex = reorderedCards.findIndex((card) => card.id === active.id)
    const before =
      landedIndex + 1 < reorderedCards.length
        ? reorderedCards[landedIndex + 1].id
        : null

    void api.moveCard(Number(active.id), before)
      .then(() => {
        if (droppedFetch) void loadCards()
      })
      .catch(() => {
        toast.error('Could not reorder cards')
        setCards(previousCards)
        void loadCards()
      })
  }

  if (notFound) {
    return (
      <div className="space-y-2">
        <h1 className="font-display text-2xl font-bold">Deck not found</h1>
        <p className="text-muted-foreground">
          This deck may have been removed.{' '}
          <Link to="/decks" className="underline">
            Back to decks
          </Link>
        </p>
      </div>
    )
  }

  if (!deck) {
    return null
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <Button
            variant="ghost"
            size="icon"
            className="mt-0.5 shrink-0"
            aria-label="Back to decks"
            title="Back to decks"
            onClick={() => navigate('/decks')}
          >
            <ArrowLeft />
          </Button>
          <div className="min-w-0 space-y-1">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="font-display text-2xl font-bold">{deck.name}</h1>
              {deck.module_id !== null && (
                <Badge variant="secondary">{deck.module_name}</Badge>
              )}
            </div>
            {deck.description && (
              <p className="text-muted-foreground">{deck.description}</p>
            )}
            <p className="text-sm text-muted-foreground">
              {deck.card_count} card{deck.card_count === 1 ? '' : 's'}
            </p>
          </div>
        </div>
        <Button
          variant="brand"
          className="h-10 px-4"
          onClick={() => navigate(`/cards/new?deck_id=${deck.id}`)}
        >
          <Plus className="size-4" />
          Add card
        </Button>
      </div>

      <div className="flex items-center gap-2">
        <Switch
          id="show-archived"
          checked={showArchived}
          onCheckedChange={setShowArchived}
        />
        <Label htmlFor="show-archived">Show archived</Label>
      </div>

      {cards.length === 0 && !loading && (
        <div className="space-y-2">
          <p className="text-muted-foreground">No cards yet.</p>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => navigate(`/cards/new?deck_id=${deck.id}`)}
          >
            <Plus className="size-4" />
            Add card
          </Button>
        </div>
      )}

      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={cards.map((card) => card.id)}
          strategy={verticalListSortingStrategy}
        >
          <ul className="space-y-3">
            {cards.map((card) => (
              <CardRow
                key={card.id}
                card={card}
                loadCard={api.getCard}
                onEdit={() => navigate(`/cards/${card.id}/edit`)}
                onArchiveToggle={() =>
                  void (card.archived ? unarchive(card) : archive(card))
                }
              />
            ))}
          </ul>
        </SortableContext>
      </DndContext>
    </div>
  )
}
