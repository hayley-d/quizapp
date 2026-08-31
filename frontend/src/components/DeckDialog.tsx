import { useState } from 'react'
import { toast } from 'sonner'
import { Trash2 } from 'lucide-react'
import { api, ApiError, type Deck, type DeckDeletionImpact, type Module, type UpdateDeckInput } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog'

const NO_MODULE = 'none'

type DeckDialogProps = {
  modules: Module[]
  deck?: Deck
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved: () => void
  onDeleted: () => void
}

export function DeckDialog({
  modules, deck, open, onOpenChange, onSaved, onDeleted,
}: DeckDialogProps) {
  const [name, setName] = useState(deck?.name ?? '')
  const [moduleId, setModuleId] = useState(
    deck?.module_id != null ? String(deck.module_id) : NO_MODULE,
  )
  const [description, setDescription] = useState(deck?.description ?? '')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)
  const [confirmingDeletion, setConfirmingDeletion] = useState(false)
  const [impact, setImpact] = useState<DeckDeletionImpact | null>(null)
  const [deleting, setDeleting] = useState(false)

  const selectedModuleId = moduleId === NO_MODULE ? null : Number(moduleId)

  async function save() {
    setBusy(true)
    setErrors({})
    try {
      if (deck) {
        const patch: UpdateDeckInput = {}
        if (name !== deck.name) patch.name = name
        if (selectedModuleId !== deck.module_id) patch.module_id = selectedModuleId
        if (description !== deck.description) patch.description = description
        if (Object.keys(patch).length > 0) await api.updateDeck(deck.id, patch)
      } else {
        await api.createDeck({ name, module_id: selectedModuleId, description })
      }
      onOpenChange(false)
      onSaved()
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

  async function openDeletionConfirmation() {
    if (!deck) return
    setImpact(null)
    setConfirmingDeletion(true)
    try {
      setImpact(await api.getDeckDeletionImpact(deck.id))
    } catch {
      setConfirmingDeletion(false)
      toast.error('Could not work out what deleting this deck would remove')
    }
  }

  async function confirmDeletion() {
    if (!deck) return
    setDeleting(true)
    try {
      await api.deleteDeck(deck.id)
      setConfirmingDeletion(false)
      onOpenChange(false)
      onDeleted()
    } catch {
      toast.error('Could not delete the deck')
    } finally {
      setDeleting(false)
    }
  }

  function deletionLines(): string[] {
    if (impact === null) return ['Working out what this would remove…']
    const cardClause =
      impact.card_count === 1 ? '1 card' : `${impact.card_count} cards`
    const answerClause =
      impact.review_count === 1 ? '1 recorded answer' : `${impact.review_count} recorded answers`
    return [
      `${cardClause} and ${answerClause} will be deleted with it.`,
      'This cannot be undone.',
    ]
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="font-display">{deck ? 'Edit deck' : 'New deck'}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="deck-name">Name</Label>
            <Input
              id="deck-name" autoFocus value={name}
              onChange={(event) => setName(event.target.value)}
              aria-invalid={!!errors.name}
            />
            {errors.name && <p className="text-sm text-destructive">{errors.name}</p>}
          </div>
          <div className="space-y-2">
            <Label htmlFor="deck-module">Module</Label>
            <Select value={moduleId} onValueChange={setModuleId}>
              <SelectTrigger id="deck-module"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_MODULE}>No module</SelectItem>
                {modules.map((module) => (
                  <SelectItem key={module.id} value={String(module.id)}>
                    {module.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {errors.module_id && <p className="text-sm text-destructive">{errors.module_id}</p>}
          </div>
          <div className="space-y-2">
            <Label htmlFor="deck-description">Description</Label>
            <Textarea
              id="deck-description" value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </div>
        </div>
        <DialogFooter className={deck ? 'sm:justify-between' : undefined}>
          {deck && (
            <Button
              variant="destructive"
              disabled={busy || deleting}
              onClick={() => void openDeletionConfirmation()}
            >
              <Trash2 className="size-4" />
              Delete deck
            </Button>
          )}
          <Button onClick={save} disabled={busy}>Save</Button>
        </DialogFooter>
      </DialogContent>
      {deck && (
        <ConfirmDeleteDialog
          open={confirmingDeletion}
          onOpenChange={(isOpen) => { if (!isOpen) setConfirmingDeletion(false) }}
          title={`Delete “${deck.name}”?`}
          lines={deletionLines()}
          confirmLabel="Delete deck"
          busy={deleting}
          confirmDisabled={impact === null}
          onConfirm={() => void confirmDeletion()}
        />
      )}
    </Dialog>
  )
}
