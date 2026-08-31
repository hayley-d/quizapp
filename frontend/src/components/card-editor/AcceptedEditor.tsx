import { useEffect, useRef } from 'react'
import { Plus, X } from 'lucide-react'
import type { AcceptedInput } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

const MIN_ROWS = 1

function emptyAccepted(): AcceptedInput {
  return { text: '', is_primary: false }
}

type AcceptedEditorProps = {
  value: AcceptedInput[]
  onChange: (next: AcceptedInput[]) => void
  errors: Record<string, string>
}

export function AcceptedEditor({ value, onChange, errors }: AcceptedEditorProps) {
  const answerTextareas = useRef<(HTMLTextAreaElement | null)[]>([])
  const focusIndex = useRef<number | null>(null)

  useEffect(() => {
    if (focusIndex.current !== null) {
      answerTextareas.current[focusIndex.current]?.focus()
      focusIndex.current = null
    }
  }, [value])

  function setText(rowIndex: number, text: string) {
    const next = value.slice()
    next[rowIndex] = { ...next[rowIndex], text }
    onChange(next)
  }

  function setPrimary(rowIndex: number) {
    onChange(value.map((answer, index) => ({ ...answer, is_primary: index === rowIndex })))
  }

  function addRow() {
    focusIndex.current = value.length
    onChange([...value, emptyAccepted()])
  }

  function removeRow(rowIndex: number) {
    if (value.length <= MIN_ROWS) return
    const next = value.filter((_, index) => index !== rowIndex)
    onChange(next)
  }

  const orphanedErrors = Object.entries(errors)
    .filter(([fieldName]) => {
      const match = /^accepted\[(\d+)\]\./.exec(fieldName)
      return match !== null && Number(match[1]) >= value.length
    })
    .map(([, message]) => message)

  return (
    <div className="space-y-2">
      {errors.accepted && <p className="text-sm text-destructive">{errors.accepted}</p>}
      {orphanedErrors.map((message, index) => (
        <p key={index} className="text-sm text-destructive">{message}</p>
      ))}
      <Label className="text-sm text-muted-foreground">Shown as the answer</Label>
      <RadioGroup
        value={String(value.findIndex((answer) => answer.is_primary))}
        onValueChange={(selectedValue) => setPrimary(Number(selectedValue))}
        className="gap-2"
      >
        {value.map((accepted, rowIndex) => {
          const fieldError = errors[`accepted[${rowIndex}].text`]
          return (
            <div key={rowIndex} className="space-y-1">
              <div className="flex items-start gap-2">
                <RadioGroupItem
                  value={String(rowIndex)}
                  id={`accepted-primary-${rowIndex}`}
                  className="mt-2.5"
                  aria-label={`Mark accepted answer ${rowIndex + 1} as shown answer`}
                />
                <Textarea
                  ref={(element) => { answerTextareas.current[rowIndex] = element }}
                  rows={2}
                  value={accepted.text}
                  onChange={(event) => setText(rowIndex, event.target.value)}
                  aria-invalid={!!fieldError}
                  placeholder={`Accepted answer ${rowIndex + 1}`}
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove accepted answer ${rowIndex + 1}`}
                  disabled={value.length <= MIN_ROWS}
                  onClick={() => removeRow(rowIndex)}
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
