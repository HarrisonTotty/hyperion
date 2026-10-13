# What a brief holds

The worker starts cold. It has the repository at the base commit, this issue, and its own
skills. A good brief lets it start without asking anything. The scaffold fills the standard
sections. This file covers the parts you write.

## The task section

- **One line on what and why**, first. Say whether the task is docs only, test only, or touches
  a contract.
- **Where it is.** Give the plan file, the line numbers at the base commit (the parent task, the
  task, the subtasks it hands over to, the Risks records), and the extractor command. Name the
  code files and symbols to read first.
- **What to build,** condensed from the plan in its own words: inputs, outputs, files, names to
  keep. Name what is out of scope ("Don't start T4.e"). Tell the worker to list a needed change
  to another task in its report rather than making it.
- **The rulings it needs, pasted.** Give each decision, sign-off and measurement it depends on,
  with its date and gist, under its own heading. The worker can't read your decision files. A
  reference to one is a dead end.
- **Data and downloads:** the URLs, licences, checksums to record, and where to put them
  (outside the clone, never in a temporary directory that is in memory). Say what may be
  committed.
- **Acceptance:** the criteria with their exact commands. A criterion "by hand" says so.

## The gate and machine limits

- Name the exact gate command and its success line. Name any extra runs by name (slow tests,
  benchmarks), and how to report their times: the machine, its threads and its load.
- Name the worker machine's limits for heavy work: workers, memory caps, threads, load. Label
  such issues `heavy`.

## The report

List each item you will need when landing: figures in the plan's table form, timings, the
deviations recorded in the plan, deferred findings (one line each), and the questions with the
worker's lean. The scaffold adds the branch, the commits and the gate line.

## Style

- Number the sections when the brief is long, and refer back by number ("§3").
- Abbreviate long paths once at the top ("`08` is `docs/agent/plans/…/08-atmospheres.md`").
  Keep any path that the check should verify in full and in backticks.
- Write instructions, not narrative. Use one term per thing, the same as in the plan.
- Never include local paths, host names, email addresses (the trailer's no-reply address is
  allowed), Claude session links or secrets. The check refuses them; the issue is public.
