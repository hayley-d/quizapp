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

type ChoicesEditorProps = {
  value: ChoiceInput[]
  onChange: (next: ChoiceInput[]) => void
  errors: Record<string, string>
}

export function ChoicesEditor({ value, onChange, errors }: ChoicesEditorProps) {
  const inputElements = useRef<(HTMLInputElement | null)[]>([])
  const focusIndex = useRef<number | null>(null)

  useEffect(() => {
    if (focusIndex.current !== null) {
      inputElements.current[focusIndex.current]?.focus()
      focusIndex.current = null
    }
  }, [value])

  function setText(rowIndex: number, text: string) {
    const next = value.slice()
    next[rowIndex] = { ...next[rowIndex], text_md: text }
    onChange(next)
  }

  function setCorrect(rowIndex: number) {
    onChange(value.map((choice, index) => ({ ...choice, is_correct: index === rowIndex })))
  }

  function addRow() {
    focusIndex.current = value.length
    onChange([...value, emptyChoice()])
  }

  function removeRow(rowIndex: number) {
    if (value.length <= MIN_ROWS) return
    const next = value.filter((_, index) => index !== rowIndex)
    onChange(next)
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>, rowIndex: number) {
    if (event.key !== 'Enter') return
    if (event.metaKey || event.ctrlKey) return
    event.preventDefault()
    if (rowIndex === value.length - 1) {
      addRow()
    } else {
      inputElements.current[rowIndex + 1]?.focus()
    }
  }

  const orphanedErrors = Object.entries(errors)
    .filter(([fieldName]) => {
      const match = /^choices\[(\d+)\]\./.exec(fieldName)
      return match !== null && Number(match[1]) >= value.length
    })
    .map(([, message]) => message)

  return (
    <div className="space-y-2">
      {errors.choices && <p className="text-sm text-destructive">{errors.choices}</p>}
      {orphanedErrors.map((message, index) => (
        <p key={index} className="text-sm text-destructive">{message}</p>
      ))}
      <RadioGroup
        value={String(value.findIndex((choice) => choice.is_correct))}
        onValueChange={(selectedValue) => setCorrect(Number(selectedValue))}
        className="gap-2"
      >
        {value.map((choice, rowIndex) => {
          const fieldError = errors[`choices[${rowIndex}].text_md`]
          return (
            <div key={rowIndex} className="space-y-1">
              <div className="flex items-center gap-2">
                <RadioGroupItem
                  value={String(rowIndex)}
                  id={`choice-correct-${rowIndex}`}
                  aria-label={`Mark choice ${rowIndex + 1} as correct`}
                />
                <Input
                  ref={(element) => { inputElements.current[rowIndex] = element }}
                  value={choice.text_md}
                  onChange={(event) => setText(rowIndex, event.target.value)}
                  onKeyDown={(event) => handleKeyDown(event, rowIndex)}
                  aria-invalid={!!fieldError}
                  placeholder={`Choice ${rowIndex + 1}`}
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove choice ${rowIndex + 1}`}
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
        Add choice
      </Button>
    </div>
  )
}
