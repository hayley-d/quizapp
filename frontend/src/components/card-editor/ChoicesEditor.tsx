import { useEffect, useRef, type KeyboardEvent } from 'react'
import { Plus, X } from 'lucide-react'
import type { ChoiceInput } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

const MIN_ROWS = 2

function emptyChoice(): ChoiceInput {
  return { text_md: '', is_correct: false }
}

type Props = {
  value: ChoiceInput[]
  onChange: (next: ChoiceInput[]) => void
  errors: Record<string, string>
}

export function ChoicesEditor({ value, onChange, errors }: Props) {
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
    next[i] = { ...next[i], text_md: text }
    onChange(next)
  }

  function setCorrect(i: number) {
    onChange(value.map((c, idx) => ({ ...c, is_correct: idx === i })))
  }

  function addRow() {
    focusIndex.current = value.length
    onChange([...value, emptyChoice()])
  }

  function removeRow(i: number) {
    if (value.length <= MIN_ROWS) return
    const next = value.filter((_, idx) => idx !== i)
    onChange(next)
  }

  function handleKeyDown(e: KeyboardEvent<HTMLInputElement>, i: number) {
    if (e.key !== 'Enter') return
    e.preventDefault()
    if (i === value.length - 1) {
      addRow()
    } else {
      inputRefs.current[i + 1]?.focus()
    }
  }

  return (
    <div className="space-y-2">
      {errors.choices && <p className="text-sm text-destructive">{errors.choices}</p>}
      <RadioGroup
        value={String(value.findIndex((c) => c.is_correct))}
        onValueChange={(v) => setCorrect(Number(v))}
        className="gap-2"
      >
        {value.map((choice, i) => {
          const fieldError = errors[`choices[${i}].text_md`]
          return (
            <div key={i} className="space-y-1">
              <div className="flex items-center gap-2">
                <RadioGroupItem
                  value={String(i)}
                  id={`choice-correct-${i}`}
                  aria-label={`Mark choice ${i + 1} as correct`}
                />
                <Input
                  ref={(el) => { inputRefs.current[i] = el }}
                  value={choice.text_md}
                  onChange={(e) => setText(i, e.target.value)}
                  onKeyDown={(e) => handleKeyDown(e, i)}
                  aria-invalid={!!fieldError}
                  placeholder={`Choice ${i + 1}`}
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove choice ${i + 1}`}
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
        Add choice
      </Button>
    </div>
  )
}
