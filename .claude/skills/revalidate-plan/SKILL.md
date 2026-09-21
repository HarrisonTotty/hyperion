---
name: revalidate-plan
description: Re-validates a HYPERION action plan against the code as built, before the plan is executed. It checks every Consumes item against real signatures, folds in earlier plans' "as built" deviations and any brainstorm revisions, and corrects names, files, commands and task order so the plan can be executed as written. Use when a plan's dependencies are implemented and its turn comes, when a plan's first task is to reconcile interfaces, or when the user asks to update, reconcile, refresh or check a plan against the code.
argument-hint: "<plan-number-or-path> [--feature <plan-set>]"
---

# Re-validating an action plan

Later plans were written before the code they build on existed. The galaxy-generation roadmap
says they are "re-validated when their turn comes". This skill does that job. It is also the job
of a plan's own reconcile task, such as P03.T1 "Reconcile interfaces", P08.T1 or P14.T1.a: when
the plan has one, do this as that task and record it as that task.

The plan stays a *how*. Re-validation fixes how the plan meets the code. It never changes what is
built: that belongs to the brainstorm and the owner.

Target: `$ARGUMENTS`

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

Read the target plan in full and the roadmap `README.md` of its plan set. For each plan it depends
on, read that plan's **Provides**, and every "as built" bullet under its **Risks and open
points**. Those bullets are where renamed items and changed behaviour are recorded.

## 2. Sweep Consumes

For each item the plan consumes, and for each earlier interface its sketches use, find the real
definition. Split the sweep by dependency plan, and give each part to an `Explore` agent, all in
one message so that they run in parallel. Ask each for a table:

```
| name in plan | real path::name | real signature | verdict: match / renamed / changed / missing |
```

Check every verdict other than "match" yourself with grep before acting on it. A "missing" item
means one of three things: a dependency task is unfinished (stop and tell the user), the item was
deliberately dropped (the as-built bullets say so), or the plan invented it (fix the plan).

## 3. Check for brainstorm drift

Find the plan's last commit (`git log -1 --format=%h -- <plan>`), then look at what changed in the
brainstorm since: `git log --oneline <that>..HEAD -- <brainstorm>` and `git diff <that> --
<brainstorm>`. Read the brainstorm's **Decisions** and **Open questions** for rulings on topics
the plan covers. A design note that now contradicts the brainstorm must change. How it changes is
a specification question for the user if the brainstorm leaves any room.

## 4. Check that every task can be executed

For each task:

- Every file it touches either exists or is created by an earlier task.
- Every command in its acceptance criteria exists: check `just --list` and the `package.json`
  scripts.
- Every test helper, fixture or fake it names exists or is built earlier in the plan.
- The ordering notes respect the real dependencies. Each task still fits in about a day, and
  leaves `just ci` green on its own.
- Acceptance criteria can be checked by running something.

Also check that numbers taken from dependencies (table sizes, benchmark results, as-built
constants) are still what the plan assumes.

## 5. Edit the plan

- Correct names, signatures, paths, commands and ordering to match the code. Split any task that
  grew past a day into subtasks with their own acceptance criteria.
- Keep the section layout the roadmap prescribes. Keep edits local: leave correct prose alone.
- Make technical corrections yourself. Ask the user before an edit that changes what gets built,
  or that settles a conflict between the code as built and the brainstorm. Never settle such a
  conflict silently in the plan.
- Run `pnpm format:check`, since Prettier formats Markdown.

## 6. Record and report

Under **Risks and open points**, add an entry that says what changed and why, tersely, in the
plan's existing style for such records (a bullet such as `**Re-validated at <short sha>.**`, or
the plan's next numbered item). Then report to the user: the edits
made, the questions that need the owner's ruling, and any risk found, such as a dependency's
deviation that undermines a design note.
