import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { api, type Deck, type Module } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ModuleDialog } from '@/components/ModuleDialog'
import { DeckDialog } from '@/components/DeckDialog'

export function DecksPage() {
  const [modules, setModules] = useState<Module[]>([])
  const [decks, setDecks] = useState<Deck[]>([])
  const [editing, setEditing] = useState<Deck | 'new' | null>(null)

  const load = useCallback(async () => {
    try {
      const [m, d] = await Promise.all([api.listModules(), api.listDecks()])
      setModules(m)
      setDecks(d)
    } catch {
      toast.error('Could not load decks')
    }
  }, [])

  useEffect(() => { void load() }, [load])

  const groups = useMemo(() => {
    const named = modules.map((m) => ({
      key: String(m.id),
      title: m.name,
      decks: decks.filter((d) => d.module_id === m.id),
    }))
    const loose = decks.filter((d) => d.module_id === null)
    return loose.length > 0
      ? [...named, { key: 'none', title: 'No module', decks: loose }]
      : named
  }, [modules, decks])

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-display text-2xl font-bold">Decks</h1>
        <div className="flex gap-2">
          <ModuleDialog onSaved={load} />
          <Button onClick={() => setEditing('new')}>New deck</Button>
        </div>
      </div>

      {groups.length === 0 && (
        <p className="text-muted-foreground">
          No modules or decks yet. Create a module (e.g. COS781), then a deck for each test.
        </p>
      )}

      {groups.map((g) => (
        <section key={g.key} className="space-y-3">
          <h2 className="font-display text-lg font-semibold text-primary">{g.title}</h2>
          {g.decks.length === 0 ? (
            <p className="text-sm text-muted-foreground">No decks in this module yet.</p>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2">
              {g.decks.map((d) => (
                <Card key={d.id}>
                  <CardHeader className="flex flex-row items-start justify-between gap-2">
                    <div>
                      <CardTitle className="font-display text-base">{d.name}</CardTitle>
                      <p className="text-sm text-muted-foreground">
                        {d.card_count} card{d.card_count === 1 ? '' : 's'}
                      </p>
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
          )}
        </section>
      ))}

      {editing && (
        <DeckDialog
          key={editing === 'new' ? 'new' : editing.id}
          modules={modules}
          deck={editing === 'new' ? undefined : editing}
          open
          onOpenChange={(o) => { if (!o) setEditing(null) }}
          onSaved={load}
        />
      )}
    </div>
  )
}
