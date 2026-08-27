import { useState } from 'react'
import { Plus } from 'lucide-react'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from '@/components/ui/dialog'

export function ModuleDialog({ onSaved }: { onSaved: () => void }) {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)

  async function save() {
    setBusy(true)
    setErrors({})
    try {
      await api.createModule(name)
      setName('')
      setOpen(false)
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
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="brand" className="h-10 px-4">
          <Plus className="size-4" />
          Create module
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader><DialogTitle className="font-display">New module</DialogTitle></DialogHeader>
        <div className="space-y-2">
          <Label htmlFor="module-name">Name</Label>
          <Input
            id="module-name"
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => { if (event.key === 'Enter') save() }}
            aria-invalid={!!errors.name}
          />
          {errors.name && <p className="text-sm text-destructive">{errors.name}</p>}
        </div>
        <DialogFooter>
          <Button onClick={save} disabled={busy}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
