---
name: review-changes
description: Reviews HYPERION changes against the project's own written standards. It routes the diff to specialist reviewer agents in parallel (Rust rules, TypeScript rules, simulation determinism, the console UX guidelines, action-plan conformance, physical accuracy), then verifies and merges their findings. Use after implementing a plan task or any non-trivial change, before committing, or when the user asks for a review against the rules, the UX guide, the plan or the brainstorm.
argument-hint: "[task-id] [git-ref-or-range] [--feature <plan-set>]"
allowed-tools:
  - Bash(git -C ${CLAUDE_PROJECT_DIR} status *)
  - Bash(git -C ${CLAUDE_PROJECT_DIR} diff *)
---

# Reviewing changes against HYPERION's standards

Each reviewer checks one written standard and cites it. This complements the generic
`/code-review`, which hunts for bugs, and does not replace it.

## Changes at invocation

```!
git -C ${CLAUDE_PROJECT_DIR} status --short
git -C ${CLAUDE_PROJECT_DIR} diff --stat HEAD
```

## 1. Fix the scope

Arguments: `$ARGUMENTS`.

- A token like `P02.T5.a` is the plan task under review, and `--feature <plan-set>` names its plan
  set when plan numbers repeat across sets. If there is no task ID, take the task from the
  conversation, or, when the scope is a commit, from that commit's subject. If there is still none,
  skip the plan-conformance reviewer.
- Any other token is a git ref or range. The default is uncommitted work against `HEAD`,
  **untracked files included**. `git diff` omits untracked files, so list them with
  `git ls-files --others --exclude-standard`, and pass new files to reviewers by name, marked as
  untracked.
- If the section above is empty (a clean tree) and no ref was given, review the last commit
  (`HEAD~1..HEAD`) and say so.
- Leave out files under `.claude/` unless they are the subject of the review.
- The tree can move during a review, for example when another session commits. If reviewers
  report that their files have no diff, let them review the commit that now holds the work.

## 2. Route

Launch every applicable reviewer in **one message** so that they run in parallel. Give each one
the scope (the ref or range, plus the explicit list of files in its area, untracked files marked),
the task ID, plan path and plan set if any, and one sentence on what the change is meant to do.
Send each reviewer only the files in its area, so that none of them wades through the whole diff.
The plan-conformance reviewer is the exception: it gets every file, because it checks for what is
missing. For a committed range, tell the determinism auditor its first and last commits.

Route to each reviewer whose area the change touches:

- `rust-reviewer`: `*.rs`, `Cargo.toml`, `Cargo.lock`, `clippy.toml`, `rustfmt.toml`.
- `determinism-auditor`: anything that could move generated output. That is
  `crates/hyperion-sim/`, `crates/hyperion-testkit/`, `crates/hyperion-fit/`, golden files, and the
  root `Cargo.toml` or `Cargo.lock` (the `libm` pin).
- `typescript-reviewer`: `*.ts`, `*.tsx`, `*.mts`, `*.cts`, `package.json`, `tsconfig*.json`,
  `.oxlintrc.json`, and `apps/hyperion/src/renderer/index.html` (the CSP). For regenerated bindings
  in `packages/protocol/src/generated/`, also pass the `crates/hyperion-protocol/` files that
  explain them.
- `ux-reviewer`: `apps/hyperion/src/renderer/` (components, CSS, displayed strings, formatting,
  canvas drawing) or `docs/frontend/ux-guidelines.md`.
- `plan-conformance-reviewer`: a plan task is in scope.
- `science-checker`: new or changed physical constants, formulas, units, astrophysical models,
  citations, fitted tables, or unit conversions in the UI.

The science checker is the costly one, because it looks sources up on the web. Route it only
when the diff contains physics: numeric literals with physical meaning, formulas, doc comments
citing papers, fitted tables, constants, or unit conversions. Tell it which items to check, and
leave out items it already verified earlier in this session.

## 3. Verify and merge

Reviewers see only part of the context, and they can be wrong. For each must-fix and should-fix
finding, open the cited location and confirm two things: the code does what the finding says, and
the cited rule says what the finding claims. Drop findings that fail either check. For a science
finding, whose rule is a paper, confirm the location and redo the arithmetic (`python3 -c`), but
keep the cited source unless it is implausible, and mark the finding "source not re-checked".

Merge duplicates across reviewers and keep the most specific citation. When reviewers give the
same issue different severities, use the one set by the standard that owns the rule (a missing
citation is should-fix, under the roadmap's figures rule). When a fix would contradict the
brainstorm or a plan figure, such as a corrected physical value, it goes under "Needs the owner's
ruling", not under the fixes. Keep a "consider" item only if it is cheap and clearly worth doing.

## 4. Report

Use this shape and omit empty sections. "No findings" is a fine result.

```
## Review of <scope> (<reviewers run>)

### Must fix
1. `path:line`: <problem>. <doc § section>. Fix: <change>. (<reviewer>)

### Should fix
...

### Consider
...

### Needs the owner's ruling
- <specification questions: a figure the source contradicts, a plan bracket that looks wrong>

### Deviations to record in the plan
- <drafted "as built" entries from the plan-conformance reviewer>

### State
- Task checklist: <n done, n missing, n different> (plan-conformance reviewer; name what is missing)
- Golden and version state: <one line from the determinism auditor>
- Physics: <n claims checked, n ok> (science checker; all `ok` means no findings)

### Not covered
- <areas no reviewer covers, e.g. CI workflow edits; visual checks not run>
```

When `implement-task` invoked this skill, go on to fix the findings. When the user invoked it
directly, report, and offer to fix, unless they already asked you to.
