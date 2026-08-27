import ReactMarkdown from 'react-markdown'
import remarkMath from 'remark-math'
import rehypeKatex from 'rehype-katex'
import { cn } from '@/lib/utils'

// KaTeX ships its own fonts inside the npm package, so importing the
// stylesheet this way keeps maths rendering correctly offline and on a
// LAN-only phone. Do NOT swap this for a CDN <link>: globals.css already
// pulls Quicksand and Inter over the network and that is a known defect
// deferred to build step 8, not a pattern to copy.
import 'katex/dist/katex.min.css'

type Props = {
  /** Markdown with inline LaTeX, as written in Obsidian: `$X, Y, Z$`, `$10\ 000$`. */
  children: string
  className?: string
}

/**
 * The app's only markdown renderer.
 *
 * The card list, the card editor's preview and (in Part 3) the session runner
 * all render through here, so what a card looks like while it is written and
 * what it looks like while it is answered cannot drift apart. Part 2a
 * deliberately shipped raw text everywhere so this would be built exactly
 * once — if you need different behaviour somewhere, add a prop here rather
 * than a second renderer.
 *
 * Raw HTML stays disabled. That is react-markdown's default and there is no
 * `rehype-raw`; card text is markdown, not a template.
 *
 * GitHub-flavoured extras (tables, strikethrough, autolinks) need `remark-gfm`
 * and are deliberately not installed. Add it when a real card needs one.
 */
export function Markdown({ children, className }: Props) {
  return (
    <div className={cn('markdown', className)}>
      <ReactMarkdown remarkPlugins={[remarkMath]} rehypePlugins={[rehypeKatex]}>
        {children}
      </ReactMarkdown>
    </div>
  )
}
