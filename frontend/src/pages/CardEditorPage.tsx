import {
  useEffect, useRef, useState,
  type ChangeEvent, type KeyboardEvent, type ReactNode,
} from 'react'
import { Link, useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { ArrowLeft, ImagePlus, Info, X } from 'lucide-react'
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
  Tooltip, TooltipContent, TooltipTrigger,
} from '@/components/ui/tooltip'
import { ChoicesEditor } from '@/components/card-editor/ChoicesEditor'
import { AcceptedEditor } from '@/components/card-editor/AcceptedEditor'
import { CardImage } from '@/components/CardImage'
import { CardPreview } from '@/components/card-editor/CardPreview'

const ANSWER_HINT: Record<CardKind, string> = {
  mc_single: 'Mark the choice that counts as correct.',
  text_answer: 'Any accepted answer counts as correct.',
  flashcard: 'Revealed when you ask to see the answer.',
}

type KeyboardShortcut = { keys: string; action: string }

const KEYBOARD_SHORTCUTS: Record<'create' | 'edit', KeyboardShortcut[]> = {
  create: [
    { keys: '\u2318/Ctrl+Enter', action: 'save & next' },
    { keys: '\u2318/Ctrl+S', action: 'save & close' },
    { keys: 'Esc', action: 'cancel' },
    { keys: '\u2318/Ctrl+P', action: 'preview' },
  ],
  edit: [
    { keys: '\u2318/Ctrl+Enter or \u2318/Ctrl+S', action: 'save & close' },
    { keys: 'Esc', action: 'cancel' },
    { keys: '\u2318/Ctrl+P', action: 'preview' },
  ],
}

type EditorSectionProps = {
  title: string
  hint: string
  children: ReactNode
}

function EditorSection({ title, hint, children }: EditorSectionProps) {
  return (
    <section className="mt-5 space-y-3 border-t pt-5 first:mt-0 first:border-t-0 first:pt-0">
      <div className="flex items-center gap-1.5">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {title}
        </h2>
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              aria-label={`About ${title.toLowerCase()}`}
              className="inline-flex size-4 items-center justify-center rounded-full text-muted-foreground transition-colors hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
            >
              <Info className="size-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent>{hint}</TooltipContent>
        </Tooltip>
      </div>
      {children}
    </section>
  )
}

function emptyChoices(): ChoiceInput[] {
  return [
    { text_md: '', is_correct: false },
    { text_md: '', is_correct: false },
  ]
}

function emptyAccepted(): AcceptedInput[] {
  return [{ text: '', is_primary: true }]
}

export function CardEditorPage() {
  const { id } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const routeKey = id ?? `new:${searchParams.get('deck_id')}`
  return <CardEditorPageInner key={routeKey} />
}

