---
name: revalidate-plan
description: Re-validates a HYPERION action plan against the code as built, before the plan is executed. It checks every Consumes item against real signatures, folds in earlier plans' "as built" deviations and any brainstorm revisions, and corrects names, files, commands and task order so the plan can be executed as written. Use when a plan's dependencies are implemented and its turn comes, when a plan's first task is to reconcile interfaces, or when the user asks to update, reconcile, refresh or check a plan against the code.
argument-hint: "<plan-number-or-path> [--feature <plan-set>]"
---

# Re-validating an action plan

Later plans were written before the code they build on existed. The galaxy-generation roadmap
says they are "re-validated when their turn comes". This skill does that job.

Some plans open with a reconcile task, such as galaxy-generation's P03.T1 "Reconcile interfaces",
P08.T1 or P14.T1.a. This skill does the plan-editing part of such a task. Build the rest of it
(module skeletons, stubs, domain tags) through the `implement-task` skill under the task's ID, so
that its commit names the task. Where the task says to record its findings somewhere else, such as
P14.T1.a's module docs, follow the task.

The plan stays a *how*. Re-validation fixes how the plan meets the code. It never changes what is
built: that belongs to the brainstorm and the owner, the user who rules on the specification. If
the roadmap says a plan is to be executed exactly as written (galaxy-generation plans 01–05),
limit your edits to reconciling names, and bring anything else to the owner.

Target: `$ARGUMENTS`. A number means `docs/agent/plans/<plan-set>/<NN>-*.md`; pass `--feature`
when more than one plan set exists. If no plan was given, ask which one.

Copy this checklist into your reply and keep it current:

```
Re-validation of plan <NN>:
- [ ] 1. Load the plan and its dependencies' records
- [ ] 2. Sweep Consumes against the code
- [ ] 3. Check for brainstorm drift
- [ ] 4. Check that every task can be executed
- [ ] 5. Edit the plan
- [ ] 6. Record and report
```

## 1. Load

Read the target plan in full and the roadmap `README.md` of its plan set. List its tasks with
`python3 .claude/skills/implement-task/scripts/plan_task.py --list <NN>` (from the repository
root).

Take the dependency plans from the plan's **Consumes** entries, not from the header's "Depends on"
line, which folds in transitive dependencies. For each dependency, read its **Provides** and its
whole **Risks and open points** section. That is where renamed items and changed behaviour are
recorded, in one of two forms: "Deviations in …, as built." bullets, or numbered items tagged with
a task, such as "R11. D4 re-checked (P02.T4)". Note any earlier "Re-validated at" record in the
target plan too.

Then check what is built: grep for a few names from each dependency's Provides.

- If none of the plan's dependencies is built, stop here and tell the user.
- If only some are, re-validate the tasks whose inputs exist. For each of the rest, record
  "pending re-validation: T<n> waits on P<NN>.T<m>" in step 6.

## 2. Sweep Consumes

For each item the plan consumes, and for each earlier interface its sketches use, find the real
definition. Split the sweep by dependency plan, and give each part to an `Explore` agent at medium
thoroughness, all in one message so that they run in parallel. Explore agents start with no
context and can't be asked follow-ups, so each prompt carries everything:

- the item names, each with its sketch quoted from the Consumes entry and the dependency's
  Provides;
- the renames that step 1 found;
- for an item described in prose rather than by name (such as "the request layer"), what it must
  do, so that the agent can find the module that does it;
- the output: one row per item, including items not found, with the definition's `file:line` and
  its signature quoted verbatim.

```
| name in plan | real path::name (file:line) | real signature, verbatim | verdict: match / renamed / changed / missing |
```

Steps 3 and 4 can go ahead while the agents run. Wait for every table before step 5.

Compare every returned signature with its sketch yourself, including rows marked "match", and
check other verdicts with grep before acting on them. A "missing" item means one of three things:
a dependency task is unfinished (record it as pending, as in step 1), the item was deliberately
dropped (the dependency's Risks records say so), or the plan invented it (fix the plan).

## 3. Check for brainstorm drift

Anchor on the plan's latest "Re-validated at <sha>" record. If it has none, anchor on the commit
that created the plan (`git log --diff-filter=A --format=%h -- <plan>`); the plan's last commit
may be an unrelated edit. Then look at what changed in the brainstorm since:
`git log --oneline <anchor>..HEAD -- <brainstorm>` and `git diff <anchor> -- <brainstorm>`. Read
the brainstorm's **Decisions** and **Open questions** for rulings on topics the plan covers. A
design note that now contradicts the brainstorm must change. How it changes is a question for the
owner if the brainstorm leaves any room.

## 4. Check that every task can be executed

For each task:

- Every file it touches either exists or is created by an earlier task.
- Every command in its acceptance criteria exists (`just --list`, the `package.json` scripts) and
  selects the right tests. `plan_task.py <task-id> --acceptance` extracts the commands and flags
  suspect `cargo test` filters. Compare each filter with the test names or module path the task
  creates.
- Every test helper, fixture or fake it names exists or is built earlier in the plan.
- The ordering notes respect the real dependencies. Each task still fits in about a day, and
  leaves `just ci` green on its own.
- Acceptance criteria can be checked by running something.

Also check that numbers taken from dependencies (table sizes, benchmark results, as-built
constants) are still what the plan assumes.

## 5. Edit the plan

- Correct names, signatures, paths, commands and ordering to match the code. Split any task that
  grew past a day into subtasks with their own acceptance criteria.
- Never renumber an existing task: other plans and commit subjects cite task IDs. Change the order
  through the ordering notes, and give new work a new ID or subtask letter. Grep
  `docs/agent/plans/` for an ID before you change anything about it.
- Keep the section layout the roadmap prescribes. Keep edits local: leave correct prose alone.
- Make technical corrections yourself. Ask the owner before an edit that changes what gets built,
  or that settles a conflict between the code as built and the brainstorm. Never settle such a
  conflict silently in the plan.
- Prettier formats Markdown: run `pnpm exec prettier --check <plan>`, and `--write` if it fails.

## 6. Record and report

Under **Risks and open points**, add an entry that says what changed and why, tersely, in the
plan's existing style for such records (a bullet such as `**Re-validated at <short sha>.**`, or
the plan's next numbered item), including any tasks pending re-validation. Then report to the
user: the edits made, the questions that need the owner's ruling, and any risk found, such as a
dependency's deviation that undermines a design note.
