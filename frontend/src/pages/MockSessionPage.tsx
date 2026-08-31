import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'

import { ChoiceList } from '@/components/session/ChoiceList'
import { MockResults } from '@/components/session/MockResults'
import { MockRunHeader } from '@/components/session/MockRunHeader'
import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import {
  ApiError,
  api,
  type MockNextResponse,
  type SessionResults,
  type SubmitAnswerInput,
} from '@/lib/api'

function elapsedSince(startedAt: number): number {
  return Math.max(0, Date.now() - startedAt)
}

export function MockSessionPage() {
  const { id } = useParams<{ id: string }>()
  const sessionId = id !== undefined && /^\d+$/.test(id) ? Number(id) : null
  const navigate = useNavigate()

  const [served, setServed] = useState<MockNextResponse | null>(null)
  const [results, setResults] = useState<SessionResults | null>(null)
  const [selectedChoiceId, setSelectedChoiceId] = useState<number | null>(null)
  const [typedAnswer, setTypedAnswer] = useState('')
  const [busy, setBusy] = useState(false)
  const [ending, setEnding] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const [overridingReviewId, setOverridingReviewId] = useState<number | null>(null)

  const submitting = useRef(false)
  const inFlight = useRef<AbortController | null>(null)
  const servedAt = useRef<number>(Date.now())
  const container = useRef<HTMLDivElement | null>(null)
  const answerInput = useRef<HTMLTextAreaElement | null>(null)

  const showResults = useCallback(async () => {
    if (sessionId === null) return
    try {
      await api.finishSession(sessionId)
      setResults(await api.sessionResults(sessionId))
      setServed(null)
    } catch {
      toast.error('Could not load your results')
    } finally {
      setLoaded(true)
    }
  }, [sessionId])

  const loadNext = useCallback(async () => {
    if (sessionId === null) return
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    try {
      const response = await api.nextCard(sessionId, controller.signal)
      if (inFlight.current !== controller) return
      if (response.mode !== 'mock') {
        navigate(`/session/${sessionId}`, { replace: true })
        return
      }
      setServed(response)
      setSelectedChoiceId(null)
      setTypedAnswer('')
      servedAt.current = Date.now()
      setLoaded(true)
    } catch (error: unknown) {
      if ((error as Error)?.name === 'AbortError') return
      if (error instanceof ApiError) {
        await showResults()
      } else {
        toast.error('Could not reach the server')
        setLoaded(true)
      }
    } finally {
      if (inFlight.current === controller) {
        inFlight.current = null
      }
    }
  }, [sessionId, navigate, showResults])

  useEffect(() => {
    void loadNext()
  }, [loadNext])

  useEffect(() => () => inFlight.current?.abort(), [])

  const card = served?.card ?? null
  const totalQuestions = served?.target_count ?? served?.pool_count ?? 0

  useEffect(() => {
    if (results !== null || !loaded) return
    if (card === null) return
    if (card.kind === 'mc_single') {
      container.current?.focus()
    } else {
      answerInput.current?.focus()
    }
  }, [card, results, loaded])

  async function send(input: SubmitAnswerInput) {
    if (sessionId === null || submitting.current) return
    submitting.current = true
    setBusy(true)
    try {
      await api.recordAnswer(sessionId, input)
      await loadNext()
    } catch (error: unknown) {
      if (error instanceof ApiError) {
        if ('card_id' in error.byField()) {
          toast.error('That card was removed. Moving on.')
          await loadNext()
        } else {
          const messages = Object.values(error.byField())
          toast.error(messages[0] ?? error.message)
        }
      } else {
        toast.error('Could not record your answer')
      }
    } finally {
      submitting.current = false
      setBusy(false)
    }
  }

  function submit() {
    if (card === null || busy) return
    const milliseconds = elapsedSince(servedAt.current)
    if (card.kind === 'mc_single') {
      if (selectedChoiceId === null) return
      void send({ card_id: card.id, choice_id: selectedChoiceId, ms: milliseconds })
      return
    }
    if (typedAnswer.trim() === '') return
    void send({ card_id: card.id, given: typedAnswer, ms: milliseconds })
  }

  async function endEarly() {
    setEnding(true)
    inFlight.current?.abort()
    await showResults()
    setEnding(false)
  }

  async function override(reviewId: number) {
    if (sessionId === null) return
    setOverridingReviewId(reviewId)
    try {
      await api.overrideReview(reviewId)
      setResults(await api.sessionResults(sessionId))
    } catch {
      toast.error('Could not count that answer')
    } finally {
      setOverridingReviewId(null)
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (busy || results !== null || card === null) return

    const target = event.target as HTMLElement

    if (event.key === 'Enter') {
      if (event.shiftKey && target.tagName === 'TEXTAREA') return
      event.preventDefault()
      submit()
      return
    }

    if (card.kind !== 'mc_single') return

    const digit = Number(event.key)
    if (!Number.isInteger(digit) || digit < 1 || digit > 9) return
    const choice = card.choices[digit - 1]
    if (choice) {
      event.preventDefault()
      setSelectedChoiceId(choice.id)
    }
  }

  if (sessionId === null) {
    return (
      <div className="space-y-4">
        <h1 className="font-display text-2xl font-bold">Session not found</h1>
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    )
  }

  if (results !== null) {
    return (
      <MockResults
        results={results}
        onOverride={(reviewId) => void override(reviewId)}
        overridingReviewId={overridingReviewId}
      />
    )
  }

  if (!loaded) return null

  if (card === null || served === null) {
    return (
      <div className="space-y-4">
        <h1 className="font-display text-2xl font-bold">This mock test is unavailable</h1>
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    )
  }

  return (
    <div
      ref={container}
      tabIndex={-1}
      onKeyDown={handleKeyDown}
      className="max-w-2xl space-y-5 focus:outline-none"
    >
      <MockRunHeader
        questionNumber={Math.min(served.answered_count + 1, totalQuestions)}
        totalQuestions={totalQuestions}
        startedAt={served.started_at}
        onEndEarly={() => void endEarly()}
        ending={ending}
      />

      <div className="space-y-3 rounded-xl border bg-card p-4 shadow-sm">
        <Markdown className="text-lg">{card.prompt_md}</Markdown>
        {card.image_path !== null && <CardImage path={card.image_path} altText="Card image" />}
      </div>

      {card.kind === 'mc_single' && (
        <ChoiceList
          choices={card.choices}
          selectedChoiceId={selectedChoiceId}
          onSelect={setSelectedChoiceId}
          disabled={busy}
        />
      )}

      {card.kind !== 'mc_single' && (
        <Textarea
          ref={answerInput}
          value={typedAnswer}
          onChange={(event) => setTypedAnswer(event.target.value)}
          placeholder="Type your answer"
          aria-label="Your answer"
        />
      )}

      <div className="flex items-center gap-3">
        <Button variant="brand" className="h-10 px-6" onClick={submit} disabled={busy}>
          Submit
        </Button>
        <span className="text-sm text-muted-foreground">
          {card.kind === 'mc_single'
            ? '1–9 to choose · Enter to submit'
            : 'Enter to submit · Shift+Enter for a new line'}
        </span>
      </div>
    </div>
  )
}
