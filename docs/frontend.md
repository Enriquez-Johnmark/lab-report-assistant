# Frontend

Status: the skills are in place (2026-11-10). The conventions for `web/` are written when it is created in M3, starting from Kompleto's `docs/frontend.md`.

Before writing or reviewing React, read the skills in `.claude/skills/`:

| Skill | Use it for |
|---|---|
| `vercel-react-best-practices` | Performance: waterfalls, bundle size, re-renders. Read `rules/*.md`, not the compiled `AGENTS.md`, which is garbled in places upstream. These don't apply to a Vite app: every `server-*` rule, `async-api-routes`, `async-suspense-boundaries`, `bundle-dynamic-imports` (use `React.lazy`), `bundle-defer-third-party` and the `rendering-hydration-*` and `rendering-script-defer-async` rules. Where a rule uses SWR, read it as TanStack Query |
| `vercel-composition-patterns` | Component design: compound components, explicit variants, no boolean-prop piles, React 19 APIs. Its examples use React Native components; the patterns are the same on the web |
| `shadcn` | Adding, composing and styling shadcn/ui components. Skip its chat, registry, MCP and design-system files |
| `frontend-design` | Visual direction: palette, type and layout chosen for the subject, not templated defaults; plain, specific copy. A physician's review screen is calm and clear before it is distinctive |
| `web-design-guidelines` | Reviewing a page: accessibility, focus, forms, motion, typography and content rules, from its pinned `guidelines.md`. Its "Title Case for headings/buttons" rule doesn't apply: headings and buttons are in sentence case |

Where a skill and this file disagree, this file wins.

## Where the skills come from

Copied unchanged from Kompleto on 2026-11-10, with the same entries in `skills-lock.json` (the hash covers each folder as installed, local changes included). Each was read in full before it was first added, and they hold guidance only, apart from the changes below.

| Skill | Source | Commit | Licence |
|---|---|---|---|
| `rust-best-practices` | github.com/apollographql/skills, `skills/rust-best-practices` | not recorded in Kompleto | MIT, declared in its `SKILL.md` |
| `vercel-react-best-practices` | github.com/vercel-labs/agent-skills, `skills/react-best-practices` | `063bee9` (2026-08-28) | MIT, declared in its `SKILL.md`; the repository has no licence file or copyright line |
| `vercel-composition-patterns` | the same repository, `skills/composition-patterns` | `063bee9` | MIT, as above |
| `shadcn` | github.com/shadcn-ui/ui, `skills/shadcn` | `25af02b` (2026-10-22) | MIT, Copyright (c) 2023 shadcn; the licence is in the folder as `LICENSE.md` |
| `frontend-design` | github.com/anthropics/skills, `skills/frontend-design` | `2c5958e` (2026-11-06) | Apache-2.0; the licence is in the folder as `LICENSE.txt` |
| `web-design-guidelines` | `guidelines.md` is `command.md` from github.com/vercel-labs/web-interface-guidelines | `434b7f9` (2026-10-05) | MIT, Copyright (c) 2025 Vercel Labs; the licence is in the folder as `LICENSE` |

**Local changes:**

- `shadcn/SKILL.md` (made in Kompleto): `allowed-tools` removed, which pre-approved every `shadcn` command with any arguments; the line that ran `npx shadcn@latest info` unpinned on every load removed. Run a pinned `pnpm dlx shadcn@<version> … -c web` by hand; the version is pinned here once `web/` exists.
- `web-design-guidelines/SKILL.md`: written locally. Upstream it downloads its rules from the guidelines repository's `main` branch each time it is used, so they could change unread; here it reads the pinned copy, `guidelines.md`.
