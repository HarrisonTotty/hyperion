# gh-collab configuration

The skills `brief-issue`, `work-issue`, `land-issue`, `survey-issues` and `post-status` share
`ghcollab.py` and one configuration. JSON files merge in this order, each overriding the last key
by key:

1. the built-in defaults (`DEFAULTS` in `ghcollab.py`);
2. `config.json` beside this file: the repository's values, committed and public;
3. `<git common dir>/gh-collab/config.json`: this clone's values, never committed. Machine-specific
   values go here: gate commands with local caps, a worker id, extra forbidden terms;
4. the file named by `$GH_COLLAB_CONFIG`, if set.

A key starting with `//` is a comment. If a file is unreadable or a value is invalid, every
script stops; none falls back to looser defaults. `python3 -I .claude/gh-collab/ghcollab.py config`
prints the merged result and its sources.

## Keys

| Key | Default | Meaning |
|---|---|---|
| `repo` | `gh repo view` | `OWNER/NAME`. Every `gh` call names it. |
| `remote` | `origin` | The remote that holds the integration branch and the lanes. |
| `base_branch` | the repository's default branch | The integration branch: lanes start from it and land on it. |
| `allowed_authors` | the repository's owner | Accounts whose issues and comments are trusted. Nothing else is. |
| `labels.task`, `.claimed`, `.done`, `.blocked` | `agent-task`, `agent-claimed`, `agent-done`, `agent-blocked` | The queue's labels. |
| `branch_prefix` | `lane/` | Lane branches are `<prefix><issue#>-<slug>`. Only these are ever pushed. |
| `task_id_pattern` | none | A regex for task IDs, with a named group `plan`. |
| `plan_label` | `plan:{plan}` | The label a task ID suggests. |
| `commit.subject_pattern` | Conventional Commits types | The subject every lane commit must match. |
| `commit.trailer` | none | The trailer a brief quotes. |
| `commit.trailer_pattern` | the trailer, escaped | The checks require exactly one matching trailer, as the message's last line. |
| `commit.require_task_id` | `true` | A brief that names a task requires its ID in every subject. |
| `commit.max_added_file_kb` | `500` | The largest file a lane commit may add. |
| `brief.required_sections` | Base, Task, Gate, Rules, Report | Headings the check requires, matched by substring. |
| `brief.label_families` | `plan:`, `area:` | The check warns when a family has no label. |
| `brief.require_milestone` | `true` | |
| `brief.setup`, `.rules`, `.report` | none | Lines the scaffold pastes into Base, Rules and Report. These are the repository's condensed rules. |
| `worker.gate_command` | none | The worker's gate, run through the shell from the root; `{remote}` and `{base}` are filled in. |
| `worker.id` | a hash of the host and clone | The name in claims and reports. No host name is ever posted. |
| `worker.max_parallel` | `2` | Issues one worker runs at a time. |
| `worker.label_limits` | `{"heavy": 1}` | At most this many of the worker's issues with the label at once, within `max_parallel`. A light issue runs beside a heavy one; `0` means the worker never takes the label. |
| `worker.poll_minutes` | `10` | The poller's interval. |
| `worker.claim_settle_seconds` | `5` | How long after claiming to re-read the comments and see who claimed first. |
| `worker.push_no_verify_after_gate` | `false` | Skip the pre-push hook once the stamped gate covers HEAD's tree. |
| `worker.deny_bash` | none | Extra regexes the worker's Bash guard refuses. |
| `land.gate_command` | none | The orchestrator's gate after picking. |
| `land.push_args` | none | Flags printed in the suggested push command. The script never pushes. |
| `survey.stale_claim_hours` | `24` | A claim with no trusted activity for this long is flagged. |
| `coordination.discussion` | `0` (none) | The coordination log's discussion number or URL. |
| `coordination.tags.orchestrator`, `.worker` | `[orchestrator]`, `[worker]` | The tag that begins each side's status posts and the orchestrator's comments on issues (answers, closing comments), since both sides may post as one account and the worker may read only the comments that begin with the orchestrator's tag. A brief's body carries none: the task label and its marker identify it. |
| `project.enabled` | `false` | Keep a Projects (v2) board in step with the labels. |
| `project.owner`, `.number`, `.title` | the repository's owner; found by title | The board. |
| `project.status_field` | `Status` | The single-select field that the statuses go in. |
| `project.statuses.*` | Todo, Claimed, In progress, In progress, Blocked, Done | The option for queued, claimed, started, done (the done label: awaiting landing), blocked and landed. Options are found by name, so their IDs are never configured. |
| `hygiene.allowed_emails` | the Claude trailer's no-reply address, `git@github.com` | Addresses public text may hold. Any no-reply address is allowed too. |
| `hygiene.forbidden` | none | Extra regexes public text must not match. Keep personal ones in the clone's file, not here. |
| `scratch_dir` | `<git common dir>/gh-collab` | Gate logs, stamps and the temporary index. |

