---
name: implement-task
description: Implements one task or subtask of a HYPERION action plan (IDs like P02.T5 or P05.T1.a, plans under docs/agent/plans/) end to end. It gathers the task, the design notes it cites and the brainstorm sections it touches, builds the task under the project rules, validates, reviews, and records deviations in the plan. Use whenever the user asks to implement, start, continue or finish a plan task, a milestone step or "the next task", even if they give only the ID.
argument-hint: "<task-id> [--feature <plan-set>]"
---

# Implementing an action-plan task

Action plans live in `docs/agent/plans/<feature>/NN-*.md`. Each plan set has a roadmap
`README.md` with its conventions, and names a brainstorm that is the specification. A plan says
how to build, never what: where plan and brainstorm disagree, the brainstorm wins until the owner
(the user, who rules on the specification) revises it.

Task: `$ARGUMENTS`. If no ID was given, run `--list` (below), find the first task whose
prerequisites exist in the code and whose own Provides do not, and confirm the choice with the
user before starting.

Copy this checklist into your reply and keep it current:

```
Task <id>:
- [ ] 1. Load the task
- [ ] 2. Check prerequisites
- [ ] 3. Settle open questions
- [ ] 4. Build with tests
- [ ] 5. Validate
- [ ] 6. Review
- [ ] 7. Record as built
- [ ] 8. Report
```

## 1. Load the task

Plans are long. Run the extractor from the repository root instead of paging through them. It
prints the plan header, the ordering notes, the task (with its phase intro, and for a subtask the
parent intro and the paragraphs shared by all subtasks), the design notes the task cites, and the
other lines in the plan set that mention the task. `--context` adds the plan's Generator version
and Risks sections. It works from headings and labels, so if a section looks cut short, read the
plan around the lines it names:

```bash
python3 .claude/skills/implement-task/scripts/plan_task.py P02.T5.a --context     # the task in context
python3 .claude/skills/implement-task/scripts/plan_task.py P02.T5.a --acceptance  # acceptance criteria and commands
python3 .claude/skills/implement-task/scripts/plan_task.py --list P02             # IDs and titles; [commit] if a subject names one
```

When more than one plan set exists, plan numbers repeat: add `--feature <plan-set>` to every call,
and pass it on to `validate` and `review-changes`.

Then read:

- The plan set's roadmap `README.md`, once per session: code shape, generator version, tests,
  figures.
- The plan's **Provides** and **Consumes** entries for what this task builds or uses. Later plans
  grep for the names in Provides.
- The brainstorm sections that the plan header lists and this task touches, and no others. Find
  them with `grep -n '^#' <brainstorm>` rather than reading the whole brainstorm.
- The plan's **Risks and open points** (printed by `--context`), especially what earlier tasks
  recorded as built. It says where the code differs from the plan text.
- The "Mentioned elsewhere" lines. Later tasks and plans that check or consume this task's output
  tell you what it must not break.

## 2. Check prerequisites

The ordering notes say which tasks come first. Confirm that each prerequisite exists **in the
code**: grep for the types and functions its Provides names. A commit message is not proof. If a
prerequisite is missing, stop and name the task it belongs to. Don't build a stand-in, because
stand-ins outlive their tasks.

Run `git status`. If there are unrelated uncommitted changes, mention them and keep your edits
apart from them.

If the task is its plan's reconcile task (one that adjusts the plan's Consumes to the code as
built, such as galaxy-generation's P03.T1), or the first task of a plan the roadmap says must be
re-validated when its turn comes and whose Risks section has no "Re-validated at" record, run the
`revalidate-plan` skill first. Then carry on here with the corrected plan.

## 3. Settle open questions before writing code

Unclear points come in two kinds, and they are handled differently:

- **Specification and UX rulings** are the owner's call. These cover what the game simulates or
  shows, brainstorm content, UX guide additions, and anything a plan marks "needs the owner's
  confirmation". Ask the user. The exception is a plan that says how to proceed without waiting
  (for example "put those edits in a commit of their own"): follow it, and flag it in the report.
- **Technical trade-offs** are yours to decide and record: algorithms, data structures,
  performance against simplicity, how to meet an acceptance criterion the plan got slightly wrong.
  Put the reasoning where the next implementer will see it, in a doc comment or in step 7.

