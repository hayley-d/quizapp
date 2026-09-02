import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, Download, FileText, Layers, Plus, Repeat, Zap } from 'lucide-react'
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
import type { LucideIcon } from 'lucide-react'
import {
  api,
  ApiError,
  deckExportUrl,
  type CardSummary,
  type Deck,
  type DeckStats,
  type SessionMode,
} from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog'
import { CardRow } from '@/components/deck/CardRow'
import { DeckStatsStrip } from '@/components/deck/DeckStatsStrip'
import { MasteryLadderBar } from '@/components/deck/MasteryLadderBar'
import { plainTextPrompt } from '@/lib/format'

type StudyLaunch =
  | { kind: 'session'; mode: SessionMode }
  | { kind: 'navigate'; to: (deckId: number) => string }

type TestTypeOption = {
  key: string
  launch: StudyLaunch
  label: string
  note: string
  available: boolean
  Icon: LucideIcon
}

const TEST_TYPE_OPTIONS: TestTypeOption[] = [
  {
    key: 'practice',
    launch: { kind: 'session', mode: 'practice' },
    label: 'Practice',
    note: 'A card you get wrong comes straight back until you get it right. No end — stop when you like.',
    available: true,
    Icon: Zap,
  },
  {
    key: 'mock',
    launch: { kind: 'session', mode: 'mock' },
    label: 'Mock test',
    note: 'Every card in the deck, once, in a fixed order. No feedback until the end.',
    available: true,
    Icon: FileText,
  },
  {
    key: 'sm2',
    launch: { kind: 'session', mode: 'sm2' },
    label: 'Spaced repetition',
    note: 'Only the cards the scheduler says are due today.',
    available: true,
    Icon: Repeat,
  },
  {
    key: 'flashcards',
    launch: { kind: 'navigate', to: (deckId) => `/flashcards/${deckId}` },
    label: 'Flashcards',
    note: 'Flip through every card in the deck. Nothing is scored.',
    available: true,
    Icon: Layers,
  },
]

const MASTERY_GOAL_CHOICES: (number | null)[] = [null, 3, 5, 10]

const SESSION_ROUTE_BY_MODE: Record<SessionMode, string> = {
  practice: '/session',
  mock: '/mock',
  sm2: '/session',
}