## Markers

Scripts leave hidden HTML comments, `<!-- gh-collab:<kind> {json} -->`, in issue bodies and
comments. The kinds are `brief`, `claim`, `start`, `report`, `release`, `answer`, `landed` and
`status`. A marker counts only in a body or comment by an allowed author.

- **Claims:** the earliest claim since the last report, release or requeue wins. A worker that
  loses deletes its own claim.
- **Tags come first, markers anywhere.** A tag begins the text: inline before prose, on a line of
  its own before a Markdown block (a heading, list, quote, table or fence). Markers are found
  anywhere in a body, so a tag never hides one.
- **Briefs written by hand** need no marker. The lane slug and base commit are read from
  `lane/<…>-<slug>` and `at <sha> or later`, and the task from the title.

## The board

The board needs the token's `project` scope. Without it, every board step prints
`board: skipped (…)` and the rest of the step goes on.

- **The scope:** run `gh auth refresh -s project` on each machine that moves cards.
- **Setup:** `brief-issue`'s `board-setup` finds or creates the board and checks the status
  options.
- **Grouping by milestone** is a view setting, which the API can't create: set it in the board's
  web page. The default table view shows the Milestone column.

## Tests

Each script has a standard-library test file beside it. `gh` is replaced by the fake GitHub in
`fakes.py`, and git runs on throwaway repositories under `$GH_COLLAB_TEST_TMP`, else
`<git common dir>/gh-collab/test-tmp`. No test touches a real repository, issue or board.

```bash
for t in .claude/gh-collab/test_ghcollab.py .claude/skills/*/scripts/test_*_issue*.py \
         .claude/skills/post-status/scripts/test_post_status.py; do python3 -I -B "$t" || break; done
```

## Security

- **Code, not prose, enforces the rules.** The trust check covers an open issue, an allowed
  author, the task label, and a body last edited by an allowed author. Every acting command runs
  it.
- **Others' text stays hidden.** Comments by other accounts are never shown to the worker, and
  the survey only counts them.
- **Earlier workers' text is context.** Workers post as an allowed account. `show` therefore
  lists the comments that carry a worker's marker as context, never as instructions.
- **The push is fixed.** `work_issue.py push` pushes `HEAD:refs/heads/<lane>` with no `+`, after
  the stamped gate and the commit checks pass. It skips the pre-push hook only when the log ran
  the configured gate, perhaps behind a wrapper.
- **A lane can't change the rules.** The commit checks refuse any lane commit that changes the
  protected paths (`PROTECTED` in `ghcollab.py`): this code, `config.json`, the five skills, the
  worker agent and `.claude/settings*.json`. They also refuse an author or committer address new
  to the integration branch, and personal details in messages.
- **The lane can't vouch for itself.** The issue worker runs the scripts from the session's own
  clone (`${CLAUDE_SKILL_DIR}`), not from the worktree it edits.
- **A rewritten lane is caught.** Landing requires the lane to still contain the head its report
  named.
- **The guard is a second line.** The worker's guard hook refuses, in Bash, raw pushes, `gh`
  writes, comment reads and hook skipping. For Edit and Write, it refuses edits to the protected
  paths. It is a defence against mistakes and injected text, not a sandbox.
- **The guard doesn't load everywhere.** Agent and skill frontmatter hooks run only in a trusted
  folder, not in a `-p` session, and not for the agents the worker itself starts. To guard every
  session and subagent on the worker machine, add the same hook to that clone's
  `.claude/settings.local.json` (matcher `Bash|Edit|Write|MultiEdit|NotebookEdit`).
- **Public text is checked.** Every brief, report, answer, closing comment and status post is
  checked for email addresses, home paths, the host name, files inside `.git/`, secrets and
  Claude session links before it is posted. Each command that posts has a dry run.
