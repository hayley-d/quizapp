import { useState } from 'react'
import { toast } from 'sonner'
import { api, ApiError, type Deck, type Module, type UpdateDeckInput } from '@/lib/api'
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

const NO_MODULE = 'none'

type DeckDialogProps = {
  modules: Module[]
  deck?: Deck
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}

export function DeckDialog({
  modules, deck, open, onOpenChange, onSaved,
}: DeckDialogProps) {
  const [name, setName] = useState(deck?.name ?? '')
  const [moduleId, setModuleId] = useState(
    deck?.module_id != null ? String(deck.module_id) : NO_MODULE,
  )
  const [description, setDescription] = useState(deck?.description ?? '')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)

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
        <DialogFooter>
          <Button onClick={save} disabled={busy}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
