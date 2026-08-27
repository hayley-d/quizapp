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