The brainstorm's numbers are rounded. When a figure becomes code, re-check it against the source
the brainstorm cites and put the citation in the doc comment. For more than a couple of figures,
hand the list to the `science-checker` agent. Where the source disagrees with the brainstorm beyond
rounding, that is a specification question: give the owner both values rather than silently
choosing one.

## 4. Build with tests

`.claude/rules/rust-dev.md` and `.claude/rules/typescript-dev.md` bind every task. They load with
matching files, and they override habits from other codebases. In addition:

- Build the files, names and tests the task lists. Keep the names in Provides. If one must change,
  record why in step 7.
- **Simulation code** (`crates/hyperion-sim`, `crates/hyperion-testkit`): load the
  `sim-determinism` skill before writing generators, random draws, or anything a golden file pins.
- **Operator-facing UI** (`apps/hyperion/src/renderer`): load the `console-ux` skill before writing
  components, CSS, displayed strings or value formatting.
- Both skills are gated by `paths`, so they may not be offered until you touch matching files. If
  one isn't offered yet, read `.claude/skills/<name>/SKILL.md` directly.
- Write the task's tests with the code. Iterate with the narrowest command:
  `cargo test -p <crate> <filter>`, or `pnpm --filter hyperion exec vitest run <path>`.
- Stay inside the task. Work that belongs to a later task or plan waits for it, even when it is
  convenient now. The roadmap wants each task to leave `just ci` green on its own.

## 5. Validate

Invoke the `validate` skill with the task ID. It runs in its own context, picks the checks for
the changed files, runs the task's acceptance commands and the CI gate, and returns a verdict with
excerpts of any failures:

- **PASS** or **PASS WITH SKIPS**: go on, and name the skipped checks in the report.
- **FAIL**: fix the cause and validate again.
- **INCOMPLETE**: resolve the cause if you can (wrong arguments, a tooling error you can work
  around by checking the task by hand), otherwise report it. Don't loop on it.

Whatever the verdict:

- Never weaken a test, threshold, tolerance or lint to get to green. That includes widening a
  bracket the plan states and explaining it only in a code comment. If the plan's number is wrong,
  it is either a technical correction (record it in step 7) or a specification question (ask the
  user, as in step 3). Either way it appears in the report.
- Some acceptance steps say "check by hand once, do not commit", such as confirming that a lint
  fires. Do them, revert the temporary edit, and report the result.
- A benchmark target the task names is a finding, not a failure. Report the measurement.

## 6. Review

Invoke the `review-changes` skill with the task ID. It sends the diff to the specialist reviewers
in parallel and returns verified findings: Rust and TypeScript rules, determinism, UX guide, plan
conformance, physical accuracy. Fix every must-fix. Fix every should-fix too, unless you have a
reason, which goes in the report. Validate again after fixing.

## 7. Record as built

The plan is the record the next task reads. Under **Risks and open points**, record what was
built differently, in the style that section already uses. In galaxy-generation, plan 01 uses
`**Deviations in T<n>, as built.**` bullets and plan 02 numbered items tagged with the task, such
as `**R11. D4 re-checked (P02.T4).**`. If the plan has no such records yet, use the first form.
Keep entries terse and factual, each deviation with its reason. Cover renamed or added public
items, changed files or acceptance commands, and discoveries that affect later tasks. The
plan-conformance reviewer's output lists candidates. Don't rewrite the task text itself, except
through `revalidate-plan` (step 2). If nothing deviated, add nothing.

If the task changed generated output, check that the plan's **Generator version** section still
holds.

## 8. Report

Tell the user briefly:

- What exists now (files, public items) and the validation result, naming anything skipped and
  why, such as `just test-wasm` when wasmtime is missing.
- The decisions you made (step 3) and the deviations you recorded (step 7).
- What waits on the owner: confirmation items and open questions.
- A commit message, or the commit itself if the user asked for commits. Make one commit per task
  with a subject that starts with the task ID, such as `P02.T5.a Galaxy parameter draws`, so that
  `--list` can mark it. When a plan asks for owner-confirmation edits in their own commit, keep
  them there.
- The tasks this one unblocks, from the ordering notes.

When you run several tasks in sequence, finish all eight steps for one task before you start the
next.
