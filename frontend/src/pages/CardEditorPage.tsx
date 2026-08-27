import { useEffect, useRef, useState, type ChangeEvent, type KeyboardEvent } from 'react'
import { Link, useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import {
  api,
  ApiError,
  KIND_LABEL,
  type AcceptedInput,
  type CardInput,
  type CardKind,
  type ChoiceInput,
} from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { ChoicesEditor } from '@/components/card-editor/ChoicesEditor'
import { AcceptedEditor } from '@/components/card-editor/AcceptedEditor'
import { CardImage } from '@/components/CardImage'
import { CardPreview } from '@/components/card-editor/CardPreview'
import { Input } from '@/components/ui/input'

function emptyChoices(): ChoiceInput[] {
  return [
    { text_md: '', is_correct: false },
    { text_md: '', is_correct: false },
  ]
}

// The single-accepted-answer case is the common one, so the first (and only)
// row defaults to primary — otherwise the validator bounces the first save
// with "Mark one answer as the primary wording" until the user clicks the
// radio it would have had to pick anyway.
function emptyAccepted(): AcceptedInput[] {
  return [{ text: '', is_primary: true }]
}

// `/cards/new` and `/cards/:id/edit` are stable routes: navigating from one
// deck's "New card" link to another, or between two cards' edit links,
// re-renders the same component instance rather than remounting it. Since
// deckId/loadError/loaded are seeded from useState initializers that only run
// once, that would leave stale state from whichever card/deck loaded first.
// Keying on the route's identity forces a fresh mount — and fresh state —
// whenever that identity changes.
export function CardEditorPage() {
  const { id } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const key = id ?? `new:${searchParams.get('deck_id')}`
  return <CardEditorPageInner key={key} />
}

function CardEditorPageInner() {
  const { id } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()

  const mode: 'create' | 'edit' = id !== undefined ? 'edit' : 'create'

  // A non-numeric id is a not-found state, not a fetch attempt.
  const cardId = mode === 'edit' && id !== undefined && /^\d+$/.test(id) ? Number(id) : null

  const deckIdParam = searchParams.get('deck_id')
  const queryDeckId =
    deckIdParam !== null && /^\d+$/.test(deckIdParam) ? Number(deckIdParam) : null

  const [deckId, setDeckId] = useState<number | null>(mode === 'create' ? queryDeckId : null)
  const [kind, setKind] = useState<CardKind>('mc_single')
  const [promptMd, setPromptMd] = useState('')
  const [answerMd, setAnswerMd] = useState('')
  const [explanationMd, setExplanationMd] = useState('')
  const [choices, setChoices] = useState<ChoiceInput[]>(emptyChoices())
  const [accepted, setAccepted] = useState<AcceptedInput[]>(emptyAccepted())
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)
  const [loadError, setLoadError] = useState(mode === 'create' && queryDeckId === null)
  const [loaded, setLoaded] = useState(mode === 'create')
  const [imagePath, setImagePath] = useState<string | null>(null)
  const [imageBusy, setImageBusy] = useState(false)
  const [view, setView] = useState<'edit' | 'preview'>('edit')

  const promptRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (mode !== 'edit') return
    if (cardId === null) {
      setLoadError(true)
      return
    }
    const controller = new AbortController()
    ;(async () => {
      try {
        const card = await api.getCard(cardId, controller.signal)
        setDeckId(card.deck_id)
        setKind(card.kind)
        setPromptMd(card.prompt_md)
        setImagePath(card.image_path)
        setAnswerMd(card.answer_md ?? '')
        setExplanationMd(card.explanation_md ?? '')
        setChoices(
          card.choices.length > 0
            ? card.choices
                .slice()
                .sort((a, b) => a.position - b.position)
                .map((c) => ({ text_md: c.text_md, is_correct: c.is_correct }))
            : emptyChoices(),
        )
        setAccepted(
          card.accepted.length > 0
            ? card.accepted.map((a) => ({ text: a.text, is_primary: a.is_primary }))
            : emptyAccepted(),
        )
        setLoaded(true)
      } catch (e) {
        if ((e as Error)?.name === 'AbortError') return
        setLoadError(true)
      }
    })()
    return () => controller.abort()
  }, [mode, cardId])

  // Coming back from Preview must land the cursor where typing continues,
  // not wherever the toggle button happened to leave it.
  useEffect(() => {
    if (view === 'edit' && loaded) promptRef.current?.focus()
  }, [view, loaded])

  function buildInput(): CardInput {
    const input: CardInput = { kind, prompt_md: promptMd }
    // Sent unconditionally, including null: cards PATCH is a full replace and
    // an absent key means "no image".
    input.image_path = imagePath
    if (explanationMd.trim() !== '') input.explanation_md = explanationMd
    if (kind === 'flashcard') input.answer_md = answerMd
    if (kind === 'mc_single') input.choices = choices
    if (kind === 'short_answer') input.accepted = accepted
    return input
  }

  /** Drops one key from the error map without disturbing the others. */
  function clearError(key: string) {
    setErrors((prev) => Object.fromEntries(
      Object.entries(prev).filter(([k]) => k !== key),
    ))
  }

  async function pickImage(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0]
    // Reset the input before anything else: without this, choosing the same
    // file again after a failure fires no change event and the picker looks
    // dead.
    e.target.value = ''
    if (!file) return

    setImageBusy(true)
    clearError('file')
    try {
      const { path } = await api.uploadImage(file)
      setImagePath(path)
    } catch (err) {
      // A rejected upload must leave typed content alone, exactly like a
      // rejected save — so this merges one field error in rather than
      // replacing the error map.
      if (err instanceof ApiError) {
        const byField = err.byField()
        if (Object.keys(byField).length === 0) toast.error(err.message)
        else setErrors((prev) => ({ ...prev, ...byField }))
      } else {
        toast.error('Could not upload the image')
      }
    } finally {
      setImageBusy(false)
    }
  }

  async function save() {
    setBusy(true)
    setErrors({})
    try {
      return mode === 'edit' && cardId !== null
        ? await api.updateCard(cardId, buildInput())
        // deckId is guaranteed non-null here: create mode with a null deckId
        // renders the "missing deck" error screen instead of this form.
        : await api.createCard({ deck_id: deckId as number, ...buildInput() })
    } catch (e) {
      // Never reset the form here — a rejected save must keep what was typed.
      if (e instanceof ApiError) {
        const byField = e.byField()
        setErrors(byField)
        if (Object.keys(byField).length === 0) toast.error(e.message)
      } else {
        toast.error('Could not reach the server')
      }
      return null
    } finally {
      setBusy(false)
    }
  }

  async function saveAndNext() {
    if (busy) return
    const card = await save()
    if (!card) return
    toast.success('Card saved')
    setPromptMd('')
    setAnswerMd('')
    setExplanationMd('')
    setChoices(emptyChoices())
    setAccepted(emptyAccepted())
    setImagePath(null)
    setErrors({})
    promptRef.current?.focus()
  }

  async function saveAndReturn() {
    if (busy) return
    const card = await save()
    if (!card) return
    navigate(`/decks/${deckId}`)
  }

  function cancel() {
    if (deckId !== null) navigate(`/decks/${deckId}`)
    else navigate('/decks')
  }

  function handleContainerKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    const mod = e.metaKey || e.ctrlKey
    if (mod && e.key === 'Enter') {
      e.preventDefault()
      if (busy) return
      void (mode === 'edit' ? saveAndReturn() : saveAndNext())
    } else if (mod && e.key.toLowerCase() === 's') {
      e.preventDefault()
      if (busy) return
      void saveAndReturn()
    } else if (mod && e.key.toLowerCase() === 'p') {
      // preventDefault matters: this is the browser's print shortcut.
      e.preventDefault()
      setView((v) => (v === 'edit' ? 'preview' : 'edit'))
    } else if (e.key === 'Escape') {
      e.preventDefault()
      if (busy) return
      cancel()
    }
  }

  function changeKind(next: CardKind) {
    setKind(next)
    // Errors from the previous kind's children no longer apply to anything on screen.
    setErrors({})
  }

  // The server can return a field error for the WRONG kind's field (e.g.
  // `answer_md` on an mc_single card) — unreachable from this client today
  // since buildInput() only ever sends kind-appropriate children, but if it
  // ever happened the message would land in a block that isn't mounted and
  // vanish silently. Render anything no other block claims, same spirit as
  // the orphaned-indexed-error fallback in ChoicesEditor/AcceptedEditor.
  const claimedErrorKeys = new Set(['kind', 'prompt_md', 'explanation_md', 'deck_id',
                                    'image_path', 'file'])
  if (kind === 'mc_single') claimedErrorKeys.add('choices')
  if (kind === 'short_answer') claimedErrorKeys.add('accepted')
  if (kind === 'flashcard') claimedErrorKeys.add('answer_md')
  const unclaimedErrors = Object.entries(errors).filter(([key]) => {
    if (claimedErrorKeys.has(key)) return false
    if (kind === 'mc_single' && key.startsWith('choices[')) return false
    if (kind === 'short_answer' && key.startsWith('accepted[')) return false
    return true
  })

  if (loadError) {
    return (
      <div className="space-y-2">
        <h1 className="font-display text-2xl font-bold">
          {mode === 'create' ? 'No deck specified' : 'Card not found'}
        </h1>
        <p className="text-muted-foreground">
          {mode === 'create'
            ? 'A new card needs a deck to belong to.'
            : 'This card may have been removed.'}{' '}
          <Link to="/decks" className="underline">
            Back to decks
          </Link>
        </p>
      </div>
    )
  }

  if (!loaded) {
    return null
  }

  return (
    <div className="max-w-2xl space-y-6" onKeyDown={handleContainerKeyDown}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-display text-2xl font-bold">
          {mode === 'create' ? 'New card' : 'Edit card'}
        </h1>
        <div className="flex items-center gap-1 rounded-lg border p-1">
          <Button
            size="sm" variant={view === 'edit' ? 'secondary' : 'ghost'}
            aria-pressed={view === 'edit'} onClick={() => setView('edit')}
          >
            Edit
          </Button>
          <Button
            size="sm" variant={view === 'preview' ? 'secondary' : 'ghost'}
            aria-pressed={view === 'preview'} onClick={() => setView('preview')}
          >
            Preview
          </Button>
        </div>
      </div>

      {view === 'edit' ? (
      <>
      {errors.deck_id && <p className="text-sm text-destructive">{errors.deck_id}</p>}

      <div className="space-y-2">
        <Label htmlFor="card-kind">Kind</Label>
        <Select value={kind} onValueChange={(v) => changeKind(v as CardKind)}>
          <SelectTrigger id="card-kind"><SelectValue /></SelectTrigger>
          <SelectContent>
            {(Object.keys(KIND_LABEL) as CardKind[]).map((k) => (
              <SelectItem key={k} value={k}>{KIND_LABEL[k]}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        {errors.kind && <p className="text-sm text-destructive">{errors.kind}</p>}
      </div>

      <div className="space-y-2">
        <Label htmlFor="card-prompt">Prompt</Label>
        <Textarea
          id="card-prompt"
          ref={promptRef}
          autoFocus
          rows={4}
          value={promptMd}
          onChange={(e) => setPromptMd(e.target.value)}
          aria-invalid={!!errors.prompt_md}
        />
        {errors.prompt_md && <p className="text-sm text-destructive">{errors.prompt_md}</p>}
      </div>

      <div className="space-y-2">
        <Label htmlFor="card-image">Image (optional)</Label>
        {imagePath === null ? (
          <Input
            id="card-image"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            disabled={imageBusy || busy}
            onChange={(e) => void pickImage(e)}
            aria-invalid={!!errors.file}
          />
        ) : (
          <div className="flex items-center gap-3">
            <CardImage path={imagePath} alt="Card image" />
            <Button
              type="button" variant="secondary" size="sm" disabled={busy}
              onClick={() => { setImagePath(null); clearError('image_path') }}
            >
              Remove
            </Button>
          </div>
        )}
        {imageBusy && <p className="text-sm text-muted-foreground">Uploading…</p>}
        {(errors.file ?? errors.image_path) && (
          <p className="text-sm text-destructive">{errors.file ?? errors.image_path}</p>
        )}
      </div>

      {kind === 'mc_single' && (
        <div className="space-y-2">
          <Label>Choices</Label>
          <ChoicesEditor value={choices} onChange={setChoices} errors={errors} />
        </div>
      )}

      {kind === 'short_answer' && (
        <div className="space-y-2">
          <Label>Accepted answers</Label>
          <AcceptedEditor value={accepted} onChange={setAccepted} errors={errors} />
        </div>
      )}

      {kind === 'flashcard' && (
        <div className="space-y-2">
          <Label htmlFor="card-answer">Answer</Label>
          <Textarea
            id="card-answer"
            rows={4}
            value={answerMd}
            onChange={(e) => setAnswerMd(e.target.value)}
            aria-invalid={!!errors.answer_md}
          />
          {errors.answer_md && <p className="text-sm text-destructive">{errors.answer_md}</p>}
        </div>
      )}

      {unclaimedErrors.map(([key, msg]) => (
        <p key={key} className="text-sm text-destructive">{msg}</p>
      ))}

      <div className="space-y-2">
        <Label htmlFor="card-explanation">Explanation (optional)</Label>
        <Textarea
          id="card-explanation"
          rows={3}
          value={explanationMd}
          onChange={(e) => setExplanationMd(e.target.value)}
          aria-invalid={!!errors.explanation_md}
        />
        {errors.explanation_md && (
          <p className="text-sm text-destructive">{errors.explanation_md}</p>
        )}
      </div>
      </>
      ) : (
        <CardPreview
          kind={kind}
          promptMd={promptMd}
          imagePath={imagePath}
          choices={choices}
          accepted={accepted}
          answerMd={answerMd}
          explanationMd={explanationMd}
        />
      )}

      <div className="flex flex-wrap items-center gap-3 border-t pt-4">
        {mode === 'create' ? (
          <>
            <Button onClick={() => void saveAndNext()} disabled={busy}>
              Save &amp; next
            </Button>
            <Button variant="secondary" onClick={() => void saveAndReturn()} disabled={busy}>
              Save &amp; close
            </Button>
          </>
        ) : (
          <Button onClick={() => void saveAndReturn()} disabled={busy}>
            Save
          </Button>
        )}
        <Button variant="ghost" onClick={cancel} disabled={busy}>
          Cancel
        </Button>
        <p className="text-sm text-muted-foreground">
          {mode === 'create'
            ? '⌘/Ctrl+Enter save & next · ⌘/Ctrl+S save & close · Esc cancel · ⌘/Ctrl+P preview'
            : '⌘/Ctrl+Enter or ⌘/Ctrl+S save & close · Esc cancel · ⌘/Ctrl+P preview'}
        </p>
      </div>
    </div>
  )
}
