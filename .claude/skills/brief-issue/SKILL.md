---
name: brief-issue
description: Writes a self-contained task brief and posts it as a GitHub issue for the issue-working agent on another machine. It scaffolds the brief from the configuration (base commit, lane branch, gate, condensed rules, report, commit trailer), suggests the milestone and plan and area labels, and checks it: required sections, a pushed base commit, duplicates, and public-repo hygiene (no personal details, email addresses, local paths, Claude session links or secrets). It posts through gh, with a dry run, and puts the issue on the Projects board. It also answers or requeues a brief, and sets up the agent labels and the board. Use when the orchestrator hands a plan task or a free-form task to the worker, answers a blocked brief, sends a lane back, or sets up the queue.
argument-hint: "<task-id or summary> | answer <issue#>"
allowed-tools:
  - Bash(python3 -I .claude/skills/brief-issue/scripts/brief_issue.py *)
---

# Writing and posting an issue brief

The worker on the other machine sees only the issue and the repository at the base commit. It
can't read the orchestrator's records, plans in progress or local files. The **brief** is the
issue body, and it must hold everything the task needs. The worker reads it with the
`work-issue` skill.

Task: `$ARGUMENTS`.

`brief_issue.py` below means `python3 -I .claude/skills/brief-issue/scripts/brief_issue.py`, run
from the repository root. Drafts go in the orchestrator's scratch directory, never in the tree.

Copy this checklist into your reply and keep it current:

```
Brief for <task>:
- [ ] 1. Gather the task and its rulings
- [ ] 2. Scaffold
- [ ] 3. Write the task section
- [ ] 4. Pick the labels and milestone
- [ ] 5. Check, fix, repeat
- [ ] 6. Dry run, read as the worker
- [ ] 7. Post, then a status post
```

1. **Gather.**
   - For a plan task, run `python3 -I .claude/skills/implement-task/scripts/plan_task.py <ID> --context`.
     Note the plan file, the line numbers at the base commit, the design notes it cites and the
     Risks records that bind it.
   - Collect every ruling, decision and measurement the task depends on from your own records.
     They get pasted, condensed.
2. **Scaffold.** Run `brief_issue.py scaffold --title "<ID> <summary>" --slug <id-or-short-name> [--task <ID>]`
   and write the output to a draft file.
   - The base commit defaults to the integration branch's remote head, which must be pushed.
     Name another with `--base-commit`.
   - `--scope <glob>` (repeatable) limits the paths the lane may change; `land-issue` enforces it.
   - The draft holds the standard Base, Gate, Rules and Report sections from the configuration,
     `[[FILL: …]]` placeholders, and a hidden brief marker (slug, base, task, scope). Keep the
     marker.
   - For a brief written by hand instead, skip the scaffold. Give `check` and `post` the marker's
     fields: `--slug`, and `--task`, `--base-commit` and `--scope` as needed.
3. **Write** each `[[FILL]]`, following [writing.md](writing.md). Delete the placeholders you
   don't need.
4. **Labels.** `brief_issue.py labels --task <ID>` lists the labels by family and the open
   milestones, and suggests the plan label and milestone. Pick the area labels yourself. Add
   `heavy` for long CPU runs or large downloads, so that the worker runs the issue alone among
   heavy ones.
5. **Check.** Run `brief_issue.py check <draft> --title "…" --label <l>... --milestone "<m>"`, fix
   what it reports, and repeat until it prints no errors. It checks:
   - the required sections;
   - that the base commit is pushed;
   - that the commit trailer is quoted;
   - that repository paths exist at the base;
   - that the labels and milestone exist;
   - that no open brief covers the same task;
   - hygiene: no email addresses, home paths, this machine's host name, files inside `.git/`,
     secrets or Claude session links.
6. **Dry run.** Run the same command as `post … --dry-run`. Read the printed issue once as the
   worker would: could you do the task from it alone?
7. **Post.** Run `post` without `--dry-run`. It adds the task label, prints the issue's URL, and
   puts the issue on the board as queued. Then post a one-line status with the `post-status`
   skill, unless you post one for a batch.

## Answering and requeuing

- **A blocked brief:** write the answer, with the ruling pasted and any changed instruction. Run
  `brief_issue.py answer N --body-file <file> --requeue [--dry-run]`. It comments the answer
  (checked like a brief) and removes the state labels, so that a worker claims the issue again
  and continues from its pushed lane. Without `--requeue` it only comments: an amendment the
  current worker reads, since comments by allowed authors amend the brief.
- **The tag:** the answer begins with the orchestrator's tag (`coordination.tags`), since the
  worker may read only the comments that do. The brief's body carries none.
- **A lane that needs fixes:** answer with what to fix, with `--requeue`.
- **A stale claim** (the survey marks it): ask on the coordination log first, then requeue.

## Setup, once per repository and machine

- `brief_issue.py setup-labels [--dry-run]` creates the four agent labels where they are missing.
- `brief_issue.py board-setup [--dry-run]` finds or creates the board and checks its status
  options. Without the token's `project` scope it says so and skips. The scope is added with
  `gh auth refresh -s project`, on each machine that moves cards.
  - Grouping by milestone is a view setting, which the API can't create. Set it once in the web
    page. The default table view already shows the Milestone column.
- `brief_issue.py board-sync [--dry-run]` moves every open brief's card to the status its labels
  imply, after labels were changed by hand.

The configuration (`.claude/gh-collab/config.md`) holds the allowed authors, labels, base branch,
trailer, the condensed rules pasted into every brief, and the board.
