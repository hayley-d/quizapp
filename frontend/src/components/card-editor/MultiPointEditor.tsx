import { useEffect, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Markdown } from '@/components/Markdown'
import { cn } from '@/lib/utils'
import {
  api,
  MULTI_POINT_MODE_LABEL,
  type AnswerPointsPreview,
  type MultiPointMode,
} from '@/lib/api'

const MODES: MultiPointMode[] = ['auto', 'on', 'off']

const MODE_HINT: Record<MultiPointMode, string> = {
  auto: 'Score as a list when the answer is written as one.',
  on: 'Always score as a list, even without bullets or numbers.',
  off: 'Never score as a list, however the answer is written.',
}

type MultiPointEditorProps = {
  source: string
  value: MultiPointMode
  onChange: (mode: MultiPointMode) => void
  error?: string
}

export function MultiPointEditor({ source, value, onChange, error }: MultiPointEditorProps) {
  const [preview, setPreview] = useState<AnswerPointsPreview | null>(null)

  useEffect(() => {
    if (source.trim() === '') {
      setPreview(null)
      return
    }
    const controller = new AbortController()
    const timer = window.setTimeout(() => {
      void api
        .previewAnswerPoints(source, value, controller.signal)
        .then(setPreview)
        .catch(() => undefined)
    }, 250)
    return () => {
      window.clearTimeout(timer)
      controller.abort()
    }
  }, [source, value])

  const splittable = source.includes('\n')
  if (!splittable && value === 'auto') return null

  return (
    <div className="space-y-3 rounded-lg border bg-muted/40 p-3">
      <div className="space-y-1">
        <p className="text-sm font-semibold">Point by point</p>
        <p className="text-sm text-muted-foreground">
          A list answer is recalled against a checklist and scored out of its points.
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        {MODES.map((mode) => (
          <Button
            key={mode}
            type="button"
            size="sm"
            variant={mode === value ? 'brand' : 'secondary'}
            aria-pressed={mode === value}
            onClick={() => onChange(mode)}
          >
            {MULTI_POINT_MODE_LABEL[mode]}
          </Button>
        ))}
      </div>

      <p className="text-sm text-muted-foreground">{MODE_HINT[value]}</p>

      {preview !== null &&
        (preview.multi_point ? (
          <div className="space-y-2">
            <p className="text-sm font-semibold text-muted-foreground">
              Scored as {preview.points.length} points
            </p>
            <ol className="space-y-1">
              {preview.points.map((point, pointIndex) => (
                <li key={point.key} className="flex items-start gap-2 text-sm">
                  <span className="mt-0.5 font-mono text-xs text-muted-foreground">
                    {pointIndex + 1}
                  </span>
                  <Markdown className="min-w-0 flex-1">{point.text}</Markdown>
                </li>
              ))}
            </ol>
            {preview.notes.length > 0 && (
              <div className="space-y-1 border-t pt-2">
                <p className="text-sm font-semibold text-muted-foreground">
                  Shown but not scored
                </p>
                {preview.notes.map((note) => (
                  <Markdown key={note} className="text-sm text-muted-foreground">
                    {note}
                  </Markdown>
                ))}
              </div>
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            Scored as one whole answer.
          </p>
        ))}

      {error && <p className={cn('text-sm text-destructive')}>{error}</p>}
    </div>
  )
}
