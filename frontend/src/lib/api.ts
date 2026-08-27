export type FieldError = { field: string; message: string }

export class ApiError extends Error {
  status: number
  fields: FieldError[]
  constructor(status: number, message: string, fields: FieldError[]) {
    super(message)
    this.status = status
    this.fields = fields
  }
  byField(): Record<string, string> {
    return Object.fromEntries(
      this.fields.map((fieldError) => [fieldError.field, fieldError.message]),
    )
  }
}

async function throwApiError(response: Response): Promise<never> {
  const payload = await response.json().catch(() => null)
  throw new ApiError(
    response.status,
    payload?.message ?? `Request failed (${response.status})`,
    payload?.fields ?? [],
  )
}

async function request<Result>(
  method: string,
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<Result> {
  const response = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal,
  })
  if (!response.ok) await throwApiError(response)
  return response.status === 204
    ? (undefined as Result)
    : ((await response.json()) as Result)
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
export type ModuleFilter = 'all' | 'none' | number

export type DeckQuery = {
  search?: string
  moduleId?: ModuleFilter
  sort?: DeckSort
}

function deckQueryString({ search, moduleId, sort }: DeckQuery): string {
  const searchParams = new URLSearchParams()
  if (search && search.trim() !== '') searchParams.set('search', search.trim())
  if (moduleId !== undefined && moduleId !== 'all') {
    searchParams.set('module_id', String(moduleId))
  }
  if (sort) searchParams.set('sort', sort)
  const queryString = searchParams.toString()
  return queryString === '' ? '' : `?${queryString}`
}

export type CardKind = 'mc_single' | 'short_answer' | 'flashcard'

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
  position: number
  created_at: string
  updated_at: string
}

export type Card = CardSummary & { choices: Choice[]; accepted: Accepted[] }

export type ChoiceInput = { text_md: string; is_correct: boolean }
export type AcceptedInput = { text: string; is_primary: boolean }

export type CardInput = {
  kind: CardKind
  prompt_md: string
  image_path?: string | null
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
  const searchParams = new URLSearchParams()
  if (deckId !== undefined) searchParams.set('deck_id', String(deckId))
  if (kind && kind !== 'all') searchParams.set('kind', kind)
  if (archived) searchParams.set('archived', archived)
  const queryString = searchParams.toString()
  return queryString === '' ? '' : `?${queryString}`
}

export type UploadedImage = { path: string }

async function uploadImage(file: File, signal?: AbortSignal): Promise<UploadedImage> {
  const formData = new FormData()
  formData.append('file', file)
  const response = await fetch('/api/images', {
    method: 'POST',
    body: formData,
    signal,
  })
  if (!response.ok) await throwApiError(response)
  return (await response.json()) as UploadedImage
}

export const api = {
  listModules: () => request<Module[]>('GET', '/modules'),
  createModule: (name: string) => request<Module>('POST', '/modules', { name }),
  listDecks: (query: DeckQuery = {}, signal?: AbortSignal) =>
    request<Deck[]>('GET', `/decks${deckQueryString(query)}`, undefined, signal),
  createDeck: (input: { name: string; module_id: number | null; description: string }) =>
    request<Deck>('POST', '/decks', input),
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
  moveCard: (id: number, before: number | null) =>
    request<Card>('POST', `/cards/${id}/move`, { before }),
  uploadImage,
}
