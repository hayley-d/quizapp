import { Link } from 'react-router-dom'

import { Button } from '@/components/ui/button'

type SessionExhaustedProps = {
  message: string
}

export function SessionExhausted({ message }: SessionExhaustedProps) {
  return (
    <div className="max-w-xl space-y-4">
      <h1 className="font-display text-2xl font-bold">Nothing left to practise</h1>
      <p className="text-muted-foreground">{message}</p>
      <div className="flex gap-3">
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    </div>
  )
}
