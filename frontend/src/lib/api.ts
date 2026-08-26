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

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
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

export const api = {
  listModules: () => request<Module[]>('GET', '/modules'),
  createModule: (name: string) => request<Module>('POST', '/modules', { name }),
  listDecks: () => request<Deck[]>('GET', '/decks'),
  createDeck: (input: { name: string; module_id: number | null; description: string }) =>
    request<Deck>('POST', '/decks', input),
  // Only send keys the user actually changed — an absent module_id means
  // "leave it alone" on the server, while null means "unparent".
  updateDeck: (
    id: number,
    patch: Partial<{ name: string; module_id: number | null; description: string }>,
  ) => request<Deck>('PATCH', `/decks/${id}`, patch),
}
