# Code rules

These rules are mandatory for all code written or edited in this repository.

## 1. Never write comments

Do not add comments to code. No inline comments, no block comments, no doc
comments, no `TODO`/`FIXME` notes, no section-divider banners, no commented-out
code. Code must explain itself through naming and structure.

If something seems to need a comment to be understood, that is a signal to
rename or restructure it instead.

Existing comments encountered while editing may be left alone, but do not add
new ones.

This does not apply to prose files (Markdown docs, README, plans) or to
configuration files where a comment is the only way to convey required
information.

## 2. Never abbreviate names

Every identifier — variables, functions, methods, types, struct fields, modules,
parameters, CSS classes, database columns, test names — uses full, unabbreviated
English words, so the code reads without needing anything explained.

Not allowed:

- Shortened words: `btn`, `cfg`, `req`, `res`, `msg`, `err`, `idx`, `qty`, `attr`, `elem`, `val`, `num`, `str`, `db`, `repo`, `ctx`, `tmp`, `calc`, `init`
- Single letters: `i`, `j`, `e`, `x`, `n`, `q`, `s`
- Dropped vowels: `qstn`, `ansr`, `crnt`
- Invented shorthand of any kind

Use instead:

| Instead of | Write |
| --- | --- |
| `btn` | `button` |
| `req`, `res` | `request`, `response` |
| `err` | `error` |
| `i` in a loop | `questionIndex`, `cardIndex` |
| `e` in a handler | `event`, `clickEvent` |
| `db` | `database` |
| `ctx` | `context` |
| `qty` | `quantity` |
| `cfg` | `configuration` |

Long names are preferred over short ones. `currentQuestionIndex` beats `idx`;
`selectedAnswerIdentifier` beats `ansId`.

## 3. Never use `any` in TypeScript

`any` switches off the type checker exactly where a type is hardest to get right, and it
spreads: every value derived from an `any` is unchecked too. It is never the answer.

Not allowed:

- Bare `any`, `any[]`, `Array<any>`, `Promise<any>`, `Record<string, any>`
- `as any`, or `as unknown as T` used to launder a cast
- `any` as a generic argument or a type-parameter default
- `// @ts-ignore` or `// @ts-expect-error` used to hide a typing problem

Use instead:

| Instead of | Write |
| --- | --- |
| `catch (error: any)` | `catch (error: unknown)`, then narrow |
| `as any` to silence a cast | the real type, or a type guard |
| `any` for JSON of unknown shape | `unknown`, narrowed at the boundary |
| `Record<string, any>` | `Record<string, unknown>`, or a declared shape |
| `any` for a callback you do not want to type | the actual signature |

`unknown` is the correct escape hatch. It is honest about what is not known and forces a
narrowing step before use. The existing error path already reads this way:
`catch (error: unknown)` followed by `error instanceof ApiError`.

If a type is genuinely hard to express, that is a signal to name the shape in
`frontend/src/lib/api.ts` alongside the other response types, not to reach for `any`.

This applies to `frontend/src/`, but not to `frontend/src/components/ui/`, which is
shadcn-generated and treated as vendored third-party code (see Accepted short forms).

## 4. Never co-author commits

Do not add `Co-Authored-By:` trailers to commit messages. No co-author lines for
Claude, for any agent, or for any tool. This applies to every commit in this
repository, including those made by subagents.

Commit messages end with their body. Nothing is appended after it.

## Accepted short forms

These are settled and must not be "fixed" by a later pass:

- `App` as a type prefix: `AppError`, `AppState`, `AppJson`, `AppResult`.
- `md` for markdown, as a field and column suffix: `prompt_md`, `answer_md`,
  `text_md`, `explanation_md`.
- `ms` for milliseconds, `idx_` as the SQL index-name prefix.
- Universal domain terms: `id`, `url`, `http`, `json`, `sql`, `css`, `html`,
  `api`, `uuid`.
- Names imposed by an external library, framework, or trait signature that
  cannot be renamed.
- `frontend/src/components/ui/` is shadcn-generated and treated as vendored
  third-party code: leave it alone, including its `cn` helper.

## Stylesheets

`src/styles/globals.css` keeps the inline colour annotations next to its theme
tokens (`--primary: oklch(0.62 0.13 195); /* turquoise */`). An `oklch()` triple
and a semantic token name cannot between them tell a reader the colour is
turquoise, so this is the "a comment is the only way to convey it" case. Prose
and rationale comments in CSS follow rule 1 and go.

The test is whether the name tells a reader what the thing is. `md` does.
A single letter like `q` for a search parameter does not, so it is spelled
`search`.
