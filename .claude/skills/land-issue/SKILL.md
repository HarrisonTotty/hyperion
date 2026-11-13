---
name: land-issue
description: Lands a worker's finished lane branch on the integration branch, for the orchestrator. It fetches the issue's lane branch and checks it: non-merge commits only, conventional subjects with the task ID, exactly one trailer, no Claude session links or secrets added, no oversized files, nothing outside the brief's scope, no force-push. It then cherry-picks it linearly, runs the gate into a stamped log, and, after the orchestrator's own push step, closes the issue with what landed and moves its board card. It never pushes. Use when an agent-done issue is ready to land, or when asked to integrate, cherry-pick or close a worker's issue.
argument-hint: "<issue-number>"
allowed-tools:
  - Bash(python3 -I .claude/skills/land-issue/scripts/land_issue.py *)
---

# Landing an issue's lane

Issue: `$ARGUMENTS`.

The integration branch stays linear: only the lane's non-merge commits are picked, never a merge,
never a force-push. The push is yours, in its own step, after the gate's log ends `exit 0`. The
script never pushes, and it won't close the issue until the commits are on the pushed
integration branch.

`land_issue.py` below means `python3 -I .claude/skills/land-issue/scripts/land_issue.py`. Run it
in the integration branch's checkout, with the integration branch checked out and clean.

Copy this checklist into your reply and keep it current:

```
Landing #N:
- [ ] 1. Read the report
- [ ] 2. Check the lane
- [ ] 3. Pick
- [ ] 4. Gate, then check its log
- [ ] 5. Push (own step)
- [ ] 6. Close
- [ ] 7. Status post
```

1. **Read** the worker's report on the issue: commits, gate line, deviations, deferred findings,
   questions. File the deferred findings and questions where your records keep them.
2. **Check:** `land_issue.py check N`. It lists what would be picked, the merges it skips, the
   files changed and the brief's scope. It fails on:
   - a lane that no longer holds the head its report named, or that was rewritten since the last
     fetch;
   - a commit without a conventional subject or the task ID, or without exactly one trailer as its
     last line;
   - a commit message with personal details or a Claude session link, or an author or committer
     address new to the integration branch;
   - an added line with a secret or session link, or an added file over the size limit;
   - a change to the collaboration's own files, or to a path outside the declared scope.
   - With no scope declared, judge the file list yourself.
   - A failure is the worker's to fix: send the lane back with
     `brief-issue`'s `answer N --requeue`, stating what to fix.
3. **Pick:** `land_issue.py pick N` checks again, then cherry-picks onto the integration branch.
   - The branch must be checked out, clean, and not behind its remote. Earlier unpushed picks are
     fine.
   - It skips commits already landed, matched by subject and author date, which cherry-pick
     keeps.
   - On a conflict it stops mid-pick. Resolve it keeping both sides' intent and regenerate
     generated files, then run `git cherry-pick --continue`, or `git cherry-pick --abort`.
4. **Gate:** run `land_issue.py gate run N` in the background, then `land_issue.py gate check N`.
   - `pick` says whether the picked tree is the lane head's. Only then did the worker's reported
     gate run on what you would push. Whether that report may stand in for your gate is your
     repository's rule. The script never decides it.
   - It runs `land.gate_command`, or yours after `--` (with this machine's caps). The log records
     HEAD's tree. The check passes only for a passing log on HEAD's tree.
   - A failure: fix forward on the integration branch, or drop the unpushed picks
     (`git reset --hard <the commit before the picks>`). Then send the lane back if the fix is
     the worker's.
5. **Push** the integration branch in its own command, never chained to the check, with the
   repository's push flags (`pick` prints the command).
6. **Close:** `land_issue.py close N --dry-run` prints the closing comment: each landed commit
   with its lane commit, and the gate line. Add `--note-file <file>` for sign-off notes or
   follow-ups. Then run it without `--dry-run`.
   - The comment begins with the orchestrator's tag (`coordination.tags`), since the worker may
     read only the comments that do, and ends with the hidden `landed` marker.
   - It refuses while any lane commit is missing from the pushed integration branch.
   - It closes the issue as completed and moves the board card to landed.
7. **Status:** post what landed and what is queued next with the `post-status` skill
   (`--role orchestrator`), once per batch.

`brief-issue`'s `answer` and `survey-issues` cover the rest of the orchestrator's side.
