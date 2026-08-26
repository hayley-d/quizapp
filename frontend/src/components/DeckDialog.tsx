import { useState } from 'react'
import { toast } from 'sonner'
import { api, ApiError, type Deck, type Module } from '@/lib/api'
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

type Props = {
  modules: Module[]
  deck?: Deck            // absent => create mode
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}

export function DeckDialog({ modules, deck, open, onOpenChange, onSaved }: Props) {
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
        const patch: Record<string, unknown> = {}
        if (name !== deck.name) patch.name = name
        if (selectedModuleId !== deck.module_id) patch.module_id = selectedModuleId
        if (description !== deck.description) patch.description = description
        if (Object.keys(patch).length > 0) await api.updateDeck(deck.id, patch)
      } else {
        await api.createDeck({ name, module_id: selectedModuleId, description })
      }
      onOpenChange(false)
      onSaved()
    } catch (e) {
      // Never reset the form here — a rejected save must keep what was typed.
      if (e instanceof ApiError) {
        const byField = e.byField()
        setErrors(e.status === 409 ? { name: e.message } : byField)
        if (e.status !== 409 && Object.keys(byField).length === 0) toast.error(e.message)
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
              onChange={(e) => setName(e.target.value)}
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
                {modules.map((m) => (
                  <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            {errors.module_id && <p className="text-sm text-destructive">{errors.module_id}</p>}
          </div>
          <div className="space-y-2">
            <Label htmlFor="deck-description">Description</Label>
            <Textarea
              id="deck-description" value={description}
              onChange={(e) => setDescription(e.target.value)}
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
