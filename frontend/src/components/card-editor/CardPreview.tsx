import { Check, Star } from 'lucide-react'
import type { AcceptedInput, CardKind, ChoiceInput } from '@/lib/api'
import { Markdown } from '@/components/Markdown'
import { CardImage } from '@/components/CardImage'

type Props = {
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

/**
 * The card as it will read once saved. Rendered from the form's live state
 * rather than from a fetch, so it shows unsaved edits — that is the whole
 * point of checking a formula before committing to it.
 *
 * Everything here goes through <Markdown>, the app's single renderer, so the
 * preview cannot disagree with the card list or (in Part 3) the session
 * runner about how a card looks.
 */
export function CardPreview({
  kind, promptMd, imagePath, choices, accepted, answerMd, explanationMd,
}: Props) {
  return (
    <div className="space-y-5 rounded-lg border p-4">
      <Section title="Prompt">
        {imagePath !== null && <CardImage path={imagePath} alt="Card image" />}
        {promptMd.trim() === ''
          ? <p className="text-sm italic text-muted-foreground">No prompt yet.</p>
          : <Markdown>{promptMd}</Markdown>}
      </Section>

      {kind === 'mc_single' && (
        <Section title="Choices">
          <ul className="space-y-1">
            {choices.map((c, i) => (
              <li key={i} className="flex items-start gap-2">
                {c.is_correct ? (
                  <Check className="mt-1 size-4 shrink-0 text-primary" aria-label="Correct" />
                ) : (
                  <span className="mt-1 size-4 shrink-0" aria-hidden="true" />
                )}
                <Markdown className="min-w-0 flex-1">{c.text_md}</Markdown>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {kind === 'short_answer' && (
        <Section title="Accepted answers">
          <ul className="space-y-1">
            {accepted.map((a, i) => (
              <li key={i} className="flex items-start gap-2">
                {a.is_primary ? (
                  <Star className="mt-1 size-4 shrink-0 text-primary" aria-label="Primary" />
                ) : (
                  <span className="mt-1 size-4 shrink-0" aria-hidden="true" />
                )}
                {/* Accepted answers are compared as plain text, but they are
                    shown back to the student as the expected answer, so they
                    render like everything else. */}
                <Markdown className="min-w-0 flex-1">{a.text}</Markdown>
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
