import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { Link, useParams } from 'react-router-dom'
import { toast } from 'sonner'

import { AnswerVerdict } from '@/components/session/AnswerVerdict'
import { ChoiceList } from '@/components/session/ChoiceList'
import { SessionExhausted } from '@/components/session/SessionExhausted'
import { SessionSummary as SessionSummaryScreen } from '@/components/session/SessionSummary'
import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  api,
  ApiError,
  type AnswerResult,
  type NextResponse,
  type RevealedAnswer,
  type SelfGrade,
  type SessionSummary,
  type SubmitAnswerInput,
} from '@/lib/api'

const SELF_GRADES: { grade: SelfGrade; label: string }[] = [
  { grade: 'again', label: 'Again' },
  { grade: 'hard', label: 'Hard' },
  { grade: 'good', label: 'Good' },
  { grade: 'easy', label: 'Easy' },
]

function elapsedSince(startedAt: number): number {
  return Math.max(0, Date.now() - startedAt)
}

export function SessionPage() {
  const { id } = useParams<{ id: string }>()
  const sessionId = id !== undefined && /^\d+$/.test(id) ? Number(id) : null

  const [served, setServed] = useState<NextResponse | null>(null)
  const [verdict, setVerdict] = useState<AnswerResult | null>(null)
  const [revealed, setRevealed] = useState<RevealedAnswer | null>(null)
  const [summary, setSummary] = useState<SessionSummary | null>(null)
  const [selectedChoiceId, setSelectedChoiceId] = useState<number | null>(null)
  const [typedAnswer, setTypedAnswer] = useState('')
  const [overridden, setOverridden] = useState(false)
  const [overriding, setOverriding] = useState(false)
  const [busy, setBusy] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const [exhausted, setExhausted] = useState<string | null>(null)

  const inFlight = useRef<AbortController | null>(null)
  const servedAt = useRef<number>(0)
  const container = useRef<HTMLDivElement>(null)
  const answerInput = useRef<HTMLInputElement>(null)
  const revealButton = useRef<HTMLButtonElement>(null)
  const nextButton = useRef<HTMLButtonElement>(null)

  const loadNext = useCallback(async () => {
    if (sessionId === null) return
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    try {
      const response = await api.nextCard(sessionId, controller.signal)
      if (inFlight.current !== controller) return
      setServed(response)
      setVerdict(null)
      setRevealed(null)
      setSelectedChoiceId(null)
      setTypedAnswer('')
      setOverridden(false)
      setExhausted(null)
      servedAt.current = Date.now()
    } catch (error: unknown) {
      if ((error as Error)?.name === 'AbortError') return
      if (error instanceof ApiError) {
        setExhausted(error.message)
      } else {
        toast.error('Could not reach the server')
      }
    } finally {
      if (inFlight.current === controller) {
        inFlight.current = null
        setLoaded(true)
      }
    }
  }, [sessionId])

  useEffect(() => {
    void loadNext()
  }, [loadNext])

  useEffect(() => () => inFlight.current?.abort(), [])

  const card = served?.card ?? null
  const graded = verdict !== null

  useEffect(() => {
    if (summary || !loaded) return
    if (graded) {
      nextButton.current?.focus()
      return
    }
    if (card?.kind === 'short_answer') {
      answerInput.current?.focus()
    } else if (card?.kind === 'flashcard' && !revealed) {
      revealButton.current?.focus()
    } else {
      container.current?.focus()
    }
  }, [graded, card?.id, card?.kind, revealed, summary, loaded])

  async function send(input: SubmitAnswerInput) {
    if (sessionId === null || busy) return
    setBusy(true)
    try {
      const result = await api.submitAnswer(sessionId, input)
      setVerdict(result)
      setServed((current) =>
        current === null
          ? current
          : {
              ...current,
              answered_count: current.answered_count + 1,
              correct_count: current.correct_count + (result.correct ? 1 : 0),
            },
      )
    } catch (error: unknown) {
      if (error instanceof ApiError) {
        toast.error(error.message)
      } else {
        toast.error('Could not reach the server')
      }
    } finally {
      setBusy(false)
    }
  }

  function submit() {
    if (!card || graded) return
    if (card.kind === 'mc_single') {
      if (selectedChoiceId === null) return
      void send({ card_id: card.id, choice_id: selectedChoiceId, ms: elapsedSince(servedAt.current) })
    } else if (card.kind === 'short_answer') {
      if (typedAnswer.trim() === '') return
      void send({ card_id: card.id, given: typedAnswer, ms: elapsedSince(servedAt.current) })
    }
  }

  function submitSelfGrade(grade: SelfGrade) {
    if (!card || graded) return
    void send({ card_id: card.id, self_grade: grade, ms: elapsedSince(servedAt.current) })
  }

  async function reveal() {
    if (sessionId === null || !card || revealed || busy) return
    setBusy(true)
    try {
      setRevealed(await api.revealCard(sessionId, card.id))
    } catch {
      toast.error('Could not load the answer')
    } finally {
      setBusy(false)
    }
  }

  async function override() {
    if (!verdict || overriding) return
    setOverriding(true)
    try {
      await api.overrideReview(verdict.review_id)
      setOverridden(true)
      setServed((current) =>
        current === null ? current : { ...current, correct_count: current.correct_count + 1 },
      )
      toast.success('Counted as correct, and accepted for next time')
    } catch (error: unknown) {
      if (error instanceof ApiError) {
        toast.error(error.message)
      } else {
        toast.error('Could not reach the server')
      }
    } finally {
      setOverriding(false)
    }
  }

  async function endSession() {
    if (sessionId === null) return
    inFlight.current?.abort()
    try {
      setSummary(await api.finishSession(sessionId))
    } catch {
      toast.error('Could not end the session')
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (summary || busy) return
    const target = event.target as HTMLElement
    const typing = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA'

    if (event.key === 'Enter') {
      event.preventDefault()
      if (graded) {
        void loadNext()
      } else if (card?.kind === 'flashcard' && !revealed) {
        void reveal()
      } else {
        submit()
      }
      return
    }

    if (typing) return

    if (card?.kind === 'flashcard' && !revealed && event.key === ' ') {
      event.preventDefault()
      void reveal()
      return
    }

    if (graded || !card) return

    const digit = Number(event.key)
    if (!Number.isInteger(digit) || digit < 1 || digit > 9) return

    if (card.kind === 'mc_single') {
      const choice = card.choices[digit - 1]
      if (choice) {
        event.preventDefault()
        setSelectedChoiceId(choice.id)
      }
    } else if (card.kind === 'flashcard' && revealed && digit <= SELF_GRADES.length) {
      event.preventDefault()
      submitSelfGrade(SELF_GRADES[digit - 1].grade)
    }
  }

  if (sessionId === null) {
    return (
      <div>
        <h1 className="font-display text-2xl font-bold">Session not found</h1>
        <Link className="mt-2 inline-block underline" to="/study">
          Back to study
        </Link>
      </div>
    )
  }

  if (!loaded) return null

  if (summary) return <SessionSummaryScreen summary={summary} />

  if (exhausted !== null || !card) {
    return <SessionExhausted message={exhausted ?? 'This session has no cards.'} />
  }

  const answeredCount = served?.answered_count ?? 0
  const correctCount = served?.correct_count ?? 0

  return (
    <div
      ref={container}
      tabIndex={-1}
      onKeyDown={handleKeyDown}
      className="max-w-2xl space-y-6 outline-none"
    >
      <header className="flex flex-wrap items-baseline justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          {answeredCount} answered · {correctCount} correct
          {answeredCount > 0 && ` · ${Math.round((correctCount / answeredCount) * 100)}%`}
          {' · '}
          {served?.pool_count ?? 0} in the pool
        </p>
        <Button variant="ghost" size="sm" onClick={() => void endSession()}>
          End session
        </Button>
      </header>

      <div className="space-y-4">
        <Markdown className="text-lg">{card.prompt_md}</Markdown>
        {card.image_path && <CardImage path={card.image_path} altText="Card image" />}
      </div>

      {card.kind === 'mc_single' && (
        <ChoiceList
          choices={card.choices}
          selectedChoiceId={selectedChoiceId}
          onSelect={setSelectedChoiceId}
          disabled={graded}
        />
      )}

      {card.kind === 'short_answer' && !graded && (
        <Input
          ref={answerInput}
          value={typedAnswer}
          onChange={(event) => setTypedAnswer(event.target.value)}
          placeholder="Type your answer"
          autoComplete="off"
          aria-label="Your answer"
        />
      )}

      {card.kind === 'flashcard' && (
        <div className="space-y-4">
          {revealed ? (
            <Markdown className="rounded-lg bg-muted px-4 py-3">
              {revealed.answer_md ?? ''}
            </Markdown>
          ) : (
            <Button ref={revealButton} variant="secondary" onClick={() => void reveal()}>
              Show answer
            </Button>
          )}
          {revealed && !graded && (
            <div className="flex flex-wrap gap-2">
              {SELF_GRADES.map((option, optionIndex) => (
                <Button
                  key={option.grade}
                  variant="secondary"
                  onClick={() => submitSelfGrade(option.grade)}
                >
                  {option.label}
                  <span className="ml-1.5 text-xs opacity-60">{optionIndex + 1}</span>
                </Button>
              ))}
            </div>
          )}
        </div>
      )}

      {graded ? (
        <AnswerVerdict
          verdict={verdict}
          overridden={overridden}
          overriding={overriding}
          onOverride={() => void override()}
          onNext={() => void loadNext()}
          nextButtonRef={nextButton}
        />
      ) : (
        card.kind !== 'flashcard' && (
          <div className="flex items-center gap-3">
            <Button variant="brand" className="h-10 px-6" onClick={submit}>
              Check
            </Button>
            <span className="text-sm text-muted-foreground">
              {card.kind === 'mc_single'
                ? '1–9 to choose · Enter to check'
                : 'Enter to check'}
            </span>
          </div>
        )
      )}
    </div>
  )
}
