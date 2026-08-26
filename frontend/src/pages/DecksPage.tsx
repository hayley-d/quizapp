import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { api, type Deck, type DeckSort, type Module, type ModuleFilter } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { ModuleDialog } from '@/components/ModuleDialog'
import { DeckDialog } from '@/components/DeckDialog'

const ALL = 'all'
const NONE = 'none'

export function DecksPage() {
  const [modules, setModules] = useState<Module[]>([])
  const [decks, setDecks] = useState<Deck[]>([])
  const [editing, setEditing] = useState<Deck | 'new' | null>(null)

  // `search` is what the user is typing; `debounced` is what we actually query with.
  const [search, setSearch] = useState('')
  const [debounced, setDebounced] = useState('')
  const [moduleFilter, setModuleFilter] = useState<ModuleFilter>(ALL)
  const [sort, setSort] = useState<DeckSort>('newest')
  const [loading, setLoading] = useState(false)

  const filtersActive = debounced.trim() !== '' || moduleFilter !== ALL

  useEffect(() => {
    const t = setTimeout(() => setDebounced(search), 250)
    return () => clearTimeout(t)
  }, [search])

  const loadModules = useCallback(async () => {
    try {
      setModules(await api.listModules())
    } catch {
      toast.error('Could not load modules')
    }
  }, [])

  useEffect(() => { void loadModules() }, [loadModules])

  // One in-flight deck request at a time. Aborting the previous one is what stops a
  // slow earlier response from overwriting a newer one.
  const inFlight = useRef<AbortController | null>(null)

  const loadDecks = useCallback(async () => {
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    try {
      const rows = await api.listDecks(
        { q: debounced, moduleId: moduleFilter, sort },
        controller.signal,
      )
      setDecks(rows)
    } catch (e) {
      if ((e as Error)?.name === 'AbortError') return   // superseded; not an error
      toast.error('Could not load decks')
    } finally {
      if (inFlight.current === controller) setLoading(false)
    }
  }, [debounced, moduleFilter, sort])

  useEffect(() => { void loadDecks() }, [loadDecks])

  // Cancel any in-flight deck request if the page unmounts.
  useEffect(() => () => inFlight.current?.abort(), [])

  function clearFilters() {
    setSearch('')
    setDebounced('')
    setModuleFilter(ALL)
  }

  const moduleName = (m: ModuleFilter) =>
    m === ALL ? 'All modules'
      : m === NONE ? 'No module'
        : (modules.find((x) => x.id === m)?.name ?? 'Unknown module')

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-display text-2xl font-bold">Decks</h1>
        <div className="flex gap-2">
          <ModuleDialog onSaved={() => { void loadModules(); void loadDecks() }} />
          <Button onClick={() => setEditing('new')}>New deck</Button>
        </div>
      </div>

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <Input
          className="sm:max-w-xs"
          placeholder="Search deck names…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Select
          value={String(moduleFilter)}
          onValueChange={(v) =>
            setModuleFilter(v === ALL || v === NONE ? (v as ModuleFilter) : Number(v))
          }
        >
          <SelectTrigger className="sm:w-52"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>All modules</SelectItem>
            <SelectItem value={NONE}>No module</SelectItem>
            {modules.map((m) => (
              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={sort} onValueChange={(v) => setSort(v as DeckSort)}>
          <SelectTrigger className="sm:w-44"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="newest">Newest first</SelectItem>
            <SelectItem value="oldest">Oldest first</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {decks.length === 0 && !loading && (
        filtersActive ? (
          <div className="space-y-2">
            <p className="text-muted-foreground">
              No decks match “{debounced}” in {moduleName(moduleFilter)}.
            </p>
            <Button variant="secondary" size="sm" onClick={clearFilters}>
              Clear filters
            </Button>
          </div>
        ) : (
          <p className="text-muted-foreground">
            No decks yet. Create a module (e.g. COS781), then a deck for each test.
          </p>
        )
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        {decks.map((d) => (
          <Card key={d.id}>
            <CardHeader className="flex flex-row items-start justify-between gap-2">
              <div className="space-y-1">
                <CardTitle className="font-display text-base">{d.name}</CardTitle>
                <div className="flex items-center gap-2">
                  {d.module_id === null ? (
                    <Badge variant="outline" className="text-muted-foreground">
                      No module
                    </Badge>
                  ) : (
                    <button
                      type="button"
                      onClick={() => setModuleFilter(d.module_id as number)}
                      title={`Filter by ${d.module_name}`}
                    >
                      <Badge variant="secondary">{d.module_name}</Badge>
                    </button>
                  )}
                  <span className="text-sm text-muted-foreground">
                    {d.card_count} card{d.card_count === 1 ? '' : 's'}
                  </span>
                </div>
              </div>
              <Button variant="ghost" size="sm" onClick={() => setEditing(d)}>
                Edit
              </Button>
            </CardHeader>
            {d.description && (
              <CardContent className="text-sm text-muted-foreground">
                {d.description}
              </CardContent>
            )}
          </Card>
        ))}
      </div>

      {editing && (
        <DeckDialog
          key={editing === 'new' ? 'new' : editing.id}
          modules={modules}
          deck={editing === 'new' ? undefined : editing}
          open
          onOpenChange={(o) => { if (!o) setEditing(null) }}
          onSaved={() => { void loadDecks() }}
        />
      )}
    </div>
  )
}