export function DeckPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()

  const deckId = id !== undefined && /^\d+$/.test(id) ? Number(id) : null

  const [deck, setDeck] = useState<Deck | null>(null)
  const [cards, setCards] = useState<CardSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [notFound, setNotFound] = useState(false)
  const [startingMode, setStartingMode] = useState<SessionMode | null>(null)
  const [masteryGoal, setMasteryGoal] = useState<number | null>(null)
  const [deckStats, setDeckStats] = useState<DeckStats | null>(null)
  const [cardPendingDeletion, setCardPendingDeletion] = useState<CardSummary | null>(null)
  const [deletingCard, setDeletingCard] = useState(false)

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
  const statsInFlight = useRef<AbortController | null>(null)

  const loadDeckStats = useCallback(
    async (signal: AbortSignal) => {
      if (deckId === null) return
      try {
        setDeckStats(await api.getDeckStats(deckId, signal))
      } catch (error) {
        if ((error as Error)?.name === 'AbortError') return
        setDeckStats(null)
      }
    },
    [deckId],
  )

  const loadCards = useCallback(async () => {
    if (deckId === null) return
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    try {
      const rows = await api.listCards(
        { deckId, archived: 'false' },
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
  }, [deckId])

  useEffect(() => {
    if (deckId === null) {
      setNotFound(true)
      setDeck(null)
      return
    }
    setNotFound(false)
    setDeck(null)
    setDeckStats(null)
    const controller = new AbortController()
    void loadDeck(controller.signal)
    void loadDeckStats(controller.signal)
    return () => controller.abort()
  }, [deckId, loadDeck, loadDeckStats])

  useEffect(() => { void loadCards() }, [loadCards])

  useEffect(() => () => {
    inFlight.current?.abort()
    deckInFlight.current?.abort()
    statsInFlight.current?.abort()
  }, [])

  function reloadDeck() {
    deckInFlight.current?.abort()
    const controller = new AbortController()
    deckInFlight.current = controller
    void loadDeck(controller.signal)
  }

  function reloadDeckStats() {
    statsInFlight.current?.abort()
    const controller = new AbortController()
    statsInFlight.current = controller
    void loadDeckStats(controller.signal)
  }

  async function deleteCard() {
    const card = cardPendingDeletion
    if (card === null) return
    setDeletingCard(true)
    try {
      await api.deleteCard(card.id)
      setCardPendingDeletion(null)
      void loadCards()
      reloadDeck()
      reloadDeckStats()
    } catch {
      toast.error('Could not delete the card')
    } finally {
      setDeletingCard(false)
    }
  }

  function goalChoiceLabel(choice: number | null): string {
    return choice === null ? 'None' : String(choice)
  }

  async function startSession(mode: SessionMode) {
    if (deckId === null) return
    setStartingMode(mode)
    try {
      const session = await api.createSession({
        mode,
        deck_ids: [deckId],
        ...(mode !== 'mock' && masteryGoal !== null ? { mastery_goal: masteryGoal } : {}),
      })
      navigate(`${SESSION_ROUTE_BY_MODE[mode]}/${session.id}`)
    } catch (error: unknown) {
      if (error instanceof ApiError) {
        const messages = Object.values(error.byField())
        toast.error(messages[0] ?? error.message)
      } else {
        toast.error('Could not start a session')
      }
      setStartingMode(null)
    }
  }

  function launchStudyOption(option: TestTypeOption) {
    if (deckId === null) return
    if (option.launch.kind === 'navigate') {
      navigate(option.launch.to(deckId))
      return
    }
    void startSession(option.launch.mode)
  }

  function isStarting(option: TestTypeOption): boolean {
    return option.launch.kind === 'session' && startingMode === option.launch.mode
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

  const statsByCardId =
    deckStats === null
      ? null
      : new Map(deckStats.cards.map((cardStats) => [cardStats.card_id, cardStats]))

  function cardDeletionLines(card: CardSummary): string[] {
    const recordedAnswers = statsByCardId?.get(card.id)?.attempt_count ?? null
    const lines = [`“${plainTextPrompt(card.prompt_md)}” will be removed from this deck.`]
    if (recordedAnswers !== null && recordedAnswers > 0) {
      lines.push(
        `${recordedAnswers} recorded answer${recordedAnswers === 1 ? '' : 's'} will go with it.`,
      )
    }
    lines.push('This cannot be undone.')
    return lines
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

  const dueCount = deckStats?.summary.due_count ?? null
  const nextDueAt = deckStats?.summary.next_due_at ?? null

  function noteFor(option: TestTypeOption): string {
    if (option.key !== 'sm2') return option.note
    if (dueCount === null) return option.note
    if (dueCount > 0) return `${dueCount} card${dueCount === 1 ? '' : 's'} due now.`
    if (nextDueAt !== null) return `Nothing due — next on ${nextDueAt.slice(0, 10)}.`
    return 'Nothing due yet.'
  }

  function isDisabled(option: TestTypeOption): boolean {
    if (option.key === 'sm2' && (dueCount === null || dueCount === 0)) return true
    return !option.available || deck?.card_count === 0 || startingMode !== null
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
        <div className="flex gap-2">
          <Button asChild variant="outline" className="h-10 px-4">
            <a href={deckExportUrl(deck.id)}>
              <Download className="size-4" />
              Export
            </a>
          </Button>
          <Button
            variant="brand"
            className="h-10 px-4"
            onClick={() => navigate(`/cards/new?deck_id=${deck.id}`)}
          >
            <Plus className="size-4" />
            Add card
          </Button>
        </div>
      </div>

      <div className="space-y-6 sm:pl-11">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {TEST_TYPE_OPTIONS.map((option) => (
            <Button
              key={option.key}
              variant="secondary"
              className="h-16 w-full gap-3 rounded-2xl text-base [&_svg:not([class*='size-'])]:size-5"
              disabled={isDisabled(option)}
              title={
                option.available && deck.card_count === 0
                  ? 'Add a card to this deck first'
                  : noteFor(option)
              }
              onClick={() => launchStudyOption(option)}
            >
              <option.Icon className={option.available ? 'text-brand' : undefined} />
              {isStarting(option) ? 'Starting…' : option.label}
            </Button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm text-muted-foreground">Goal: move cards up</span>
          {MASTERY_GOAL_CHOICES.map((choice) => (
            <Button
              key={goalChoiceLabel(choice)}
              variant={masteryGoal === choice ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setMasteryGoal(choice)}
            >
              {goalChoiceLabel(choice)}
            </Button>
          ))}
          <span className="text-xs text-muted-foreground">
            for practice and spaced repetition
          </span>
        </div>

        {deckStats !== null && (
          <div className="space-y-3">
            <MasteryLadderBar counts={deckStats.summary.mastery_counts} />
            <DeckStatsStrip summary={deckStats.summary} />
          </div>
        )}

        {cards.length === 0 && !loading && (
          <p className="text-muted-foreground">No cards yet.</p>
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
            <ul className="space-y-3 sm:-ml-7">
              {cards.map((card) => (
                <CardRow
                  key={card.id}
                  card={card}
                  cardStats={
                    statsByCardId === null
                      ? undefined
                      : (statsByCardId.get(card.id) ?? null)
                  }
                  loadCard={api.getCard}
                  onEdit={() => navigate(`/cards/${card.id}/edit`)}
                  onDelete={() => setCardPendingDeletion(card)}
                />
              ))}
            </ul>
          </SortableContext>
        </DndContext>
      </div>

      {cardPendingDeletion !== null && (
        <ConfirmDeleteDialog
          open
          onOpenChange={(isOpen) => { if (!isOpen) setCardPendingDeletion(null) }}
          title="Delete this card?"
          lines={cardDeletionLines(cardPendingDeletion)}
          confirmLabel="Delete card"
          busy={deletingCard}
          onConfirm={() => void deleteCard()}
        />
      )}
    </div>
  )
}
