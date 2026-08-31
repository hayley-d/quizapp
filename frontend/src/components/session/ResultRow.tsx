import { Check, X } from 'lucide-react'

import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { KIND_LABEL, type ResultQuestion } from '@/lib/api'
import { cn } from '@/lib/utils'

type ResultRowProps = {
  question: ResultQuestion
  position: number
  onOverride: (reviewId: number) => void
  overriding: boolean
}

function verdictLabel(question: ResultQuestion): string {
  if (question.overridden) return 'Counted as correct'
  return question.correct ? 'Correct' : 'Not quite'
}

export function ResultRow({ question, position, onOverride, overriding }: ResultRowProps) {
  const VerdictIcon = question.correct ? Check : X

  return (
    <li
      className={cn(
        'space-y-3 rounded-xl border border-l-4 bg-card p-4 shadow-sm',
        question.correct ? 'border-l-success' : 'border-l-destructive',
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm font-medium text-muted-foreground tabular-nums">
          {position}.
        </span>
        <span
          className={cn(
            'inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-semibold',
            question.correct
              ? 'bg-success text-success-foreground'
              : 'bg-destructive text-destructive-foreground',
          )}
        >
          <VerdictIcon className="size-3.5" aria-hidden="true" />
          {verdictLabel(question)}
        </span>
        <Badge variant="secondary">{KIND_LABEL[question.kind]}</Badge>
      </div>

      <Markdown>{question.prompt_md}</Markdown>
      {question.image_path !== null && (
        <CardImage path={question.image_path} altText="Card image" />
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-1">
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Your answer
          </p>
          {question.given === null || question.given.trim() === '' ? (
            <p className="text-sm text-muted-foreground">
              {question.self_grade === null ? 'Nothing typed' : `Self-graded ${question.self_grade}`}
            </p>
          ) : (
            <p className="text-sm whitespace-pre-wrap">{question.given}</p>
          )}
        </div>
        <div className="space-y-1">
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {question.expected.length > 1 ? 'The answer' : 'The answer was'}
          </p>
          {question.expected.length === 0 ? (
            <p className="text-sm text-muted-foreground">No answer recorded</p>
          ) : (
            <div className="space-y-1">
              <Markdown className="text-sm">{question.expected[0]}</Markdown>
              {question.expected.length > 1 && (
                <p className="text-xs text-muted-foreground">
                  also accepted: {question.expected.slice(1).join(', ')}
                </p>
              )}
            </div>
          )}
        </div>
      </div>

      {question.explanation_md !== null && (
        <Markdown className="border-t pt-3 text-sm text-muted-foreground">
          {question.explanation_md}
        </Markdown>
      )}

      {question.can_override && !question.overridden && (
        <Button
          variant="secondary"
          className="h-8 px-3 text-sm"
          disabled={overriding}
          onClick={() => onOverride(question.review_id)}
        >
          {overriding ? 'Counting…' : 'I was right'}
        </Button>
      )}
    </li>
  )
}
