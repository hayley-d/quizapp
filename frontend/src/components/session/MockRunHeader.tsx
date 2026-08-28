import { MockTimer } from '@/components/session/MockTimer'
import { Button } from '@/components/ui/button'

type MockRunHeaderProps = {
  questionNumber: number
  totalQuestions: number
  startedAt: string
  onEndEarly: () => void
  ending: boolean
}

export function MockRunHeader({
  questionNumber,
  totalQuestions,
  startedAt,
  onEndEarly,
  ending,
}: MockRunHeaderProps) {
  const completedFraction =
    totalQuestions === 0 ? 0 : Math.min(1, (questionNumber - 1) / totalQuestions)

  return (
    <div className="space-y-2 rounded-xl border bg-card px-4 py-2.5 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm font-medium">
          Question {questionNumber} of {totalQuestions}
        </p>
        <div className="flex items-center gap-3">
          <MockTimer startedAt={startedAt} />
          <Button variant="ghost" className="h-8 px-3 text-sm" onClick={onEndEarly} disabled={ending}>
            {ending ? 'Ending…' : 'End test early'}
          </Button>
        </div>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary">
        <div
          className="h-full rounded-full bg-brand transition-[width]"
          style={{ width: `${completedFraction * 100}%` }}
        />
      </div>
    </div>
  )
}
