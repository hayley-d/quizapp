import { Check, Star } from 'lucide-react'
import type { AcceptedInput, CardKind, ChoiceInput } from '@/lib/api'
import { Markdown } from '@/components/Markdown'
import { CardImage } from '@/components/CardImage'

type CardPreviewProps = {
  kind: CardKind
  promptMd: string
  imagePath: string | null
  choices: ChoiceInput[]
  accepted: AcceptedInput[]
  answerMd: string
  explanationMd: string
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-2">
      <h2 className="text-sm font-medium text-muted-foreground">{title}</h2>
      {children}
    </section>
  )
}

export function CardPreview({
  kind, promptMd, imagePath, choices, accepted, answerMd, explanationMd,
}: CardPreviewProps) {
  return (
    <div className="space-y-5 rounded-lg border p-4">
      <Section title="Prompt">
        {imagePath !== null && <CardImage path={imagePath} altText="Card image" />}
        {promptMd.trim() === ''
          ? <p className="text-sm italic text-muted-foreground">No prompt yet.</p>
          : <Markdown>{promptMd}</Markdown>}
      </Section>

      {kind === 'mc_single' && (
        <Section title="Choices">
          <ul className="space-y-1">
            {choices.map((choice, choiceIndex) => (
              <li key={choiceIndex} className="flex items-start gap-2">
                {choice.is_correct ? (
                  <Check className="mt-1 size-4 shrink-0 text-primary" aria-label="Correct" />
                ) : (
                  <span className="mt-1 size-4 shrink-0" aria-hidden="true" />
                )}
                <Markdown className="min-w-0 flex-1">{choice.text_md}</Markdown>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {kind === 'text_answer' && (
        <Section title="Accepted answers">
          <ul className="space-y-1">
            {accepted.map((answer, answerIndex) => (
              <li key={answerIndex} className="flex items-start gap-2">
                {answer.is_primary ? (
                  <Star className="mt-1 size-4 shrink-0 text-primary" aria-label="Primary" />
                ) : (
                  <span className="mt-1 size-4 shrink-0" aria-hidden="true" />
                )}
                <Markdown className="min-w-0 flex-1">{answer.text}</Markdown>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {kind === 'flashcard' && (
        <Section title="Answer">
          <Markdown>{answerMd}</Markdown>
        </Section>
      )}

      {explanationMd.trim() !== '' && (
        <Section title="Explanation">
          <Markdown>{explanationMd}</Markdown>
        </Section>
      )}
    </div>
  )
}
