---
name: review-changes
description: Reviews HYPERION changes against the project's own written standards. It routes the diff to specialist reviewer agents in parallel (Rust rules, TypeScript rules, simulation determinism, the console UX guidelines, action-plan conformance, physical accuracy), then verifies and merges their findings. Use after implementing a plan task or any non-trivial change, before committing, or when the user asks for a review against the rules, the UX guide, the plan or the brainstorm.
argument-hint: "[task-id] [git-ref-or-range]"
---

# Reviewing changes against HYPERION's standards

Each reviewer checks one written standard and cites it. This complements the generic
`/code-review`, which hunts for bugs, and does not replace it.

## Changes at invocation

```!
git status --short
git diff --stat HEAD
```

## 1. Fix the scope

Arguments: `$ARGUMENTS`.

- A token like `P02.T5.a` is the plan task under review. If there is none, take the task from the
  conversation or the latest commit subject. If there is still none, skip the plan-conformance
  reviewer.
- Any other token is a git ref or range. The default is uncommitted work against `HEAD`,
  **untracked files included**. `git diff` omits untracked files, so list them with
  `git ls-files --others --exclude-standard`, and pass new files to reviewers by name.
- Leave out files under `.claude/` unless they are the subject of the review.
- The tree can move during a review, for example when another session commits. If reviewers
  report that their files have no diff, let them review the commit that now holds the work.

## 2. Route

Launch every applicable reviewer in **one message** so that they run in parallel. Give each one
the scope (ref or range, plus the explicit list of files in its area, untracked included), the
task ID and plan path if any, and one sentence on what the change is meant to do. Send each
reviewer only the files in its area, so that none of them wades through the whole diff.

| Reviewer                    | Route when the change touches                                                                                                                 |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust-reviewer`             | `*.rs`, `Cargo.toml`                                                                                                                          |
| `determinism-auditor`       | `crates/hyperion-sim/`, `crates/hyperion-testkit/`, golden files, generated tables: anything that could move generated output                 |
| `typescript-reviewer`       | `*.ts`, `*.tsx`, `package.json`, `tsconfig*.json`                                                                                             |
| `ux-reviewer`               | `apps/hyperion/src/renderer/` (components, CSS, displayed strings, formatting, canvas drawing) or `docs/frontend/ux-guidelines.md`            |
| `plan-conformance-reviewer` | a plan task is in scope                                                                                                                       |
| `science-checker`           | new or changed physical constants, formulas, units, astrophysical models, citations, fitted tables, or unit conversions in the UI              |

The science checker is the costly one, because it looks sources up on the web. Route it only
when the diff contains physics: numeric literals with physical meaning, formulas, doc comments
citing papers, `tables/`, `consts.rs`, or unit conversions. Tell it which items to check.

## 3. Verify and merge

Reviewers see only part of the context, and they can be wrong. For each must-fix and should-fix
finding, open the cited location and confirm two things: the code does what the finding says, and
the cited rule says what the finding claims. Drop findings that fail either check. Merge
duplicates across reviewers and keep the most specific citation. Keep a "consider" item only if it
is cheap and clearly worth doing.

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

### Deviations to record in the plan
- <drafted "as built" bullets from the plan-conformance reviewer>

### Not covered
- <areas no reviewer covers, e.g. CI workflow edits; visual checks not run>
```

When `implement-task` invoked this skill, go on to fix the findings. When the user invoked it
directly, report, and offer to fix, unless they already asked you to.
