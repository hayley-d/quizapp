export type FieldError = { field: string; message: string }

export class ApiError extends Error {
  status: number
  fields: FieldError[]
  constructor(status: number, message: string, fields: FieldError[]) {
    super(message)
    this.status = status
    this.fields = fields
  }
  /** Field errors keyed by field name, for inline rendering. */
  byField(): Record<string, string> {
    return Object.fromEntries(this.fields.map((f) => [f.field, f.message]))
  }
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal,
  })
  if (!res.ok) {
    const payload = await res.json().catch(() => null)
    throw new ApiError(
      res.status,
      payload?.message ?? `Request failed (${res.status})`,
      payload?.fields ?? [],
    )
  }
  return res.status === 204 ? (undefined as T) : ((await res.json()) as T)
}

export type Module = { id: number; name: string; created_at: string; deck_count: number }
export type Deck = {
  id: number
  module_id: number | null
  module_name: string | null
  name: string
  description: string
  created_at: string
  card_count: number
}

export type UpdateDeckInput = Partial<{
  name: string
  module_id: number | null
  description: string
}>

export type DeckSort = 'newest' | 'oldest'
/** 'all' | 'none' | a module id */
export type ModuleFilter = 'all' | 'none' | number

export type DeckQuery = {
  q?: string
  moduleId?: ModuleFilter
  sort?: DeckSort
}

function deckQueryString({ q, moduleId, sort }: DeckQuery): string {
  const params = new URLSearchParams()
  if (q && q.trim() !== '') params.set('q', q.trim())
  if (moduleId !== undefined && moduleId !== 'all') params.set('module_id', String(moduleId))
  if (sort) params.set('sort', sort)
  const s = params.toString()
  return s === '' ? '' : `?${s}`
}

export type CardKind = 'mc_single' | 'short_answer' | 'flashcard'

/** User-facing labels for each kind. Single source of truth — DeckPage's list
 * badges and CardEditorPage's kind selector must not drift apart. */
export const KIND_LABEL: Record<CardKind, string> = {
  mc_single: 'Multiple choice',
  short_answer: 'Short answer',
  flashcard: 'Flashcard',
}

export type Choice = { id: number; text_md: string; is_correct: boolean; position: number }
export type Accepted = { id: number; text: string; normalised: string; is_primary: boolean }

export type CardSummary = {
  id: number
  deck_id: number
  kind: CardKind
  prompt_md: string
  image_path: string | null
  answer_md: string | null
  explanation_md: string | null
  archived: boolean
  created_at: string
  updated_at: string
}

export type Card = CardSummary & { choices: Choice[]; accepted: Accepted[] }

/** `position` and `normalised` are server-assigned; never send them. */
export type ChoiceInput = { text_md: string; is_correct: boolean }
export type AcceptedInput = { text: string; is_primary: boolean }

/**
 * The whole editable card. Unlike `updateDeck`, this is a full replace, not a
 * sparse patch — the editor always holds the entire card, so an omitted
 * optional means null on the server.
 */
export type CardInput = {
  kind: CardKind
  prompt_md: string
  answer_md?: string | null
  explanation_md?: string | null
  choices?: ChoiceInput[]
  accepted?: AcceptedInput[]
}

export type CardQuery = {
  deckId?: number
  kind?: CardKind | 'all'
  archived?: 'true' | 'false' | 'all'
}

function cardQueryString({ deckId, kind, archived }: CardQuery): string {
  const params = new URLSearchParams()
  if (deckId !== undefined) params.set('deck_id', String(deckId))
  if (kind && kind !== 'all') params.set('kind', kind)
  if (archived) params.set('archived', archived)
  const s = params.toString()
  return s === '' ? '' : `?${s}`
}

export const api = {
  listModules: () => request<Module[]>('GET', '/modules'),
  createModule: (name: string) => request<Module>('POST', '/modules', { name }),
  listDecks: (query: DeckQuery = {}, signal?: AbortSignal) =>
    request<Deck[]>('GET', `/decks${deckQueryString(query)}`, undefined, signal),
  createDeck: (input: { name: string; module_id: number | null; description: string }) =>
    request<Deck>('POST', '/decks', input),
  // Only send keys the user actually changed — an absent module_id means
  // "leave it alone" on the server, while null means "unparent".
  updateDeck: (id: number, patch: UpdateDeckInput) =>
    request<Deck>('PATCH', `/decks/${id}`, patch),
  getDeck: (id: number, signal?: AbortSignal) =>
    request<Deck>('GET', `/decks/${id}`, undefined, signal),
  listCards: (query: CardQuery = {}, signal?: AbortSignal) =>
    request<CardSummary[]>('GET', `/cards${cardQueryString(query)}`, undefined, signal),
  getCard: (id: number, signal?: AbortSignal) =>
    request<Card>('GET', `/cards/${id}`, undefined, signal),
  createCard: (input: CardInput & { deck_id: number }) =>
    request<Card>('POST', '/cards', input),
  updateCard: (id: number, input: CardInput) =>
    request<Card>('PATCH', `/cards/${id}`, input),
  archiveCard: (id: number) => request<Card>('POST', `/cards/${id}/archive`, {}),
  unarchiveCard: (id: number) => request<Card>('POST', `/cards/${id}/unarchive`, {}),
}
