import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, ArrowRight, Shuffle } from 'lucide-react'
import { toast } from 'sonner'
import { api, type Card, type CardSummary, type Deck } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { FlashcardAnswer } from '@/components/flashcards/FlashcardAnswer'
import { useFlip } from '@/components/deck/useFlip'
import { useSlide } from '@/components/flashcards/useSlide'
import { plainTextPrompt } from '@/lib/format'

function shuffled(order: number[]): number[] {
  const reordered = [...order]
  for (let shuffleIndex = reordered.length - 1; shuffleIndex > 0; shuffleIndex -= 1) {
    const swapIndex = Math.floor(Math.random() * (shuffleIndex + 1))
    const held = reordered[shuffleIndex]
    reordered[shuffleIndex] = reordered[swapIndex]
    reordered[swapIndex] = held
  }
  return reordered
}

function needsFullCard(card: CardSummary): boolean {
  return card.kind === 'mc_single' || card.kind === 'text_answer'
}

export function FlashcardsPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()

  const deckId = id !== undefined && /^\d+$/.test(id) ? Number(id) : null

  const [deck, setDeck] = useState<Deck | null>(null)
  const [notFound, setNotFound] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const [cards, setCards] = useState<CardSummary[]>([])
  const [order, setOrder] = useState<number[]>([])
  const [position, setPosition] = useState(0)
  const [fullCardsById, setFullCardsById] = useState<Map<number, Card>>(new Map())

  const { face, flip, resetToFront, rotatorStyle, perspectiveStyle } = useFlip()
  const { slide, sliderStyle } = useSlide()

  const container = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (deckId === null) {
      setNotFound(true)
      setLoaded(true)
      return
    }

    const controller = new AbortController()
    setNotFound(false)
    setLoaded(false)

    Promise.all([
      api.getDeck(deckId, controller.signal),
      api.listCards({ deckId, archived: 'false' }, controller.signal),
    ])
      .then(([fetchedDeck, fetchedCards]) => {
        setDeck(fetchedDeck)
        setCards(fetchedCards)
        setOrder(fetchedCards.map((_, cardIndex) => cardIndex))
        setPosition(0)
        setFullCardsById(new Map())
      })
      .catch((error: unknown) => {
        if ((error as Error)?.name === 'AbortError') return
        setNotFound(true)
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoaded(true)
      })

    return () => controller.abort()
  }, [deckId])

  useEffect(() => {
    if (order.length === 0) return

    const wantedIndexes = [order[position], order[(position + 1) % order.length]]
    const wantedCards = wantedIndexes
      .map((cardIndex) => cards[cardIndex])
      .filter(
        (card): card is CardSummary =>
          card !== undefined && needsFullCard(card) && !fullCardsById.has(card.id),
      )

    const uniqueCards = wantedCards.filter(
      (card, cardIndex) => wantedCards.findIndex((other) => other.id === card.id) === cardIndex,
    )
    if (uniqueCards.length === 0) return

    const controller = new AbortController()

    Promise.all(uniqueCards.map((card) => api.getCard(card.id, controller.signal)))
      .then((fetchedCards) => {
        if (controller.signal.aborted) return
        setFullCardsById((previous) => {
          const merged = new Map(previous)
          fetchedCards.forEach((fetchedCard) => merged.set(fetchedCard.id, fetchedCard))
          return merged
        })
      })
      .catch((error: unknown) => {
        if ((error as Error)?.name === 'AbortError') return
        toast.error('Could not load the answer')
      })

    return () => controller.abort()
  }, [cards, fullCardsById, order, position])

  useEffect(() => {
    if (loaded && !notFound) container.current?.focus()
  }, [loaded, notFound])

  const goToNext = useCallback(() => {
    if (order.length === 0) return
    slide('next', () => {
      resetToFront()
      setPosition((current) => (current + 1) % order.length)
    })
  }, [order.length, resetToFront, slide])

  const goToPrevious = useCallback(() => {
    if (order.length === 0) return
    slide('previous', () => {
      resetToFront()
      setPosition((current) => (current - 1 + order.length) % order.length)
    })
  }, [order.length, resetToFront, slide])

  function shuffleCards() {
    if (order.length === 0) return
    slide('next', () => {
      resetToFront()
      setOrder((current) => shuffled(current))
      setPosition(0)
    })
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'ArrowRight') {
      event.preventDefault()
      goToNext()
      return
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      goToPrevious()
      return
    }
    if (event.key === ' ' || event.key === 'Enter') {
      event.preventDefault()
      flip()
    }
  }

  if (!loaded) return null

  if (notFound || deck === null) {
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

  const header = (
    <div className="flex min-w-0 items-start gap-3">
      <Button
        variant="ghost"
        size="icon"
        className="mt-0.5 shrink-0"
        aria-label="Back to deck"
        title="Back to deck"
        onClick={() => navigate(`/decks/${deck.id}`)}
      >
        <ArrowLeft />
      </Button>
      <h1 className="font-display min-w-0 text-2xl font-bold">{deck.name}</h1>
    </div>
  )

  if (order.length === 0) {
    return (
      <div className="space-y-4">
        {header}
        <p className="text-muted-foreground sm:pl-11">
          No cards yet.{' '}
          <Link to={`/decks/${deck.id}`} className="underline">
            Back to the deck
          </Link>
        </p>
      </div>
    )
  }

  const card = cards[order[position]]
  const fullCard = fullCardsById.get(card.id) ?? null
  const showingAnswer = face === 'back'
  const promptLabel = plainTextPrompt(card.prompt_md)
  const answerReady = !needsFullCard(card) || fullCard !== null
  const viewedFraction = (position + 1) / order.length

  return (
    <div
      ref={container}
      tabIndex={-1}
      onKeyDown={handleKeyDown}
      className="space-y-5 focus:outline-none"
    >
      {header}

      <div style={perspectiveStyle} className="sm:pl-11">
        <div style={sliderStyle}>
          <div
            role="button"
            tabIndex={0}
            aria-label={`${showingAnswer ? 'Show question' : 'Show answer'}: ${promptLabel}`}
            style={rotatorStyle}
            onClick={(clickEvent) => {
              if ((clickEvent.target as HTMLElement).closest('a,button')) return
              flip()
            }}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) return
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                event.stopPropagation()
                flip()
              }
            }}
            className="flex min-h-[60vh] cursor-pointer flex-col items-center justify-center gap-4 rounded-2xl border bg-card p-8 shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {showingAnswer ? (
              answerReady ? (
                fullCard !== null ? (
                  <FlashcardAnswer card={fullCard} />
                ) : (
                  <Markdown className="text-center text-2xl">{card.answer_md ?? '—'}</Markdown>
                )
              ) : (
                <div className="h-6 w-48 animate-pulse rounded bg-muted" aria-busy="true" />
              )
            ) : (
              <>
                <Markdown className="text-center text-2xl">{card.prompt_md}</Markdown>
                {card.image_path !== null && (
                  <div onClick={(event) => event.stopPropagation()}>
                    <CardImage path={card.image_path} altText={promptLabel} />
                  </div>
                )}
              </>
            )}
        </div>
        </div>
      </div>

      <div className="flex items-center justify-center gap-4 sm:pl-11">
        <Button
          variant="secondary"
          size="icon-lg"
          className="rounded-full"
          aria-label="Previous card"
          title="Previous card"
          onClick={goToPrevious}
        >
          <ArrowLeft />
        </Button>
        <span className="min-w-16 text-center text-sm text-muted-foreground">
          {position + 1} / {order.length}
        </span>
        <Button
          variant="brand"
          size="icon-lg"
          className="rounded-full"
          aria-label="Next card"
          title="Next card"
          onClick={goToNext}
        >
          <ArrowRight />
        </Button>
        <Button
          variant="secondary"
          size="icon-lg"
          className="ml-4 rounded-full"
          aria-label="Shuffle cards"
          title="Shuffle cards"
          onClick={shuffleCards}
        >
          <Shuffle />
        </Button>
      </div>

      <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary sm:ml-11 sm:w-[calc(100%-2.75rem)]">
        <div
          className="h-full rounded-full bg-brand transition-[width]"
          style={{ width: `${viewedFraction * 100}%` }}
        />
      </div>
    </div>
  )
}
