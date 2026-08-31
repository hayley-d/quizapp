import type { Card } from '@/lib/api'
import { Markdown } from '@/components/Markdown'

const MISSING_ANSWER = '—'

function correctChoiceText(card: Card): string {
  const correctChoice = card.choices.find((choice) => choice.is_correct)
  return correctChoice?.text_md ?? MISSING_ANSWER
}

function primaryAcceptedText(card: Card): string {
  const primaryAccepted = card.accepted.find((accepted) => accepted.is_primary)
  return primaryAccepted?.text ?? card.accepted[0]?.text ?? MISSING_ANSWER
}

export function FlashcardAnswer({ card }: { card: Card }) {
  if (card.kind === 'text_answer') {
    return <p className="text-center text-2xl font-medium">{primaryAcceptedText(card)}</p>
  }

  const answerMarkdown =
    card.kind === 'mc_single' ? correctChoiceText(card) : (card.answer_md ?? MISSING_ANSWER)

  return <Markdown className="text-center text-2xl">{answerMarkdown}</Markdown>
}
