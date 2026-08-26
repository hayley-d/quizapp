import { useEffect, useRef, type KeyboardEvent } from 'react'
import { Plus, X } from 'lucide-react'
import type { AcceptedInput } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

const MIN_ROWS = 1

function emptyAccepted(): AcceptedInput {
  return { text: '', is_primary: false }
}

type Props = {
  value: AcceptedInput[]
  onChange: (next: AcceptedInput[]) => void
  errors: Record<string, string>
}

export function AcceptedEditor({ value, onChange, errors }: Props) {
  // One ref per row so a freshly-appended row can be focused after render.
  const inputRefs = useRef<(HTMLInputElement | null)[]>([])
  const focusIndex = useRef<number | null>(null)

  useEffect(() => {
    if (focusIndex.current !== null) {
      inputRefs.current[focusIndex.current]?.focus()
      focusIndex.current = null
    }
  }, [value])

  function setText(i: number, text: string) {
    const next = value.slice()
    next[i] = { ...next[i], text }
    onChange(next)
  }

  function setPrimary(i: number) {
    onChange(value.map((a, idx) => ({ ...a, is_primary: idx === i })))
  }

  function addRow() {
    focusIndex.current = value.length
    onChange([...value, emptyAccepted()])
  }

  function removeRow(i: number) {
    if (value.length <= MIN_ROWS) return
    const next = value.filter((_, idx) => idx !== i)
    onChange(next)
  }

  function handleKeyDown(e: KeyboardEvent<HTMLInputElement>, i: number) {
    if (e.key !== 'Enter') return
    // Cmd/Ctrl+Enter is the page's save-and-next shortcut, not "append a row" —
    // let it bubble untouched to the container's handler instead of also
    // appending here, or the keystroke would do both at once.
    if (e.metaKey || e.ctrlKey) return
    e.preventDefault()
    if (i === value.length - 1) {
      addRow()
    } else {
      inputRefs.current[i + 1]?.focus()
    }
  }

  // A 422 can name an indexed row (e.g. `accepted[2].text`) for a row the
  // user has since deleted while the save was in flight. Surface it as a
  // list-level notice rather than letting it silently vanish.
  const orphanedErrors = Object.entries(errors)
    .filter(([k]) => {
      const m = /^accepted\[(\d+)\]\./.exec(k)
      return m !== null && Number(m[1]) >= value.length
    })
    .map(([, msg]) => msg)

  return (
    <div className="space-y-2">
      {errors.accepted && <p className="text-sm text-destructive">{errors.accepted}</p>}
      {orphanedErrors.map((msg, i) => (
        <p key={i} className="text-sm text-destructive">{msg}</p>
      ))}
      <Label className="text-sm text-muted-foreground">Shown as the answer</Label>
      <RadioGroup
        value={String(value.findIndex((a) => a.is_primary))}
        onValueChange={(v) => setPrimary(Number(v))}
        className="gap-2"
      >
        {value.map((accepted, i) => {
          const fieldError = errors[`accepted[${i}].text`]
          return (
            <div key={i} className="space-y-1">
              <div className="flex items-center gap-2">
                <RadioGroupItem
                  value={String(i)}
                  id={`accepted-primary-${i}`}
                  aria-label={`Mark accepted answer ${i + 1} as shown answer`}
                />
                <Input
                  ref={(el) => { inputRefs.current[i] = el }}
                  value={accepted.text}
                  onChange={(e) => setText(i, e.target.value)}
                  onKeyDown={(e) => handleKeyDown(e, i)}
                  aria-invalid={!!fieldError}
                  placeholder={`Accepted answer ${i + 1}`}
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove accepted answer ${i + 1}`}
                  disabled={value.length <= MIN_ROWS}
                  onClick={() => removeRow(i)}
                >
                  <X className="size-4" />
                </Button>
              </div>
              {fieldError && (
                <p className="pl-7 text-sm text-destructive">{fieldError}</p>
              )}
            </div>
          )
        })}
      </RadioGroup>
      <Button type="button" variant="secondary" size="sm" onClick={addRow}>
        <Plus className="size-4" />
        Add accepted answer
      </Button>
    </div>
  )
}
