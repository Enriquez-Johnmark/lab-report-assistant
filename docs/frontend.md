# Frontend

Status: `web/` created in M3 (2026-11-10) as a copy of Balanse's web shell, which follows Kompleto's `docs/frontend.md`; its principles, stack, folder layout and conventions apply here unchanged, with these differences:

- **Rust decides; React shows.** Who may approve, what blocks a draft and how many new drafts are left are the API's; the pages only show what it returns and ask it to act.
- **Values come from the lab data.** The results table shows the API's `results`, never text from a draft.
- **Dates** are shown in the viewer's own time zone.
- **shadcn command line:** `pnpm dlx shadcn@4.22.1 … -c web`, the same pinned version as Kompleto.
- **Browser tests** (`pnpm e2e`) run against a server with one clinic, its administrator and a physician; CI sets them up.

## The look (M7, 2026-11-11)

A lab's printed report sheet, read under clinic light: calm, ruled and exact. Its values live in `web/src/styles/globals.css`; components never set colours of their own. The patient's page (`src/web/patient.rs`) and the PDF follow the same rules.

- **One accent:** a deep clinical teal, spent only on the primary action and the current place (the navigation's underline, the dot of "Ready to review"). Never on every row.
- **One grey family:** warm greys, for the paper ground, the ink, rules and quiet text. Light only; there is no dark theme.
- **Meaning colours:** amber only for a value outside the lab's range; red for what failed (a blocked draft, a notice not sent); green for a notice sent or opened. Each always comes with words, never colour alone.
- **Type:** IBM Plex Sans (OFL, bundled from `@fontsource-variable/ibm-plex-sans`). Table headings are small, spaced capitals, as on a lab report. Every value uses tabular figures.
- **One corner rule:** controls 6px, panels 8px, status pills fully round. The larger radius steps collapse onto the panel radius.
- **Hierarchy by space and rules, not boxes:**
  - a page has one white sheet for its main work (the queue, a report), with its actions in the sheet's footer;
  - everything else is a plain section under a rule (`components/section.tsx`);
  - no card inside a card.
- **Status in words:** a small dot plus words (`components/status.tsx`), not a coloured pill on every row.
- **The frame:** a letterhead across the top (the product, then the clinic and who is logged in) with the places beneath it. No sidebar.

Before writing or reviewing React, read the skills in `.claude/skills/`:

| Skill | Use it for |
|---|---|
| `vercel-react-best-practices` | Performance: waterfalls, bundle size, re-renders. Read `rules/*.md`, not the compiled `AGENTS.md`, which is garbled in places upstream. These don't apply to a Vite app: every `server-*` rule, `async-api-routes`, `async-suspense-boundaries`, `bundle-dynamic-imports` (use `React.lazy`), `bundle-defer-third-party` and the `rendering-hydration-*` and `rendering-script-defer-async` rules. Where a rule uses SWR, read it as TanStack Query |
| `vercel-composition-patterns` | Component design: compound components, explicit variants, no boolean-prop piles, React 19 APIs. Its examples use React Native components; the patterns are the same on the web |
| `shadcn` | Adding, composing and styling shadcn/ui components. Skip its chat, registry, MCP and design-system files |
| `frontend-design` | Visual direction: palette, type and layout chosen for the subject, not templated defaults; plain, specific copy. A physician's review screen is calm and clear before it is distinctive |
| `web-design-guidelines` | Reviewing a page: accessibility, focus, forms, motion, typography and content rules, from its pinned `guidelines.md`. Its "Title Case for headings/buttons" rule doesn't apply: headings and buttons are in sentence case |
| `ui-taste` | Audits and polish passes: read `reference/audit.md` (look only), then `reference/polish.md`, `reference/craft.md`, `reference/operate.md`, `reference/distill.md` and the web parts of `reference/anti-slop-craft.md`. Polish keeps this product's look; it never smuggles in a redesign. Skip the iOS, native and mobile playbooks, the paid Uizze MCP (`find_ui_references`, `find_ui_materials`) and the image-first step (no image tool here: work from a screenshot of the current screen). Its `visual-evidence.json` receipts stay out of the repository |

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
| `tdd`, `grilling`, `grill-me`, `domain-modeling` | github.com/mattpocock/skills, `skills/engineering/tdd`, `skills/productivity/grilling`, `skills/productivity/grill-me`, `skills/engineering/domain-modeling` | `4ae2317` (2026-10-31) | MIT, Copyright (c) 2026 Matt Pocock; the licence is copied into each folder as `LICENSE` |
| `ui-taste` | uizze.sh (`https://uizze.sh/.well-known/skills/ui-taste/`), version 0.3.0 (`uizze-design-v14`), every file matching its `CHECKSUMS.sha256`; added 2026-11-10 | not published as a git commit; pinned by the folder hash in `skills-lock.json` | Apache-2.0 (`LICENSE`), adapted from Impeccable (Apache-2.0) and Appllama's skills (MIT); the notices are in `NOTICE` and `MODIFICATIONS.md` |

**`ui-taste` (added 2026-11-10)** is an unchanged copy, read in full first. It is Markdown guidance plus one optional script, `scripts/verify-visual-evidence.mjs`, which only reads local files to check a screenshot receipt. The upstream install command (`npx skills add https://uizze.sh/`) was not used, since it installs whatever is newest, unread; the files were downloaded from the same address and checked against the bundle's checksums.

**Local changes:**

- `shadcn/SKILL.md` (made in Kompleto): `allowed-tools` removed, which pre-approved every `shadcn` command with any arguments; the line that ran `npx shadcn@latest info` unpinned on every load removed. Run a pinned `pnpm dlx shadcn@<version> … -c web` by hand; the version is pinned here once `web/` exists.
- `web-design-guidelines/SKILL.md`: written locally. Upstream it downloads its rules from the guidelines repository's `main` branch each time it is used, so they could change unread; here it reads the pinned copy, `guidelines.md`.

**The workflow skills (added 2026-11-10)** are unchanged copies, read in full first; they hold guidance only and run nothing. The upstream install command (`npx skills@latest add mattpocock/skills`) was not used, since it installs whatever is newest, unread. How they apply here:

- `tdd`: its "seams" are this project's acceptance criteria, already agreed with the owner in `docs/product/spec.md`, so they aren't asked again. It refers to two skills not added here: `codebase-design` (optional vocabulary) and its own `code-review` (this session's built-in `code-review` skill is used instead).
- `grilling` (and `grill-me`, which only calls it): numbered rounds with a recommended answer for every owner decision.
- `domain-modeling`: `GLOSSARY.md` and `docs/adr/`, created when the first term or decision needs them.
