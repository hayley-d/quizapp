import { useState } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { api, ApiError, type Module } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger,
} from '@/components/ui/dialog'
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog'

type ModuleDialogProps = {
  modules: Module[]
  onChanged: (deletedModuleId: number | null) => void
}

export function ModuleDialog({ modules, onChanged }: ModuleDialogProps) {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)
  const [modulePendingDeletion, setModulePendingDeletion] = useState<Module | null>(null)
  const [deleting, setDeleting] = useState(false)

  async function save() {
    setBusy(true)
    setErrors({})
    try {
      await api.createModule(name)
      setName('')
      onChanged(null)
    } catch (error) {
      if (error instanceof ApiError) {
        const errorsByField = error.byField()
        setErrors(error.status === 409 ? { name: error.message } : errorsByField)
        if (error.status !== 409 && Object.keys(errorsByField).length === 0) {
          toast.error(error.message)
        }
      } else {
        toast.error('Could not reach the server')
      }
    } finally {
      setBusy(false)
    }
  }

  async function confirmDeletion() {
    const module = modulePendingDeletion
    if (module === null) return
    setDeleting(true)
    try {
      await api.deleteModule(module.id)
      setModulePendingDeletion(null)
      onChanged(module.id)
    } catch {
      toast.error('Could not delete the module')
    } finally {
      setDeleting(false)
    }
  }

  function deletionLines(module: Module): string[] {
    if (module.deck_count === 0) return ['It has no decks.']
    const deckClause =
      module.deck_count === 1 ? 'Its 1 deck' : `Its ${module.deck_count} decks`
    return [`${deckClause} will be kept, and move to No module.`]
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="brand" className="h-10 px-4">
          Modules
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader><DialogTitle className="font-display">Modules</DialogTitle></DialogHeader>

        <div className="space-y-2">
          <Label htmlFor="module-name">New module</Label>
          <div className="flex gap-2">
            <Input
              id="module-name"
              autoFocus
              className="min-w-0 flex-1"
              value={name}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => { if (event.key === 'Enter') void save() }}
              aria-invalid={!!errors.name}
            />
            <Button onClick={() => void save()} disabled={busy}>
              <Plus className="size-4" />
              Add
            </Button>
          </div>
          {errors.name && <p className="text-sm text-destructive">{errors.name}</p>}
        </div>

        <div className="space-y-1 border-t pt-3">
          {modules.length === 0 ? (
            <p className="text-muted-foreground">No modules yet.</p>
          ) : (
            <ul className="space-y-1">
              {modules.map((module) => (
                <li key={module.id} className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 truncate">{module.name}</span>
                  <span className="text-muted-foreground">
                    {module.deck_count} deck{module.deck_count === 1 ? '' : 's'}
                  </span>
                  <Button
                    variant="destructive"
                    size="icon-sm"
                    aria-label={`Delete module ${module.name}`}
                    title="Delete module"
                    disabled={deleting}
                    onClick={() => setModulePendingDeletion(module)}
                  >
                    <Trash2 />
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </DialogContent>

      {modulePendingDeletion !== null && (
        <ConfirmDeleteDialog
          open
          onOpenChange={(isOpen) => { if (!isOpen) setModulePendingDeletion(null) }}
          title={`Delete “${modulePendingDeletion.name}”?`}
          lines={deletionLines(modulePendingDeletion)}
          confirmLabel="Delete module"
          busy={deleting}
          onConfirm={() => void confirmDeletion()}
        />
      )}
    </Dialog>
  )
}
