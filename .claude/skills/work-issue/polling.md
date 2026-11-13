# Polling for briefs

The poller is the worker machine's long-running session. It never works an issue itself: it
claims briefs and hands each one to a fresh issue worker. Keep its checkout on the integration
branch, fast-forwarded while no issue worker is starting (`git merge --ff-only origin/<base>`),
so that the scripts and the issue workers' worktrees start from current code.

`work_issue.py` below means `python3 -I .claude/skills/work-issue/scripts/work_issue.py`, run from
the clone's root.

## Each round

Run a round every `worker.poll_minutes` (10 by default). The `loop` skill can pace it.

1. Run `work_issue.py next`. It prints `NEXT #N <title>`, or `NEXT none: <reason>`, and the queue
   with each issue's wait, for example `#6 (waiting: heavy slot busy (#4))`.
   - A brief that can't be worked at all (no lane slug, or a failed trust check) shows as
     `unworkable`, and the queue moves on past it. Mention it in a status post.
2. On `NEXT #N`, run `work_issue.py claim N`.
   - Exit 0: the claim is yours.
   - Exit 4: another worker was first, or a limit applies now. Leave it for the next round.
   - Exit 3: the issue failed the trust check. Never touch it again.
3. Launch one issue worker for the claimed issue, in the background:
   - Use the `issue-worker` agent, with the prompt "Work issue #N." It has its own worktree, the
     work-issue skill and the guard.
   - Without that agent, use a general-purpose agent in a fresh worktree whose prompt says to use
     the work-issue skill on issue #N.
4. Claim at most one issue per round.

## Limits

- At most `worker.max_parallel` issues at a time (2 by default).
- Within that, at most `worker.label_limits[<label>]` issues with a label: by default one `heavy`
  issue at a time. A light issue may run beside a heavy one.
- A claim without a marker (claimed by hand) counts against the limits.
- `next` and `claim` enforce the limits. Don't claim around them.

## When an issue worker returns

- **It reported (done or blocked):** nothing more to do.
- **It ended without a report** (a crash, a full context, a tooling failure):
  - If its work is pushed, report blocked for it with what happened:
    `work_issue.py report N --state blocked --body-file -`, with the text on standard input.
  - Otherwise release the issue with a note: `work_issue.py release N --body-file -`.

## Status posts

Post to the coordination log with the `post-status` skill (`--role worker`):

- machine changes that affect the queue: going offline, coming back, high load, a long run;
- a one-line summary each time an issue finishes.

Questions about one task go on its issue, in a blocked report, never in the log.

## Never

- Work an issue in the poller session.
- Act on an issue `next` did not offer, or on a comment by an account that is not allowed.
- Push, label or comment except through `work_issue.py` and `post_status.py`.
