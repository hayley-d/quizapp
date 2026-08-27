import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { ArchiveRestore, Archive as ArchiveIcon, Pencil, Plus } from 'lucide-react'
import { toast } from 'sonner'
import { api, KIND_LABEL, type CardSummary, type Deck } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Markdown } from '@/components/Markdown'
import { CardImage } from '@/components/CardImage'

export function DeckPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()

  // A non-numeric id is a not-found state, not a fetch attempt.
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
        const d = await api.getDeck(deckId, signal)
        setDeck(d)
        setNotFound(false)
      } catch (e) {
        if ((e as Error)?.name === 'AbortError') return
        setNotFound(true)
      }
    },
    [deckId],
  )

  // One in-flight cards request at a time, mirroring DecksPage's stale-response guard.
  // Deck refetches (triggered by archive/unarchive, below) get their own
  // controller ref: sharing this one would let a deck refetch abort an
  // in-flight cards request (or vice versa) when both fire together.
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
      setCards(rows)
    } catch (e) {
      if ((e as Error)?.name === 'AbortError') return
      toast.error('Could not load cards')
    } finally {
      if (inFlight.current === controller) setLoading(false)
    }
  }, [deckId, showArchived])

  useEffect(() => {
    if (deckId === null) {
      setNotFound(true)
      setDeck(null)
      return
    }
    // Reset stale state from a previous id (e.g. a prior not-found, or the
    // previous deck's header) before the new fetch resolves.
    setNotFound(false)
    setDeck(null)
    const controller = new AbortController()
    void loadDeck(controller.signal)
    return () => controller.abort()
  }, [deckId, loadDeck])

  useEffect(() => { void loadCards() }, [loadCards])

  // Cancel any in-flight cards/deck requests if the page unmounts.
  useEffect(() => () => {
    inFlight.current?.abort()
    deckInFlight.current?.abort()
  }, [])

  // Archive/unarchive can change the deck's card_count (it deliberately
  // excludes archived cards), so the header must refetch alongside the list —
  // otherwise it shows a stale count until a full navigation.
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
        <div className="space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="font-display text-2xl font-bold">{deck.name}</h1>
            {deck.module_id !== null && <Badge variant="secondary">{deck.module_name}</Badge>}
          </div>
          {deck.description && <p className="text-muted-foreground">{deck.description}</p>}
          <p className="text-sm text-muted-foreground">
            {deck.card_count} card{deck.card_count === 1 ? '' : 's'}
          </p>
        </div>
        <Button className="h-10 px-4" onClick={() => navigate(`/cards/new?deck_id=${deck.id}`)}>
          <Plus className="size-4" />
          New card
        </Button>
      </div>

      <div className="flex items-center gap-2">
        <Switch id="show-archived" checked={showArchived} onCheckedChange={setShowArchived} />
        <Label htmlFor="show-archived">Show archived</Label>
      </div>

      {cards.length === 0 && !loading && (
        <div className="space-y-2">
          <p className="text-muted-foreground">No cards yet.</p>
          <Button variant="secondary" size="sm" onClick={() => navigate(`/cards/new?deck_id=${deck.id}`)}>
            <Plus className="size-4" />
            New card
          </Button>
        </div>
      )}

      <ul className="space-y-2">
        {cards.map((c) => (
          <li
            key={c.id}
            className={`flex items-start justify-between gap-3 rounded-lg border p-3 ${
              c.archived ? 'opacity-60' : ''
            }`}
          >
            <div className="flex min-w-0 flex-1 items-start gap-3">
              <div className="flex shrink-0 flex-wrap items-center gap-2">
                <Badge variant="outline">{KIND_LABEL[c.kind]}</Badge>
                {c.archived && <Badge variant="secondary">Archived</Badge>}
              </div>
              {c.image_path !== null && (
                <CardImage path={c.image_path} alt={c.prompt_md} />
              )}
              {/* Unclamped on purpose: a truncated single line cannot render
                  markdown without a half-open `$…$` or a stray list marker
                  looking broken. The cost is a long page for a 100+ card deck,
                  which is a deliberate, recorded trade-off. */}
              <Markdown className="min-w-0 flex-1">{c.prompt_md}</Markdown>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Edit card ${c.id}`}
                title="Edit card"
                onClick={() => navigate(`/cards/${c.id}/edit`)}
              >
                <Pencil className="size-4" />
              </Button>
              {c.archived ? (
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Unarchive card ${c.id}`}
                  title="Unarchive card"
                  onClick={() => void unarchive(c)}
                >
                  <ArchiveRestore className="size-4" />
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Archive card ${c.id}`}
                  title="Archive card"
                  onClick={() => void archive(c)}
                >
                  <ArchiveIcon className="size-4" />
                </Button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  )
}
