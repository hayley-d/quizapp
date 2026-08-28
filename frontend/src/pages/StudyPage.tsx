import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'

import { api, ApiError, type Deck, type SessionMode } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

type ModeOption = {
  value: SessionMode
  label: string
  note: string
  available: boolean
}

const MODE_OPTIONS: ModeOption[] = [
  {
    value: 'practice',
    label: 'Practice',
    note: 'Weighted towards what you keep getting wrong. No end — stop when you like.',
    available: true,
  },
  {
    value: 'mock',
    label: 'Mock test',
    note: 'Arrives in part 5.',
    available: false,
  },
  {
    value: 'sm2',
    label: 'Spaced repetition',
    note: 'Arrives in part 7.',
    available: false,
  },
]

type DeckGroup = {
  moduleName: string
  decks: Deck[]
}

function groupByModule(decks: Deck[]): DeckGroup[] {
  const groups: DeckGroup[] = []
  for (const deck of decks) {
    const moduleName = deck.module_name ?? 'No module'
    const existing = groups.find((group) => group.moduleName === moduleName)
    if (existing) {
      existing.decks.push(deck)
    } else {
      groups.push({ moduleName, decks: [deck] })
    }
  }
  return groups
}

export function StudyPage() {
  const navigate = useNavigate()
  const [decks, setDecks] = useState<Deck[]>([])
  const [selectedDeckIds, setSelectedDeckIds] = useState<number[]>([])
  const [loading, setLoading] = useState(true)
  const [starting, setStarting] = useState(false)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const inFlight = useRef<AbortController | null>(null)

  const loadDecks = useCallback(async () => {
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    try {
      const rows = await api.listDecks({}, controller.signal)
      if (inFlight.current !== controller) return
      setDecks(rows)
    } catch (error: unknown) {
      if ((error as Error)?.name === 'AbortError') return
      toast.error('Could not load decks')
    } finally {
      if (inFlight.current === controller) {
        inFlight.current = null
        setLoading(false)
      }
    }
  }, [])

  useEffect(() => {
    void loadDecks()
  }, [loadDecks])

  useEffect(() => () => inFlight.current?.abort(), [])

  const selectedDecks = decks.filter((deck) => selectedDeckIds.includes(deck.id))
  const selectedCardCount = selectedDecks.reduce((total, deck) => total + deck.card_count, 0)
  const canStart = selectedCardCount > 0 && !starting

  function toggleDeck(deckId: number, checked: boolean) {
    setErrors({})
    setSelectedDeckIds((current) =>
      checked ? [...current, deckId] : current.filter((id) => id !== deckId),
    )
  }

  async function start() {
    setStarting(true)
    setErrors({})
    try {
      const session = await api.createSession({
        mode: 'practice',
        deck_ids: selectedDeckIds,
      })
      navigate(`/session/${session.id}`)
    } catch (error: unknown) {
      if (error instanceof ApiError) {
        const errorsByField = error.byField()
        setErrors(errorsByField)
        if (Object.keys(errorsByField).length === 0) toast.error(error.message)
      } else {
        toast.error('Could not reach the server')
      }
      setStarting(false)
    }
  }

  if (loading) return null

  const groups = groupByModule(decks)

  return (
    <div className="max-w-2xl space-y-8">
      <div>
        <h1 className="font-display text-2xl font-bold">Study</h1>
        <p className="mt-2 text-muted-foreground">
          Pick what to revise, then start. Everything is graded as you go.
        </p>
      </div>

      <section className="space-y-3">
        <h2 className="font-display text-lg font-semibold">Mode</h2>
        <RadioGroup value="practice" className="space-y-2 rounded-xl border bg-card p-4 shadow-sm">
          {MODE_OPTIONS.map((option) => (
            <div key={option.value} className="flex items-start gap-3">
              <RadioGroupItem
                value={option.value}
                id={`mode-${option.value}`}
                disabled={!option.available}
                className="mt-1"
              />
              <div className="grid gap-0.5">
                <Label
                  htmlFor={`mode-${option.value}`}
                  className={option.available ? '' : 'text-muted-foreground'}
                >
                  {option.label}
                </Label>
                <span className="text-sm text-muted-foreground">{option.note}</span>
              </div>
            </div>
          ))}
        </RadioGroup>
      </section>

      <section className="space-y-3">
        <h2 className="font-display text-lg font-semibold">Decks</h2>

        {decks.length === 0 ? (
          <p className="text-muted-foreground">
            No decks yet. Make one on the{' '}
            <Link className="underline" to="/decks">
              decks screen
            </Link>{' '}
            first.
          </p>
        ) : (
          <div className="space-y-5">
            {groups.map((group) => (
              <div key={group.moduleName} className="space-y-2">
                <h3 className="text-sm font-semibold text-muted-foreground">
                  {group.moduleName}
                </h3>
                <ul className="space-y-2 rounded-xl border bg-card p-4 shadow-sm">
                  {group.decks.map((deck) => (
                    <li key={deck.id} className="flex items-start gap-3">
                      <Checkbox
                        id={`deck-${deck.id}`}
                        checked={selectedDeckIds.includes(deck.id)}
                        onCheckedChange={(checked) => toggleDeck(deck.id, checked === true)}
                        disabled={deck.card_count === 0}
                        className="mt-1"
                      />
                      <div className="grid gap-0.5">
                        <Label
                          htmlFor={`deck-${deck.id}`}
                          className={deck.card_count === 0 ? 'text-muted-foreground' : ''}
                        >
                          {deck.name}
                        </Label>
                        <span className="text-sm text-muted-foreground">
                          {deck.card_count === 0
                            ? 'No cards yet'
                            : `${deck.card_count} card${deck.card_count === 1 ? '' : 's'}`}
                        </span>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}

        {errors.deck_ids && <p className="text-sm text-destructive">{errors.deck_ids}</p>}
        {errors.mode && <p className="text-sm text-destructive">{errors.mode}</p>}
      </section>

      <div className="flex items-center gap-4">
        <Button
          variant="brand"
          className="h-10 px-6"
          disabled={!canStart}
          onClick={() => void start()}
        >
          Start practising
        </Button>
        <span className="text-sm text-muted-foreground">
          {selectedCardCount === 0
            ? 'Select a deck to begin'
            : `${selectedCardCount} card${selectedCardCount === 1 ? '' : 's'} selected`}
        </span>
      </div>
    </div>
  )
}
