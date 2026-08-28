import { useEffect, useState } from 'react'

import { formatClock } from '@/lib/format'

type MockTimerProps = {
  startedAt: string
}

export function MockTimer({ startedAt }: MockTimerProps) {
  const startedAtMilliseconds = Date.parse(startedAt)
  const [elapsedMilliseconds, setElapsedMilliseconds] = useState(() =>
    Math.max(0, Date.now() - startedAtMilliseconds),
  )

  useEffect(() => {
    const interval = setInterval(
      () => setElapsedMilliseconds(Math.max(0, Date.now() - startedAtMilliseconds)),
      1000,
    )
    return () => clearInterval(interval)
  }, [startedAtMilliseconds])

  return (
    <span role="timer" aria-hidden="true" className="font-display tabular-nums">
      {formatClock(elapsedMilliseconds)}
    </span>
  )
}