function CardEditorPageInner() {
  const { id } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()

  const mode: 'create' | 'edit' = id !== undefined ? 'edit' : 'create'

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

  const promptTextarea = useRef<HTMLTextAreaElement>(null)

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
                .sort((left, right) => left.position - right.position)
                .map((choice) => ({
                  text_md: choice.text_md,
                  is_correct: choice.is_correct,
                }))
            : emptyChoices(),
        )
        setAccepted(
          card.accepted.length > 0
            ? card.accepted.map((answer) => ({
                text: answer.text,
                is_primary: answer.is_primary,
              }))
            : emptyAccepted(),
        )
        setLoaded(true)
      } catch (error) {
        if ((error as Error)?.name === 'AbortError') return
        setLoadError(true)
      }
    })()
    return () => controller.abort()
  }, [mode, cardId])

  useEffect(() => {
    if (view === 'edit' && loaded) promptTextarea.current?.focus()
  }, [view, loaded])

  function buildInput(): CardInput {
    const input: CardInput = { kind, prompt_md: promptMd }
    input.image_path = imagePath
    if (explanationMd.trim() !== '') input.explanation_md = explanationMd
    if (kind === 'flashcard') input.answer_md = answerMd
    if (kind === 'mc_single') input.choices = choices
    if (kind === 'text_answer') input.accepted = accepted
    return input
  }

  function clearError(fieldToClear: string) {
    setErrors((previousErrors) => Object.fromEntries(
      Object.entries(previousErrors).filter(([field]) => field !== fieldToClear),
    ))
  }

  async function pickImage(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return

    setImageBusy(true)
    clearError('file')
    try {
      const { path } = await api.uploadImage(file)
      setImagePath(path)
    } catch (error) {
      if (error instanceof ApiError) {
        const errorsByField = error.byField()
        if (Object.keys(errorsByField).length === 0) toast.error(error.message)
        else setErrors((previousErrors) => ({ ...previousErrors, ...errorsByField }))
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
        : await api.createCard({ deck_id: deckId as number, ...buildInput() })
    } catch (error) {
      if (error instanceof ApiError) {
        const errorsByField = error.byField()
        setErrors(errorsByField)
        if (Object.keys(errorsByField).length === 0) toast.error(error.message)
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
    promptTextarea.current?.focus()
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

  function handleContainerKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const modifierHeld = event.metaKey || event.ctrlKey
    if (modifierHeld && event.key === 'Enter') {
      event.preventDefault()
      if (busy) return
      void (mode === 'edit' ? saveAndReturn() : saveAndNext())
    } else if (modifierHeld && event.key.toLowerCase() === 's') {
      event.preventDefault()
      if (busy) return
      void saveAndReturn()
    } else if (modifierHeld && event.key.toLowerCase() === 'p') {
      event.preventDefault()
      setView((currentView) => (currentView === 'edit' ? 'preview' : 'edit'))
    } else if (event.key === 'Escape') {
      event.preventDefault()
      if (busy) return
      cancel()
    }
  }

  function changeKind(next: CardKind) {
    setKind(next)
    setErrors({})
  }

  const claimedErrorKeys = new Set(['kind', 'prompt_md', 'explanation_md', 'deck_id',
                                    'image_path', 'file'])
  if (kind === 'mc_single') claimedErrorKeys.add('choices')
  if (kind === 'text_answer') claimedErrorKeys.add('accepted')
  if (kind === 'flashcard') claimedErrorKeys.add('answer_md')
  const unclaimedErrors = Object.entries(errors).filter(([field]) => {
    if (claimedErrorKeys.has(field)) return false
    if (kind === 'mc_single' && field.startsWith('choices[')) return false
    if (kind === 'text_answer' && field.startsWith('accepted[')) return false
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
    <div className="space-y-6" onKeyDown={handleContainerKeyDown}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Back to deck"
            title="Back to deck"
            disabled={busy}
            onClick={cancel}
          >
            <ArrowLeft />
          </Button>
          <div className="space-y-0.5">
            <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {KIND_LABEL[kind]}
            </p>
            <h1 className="font-display text-2xl font-bold leading-none">
              {mode === 'create' ? 'New card' : 'Edit card'}
            </h1>
          </div>
        </div>
        <div className="flex items-center gap-1 rounded-lg border bg-muted/40 p-1">
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

      <div className="mx-auto w-full max-w-2xl space-y-6">
      {view === 'edit' ? (
      <div className="rounded-xl border bg-card p-5 shadow-sm">
        {errors.deck_id && (
          <p className="pb-4 text-sm text-destructive">{errors.deck_id}</p>
        )}

        <EditorSection title="Format" hint="How you answer this card.">
          <div className="inline-flex flex-wrap gap-1 rounded-lg border bg-muted/40 p-1">
            {(Object.keys(KIND_LABEL) as CardKind[]).map((cardKind) => (
              <Button
                key={cardKind}
                type="button"
                size="sm"
                variant={kind === cardKind ? 'secondary' : 'ghost'}
                aria-pressed={kind === cardKind}
                onClick={() => changeKind(cardKind)}
              >
                {KIND_LABEL[cardKind]}
              </Button>
            ))}
          </div>
          {errors.kind && <p className="text-sm text-destructive">{errors.kind}</p>}
        </EditorSection>

        <EditorSection title="Question" hint="Markdown and LaTeX are supported.">
          <Label htmlFor="card-prompt" className="sr-only">Prompt</Label>
          <Textarea
            id="card-prompt"
            ref={promptTextarea}
            autoFocus
            rows={4}
            placeholder="Ask the question"
            value={promptMd}
            onChange={(event) => setPromptMd(event.target.value)}
            aria-invalid={!!errors.prompt_md}
          />
          {errors.prompt_md && <p className="text-sm text-destructive">{errors.prompt_md}</p>}

          {imagePath === null ? (
            <div className="flex flex-wrap items-center gap-3">
              <input
                id="card-image"
                type="file"
                className="sr-only"
                accept="image/png,image/jpeg,image/webp"
                disabled={imageBusy || busy}
                onChange={(event) => void pickImage(event)}
                aria-invalid={!!errors.file}
              />
              <Button
                asChild
                variant="secondary"
                size="sm"
                className={imageBusy || busy ? 'pointer-events-none opacity-50' : ''}
              >
                <label htmlFor="card-image">
                  <ImagePlus />
                  {imageBusy ? 'Uploading' : 'Add an image'}
                </label>
              </Button>
              <span className="text-sm text-muted-foreground">
                Optional · PNG, JPEG or WebP
              </span>
            </div>
          ) : (
            <div className="flex items-center gap-3">
              <CardImage path={imagePath} altText="Card image" />
              <Button
                type="button" variant="ghost" size="sm" disabled={busy}
                onClick={() => { setImagePath(null); clearError('image_path') }}
              >
                <X />
                Remove image
              </Button>
            </div>
          )}
          {(errors.file ?? errors.image_path) && (
            <p className="text-sm text-destructive">{errors.file ?? errors.image_path}</p>
          )}
        </EditorSection>

        <EditorSection title="Answer" hint={ANSWER_HINT[kind]}>
          {kind === 'mc_single' && (
            <ChoicesEditor value={choices} onChange={setChoices} errors={errors} />
          )}
          {kind === 'text_answer' && (
            <AcceptedEditor value={accepted} onChange={setAccepted} errors={errors} />
          )}
          {kind === 'flashcard' && (
            <>
              <Label htmlFor="card-answer" className="sr-only">Answer</Label>
              <Textarea
                id="card-answer"
                rows={4}
                placeholder="Write the answer"
                value={answerMd}
                onChange={(event) => setAnswerMd(event.target.value)}
                aria-invalid={!!errors.answer_md}
              />
              {errors.answer_md && (
                <p className="text-sm text-destructive">{errors.answer_md}</p>
              )}
            </>
          )}
        </EditorSection>

        <EditorSection title="Explanation" hint="Optional. Shown once the card has been answered.">
          <Label htmlFor="card-explanation" className="sr-only">Explanation</Label>
          <Textarea
            id="card-explanation"
            rows={3}
            placeholder="Explain why the answer is what it is"
            value={explanationMd}
            onChange={(event) => setExplanationMd(event.target.value)}
            aria-invalid={!!errors.explanation_md}
          />
          {errors.explanation_md && (
            <p className="text-sm text-destructive">{errors.explanation_md}</p>
          )}
        </EditorSection>

        {unclaimedErrors.length > 0 && (
          <div className="space-y-1 border-t pt-5">
            {unclaimedErrors.map(([field, message]) => (
              <p key={field} className="text-sm text-destructive">{message}</p>
            ))}
          </div>
        )}
      </div>
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

      <div className="space-y-3 border-t pt-4">
        <div className="flex flex-wrap items-center gap-3">
          {mode === 'create' ? (
            <>
              <Button onClick={() => void saveAndNext()} disabled={busy}>
                Save &amp; next
              </Button>
              <Button
                variant="secondary"
                onClick={() => void saveAndReturn()}
                disabled={busy}
              >
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
        </div>
        <p className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {KEYBOARD_SHORTCUTS[mode].map((shortcut) => (
            <span key={shortcut.action} className="flex items-center gap-1.5">
              <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 font-sans text-[0.7rem] font-medium">
                {shortcut.keys}
              </kbd>
              {shortcut.action}
            </span>
          ))}
        </p>
      </div>
      </div>
    </div>
  )
}
