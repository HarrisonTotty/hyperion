---
name: validate
description: Runs HYPERION's checks for the current changes in an isolated context and reports a verdict, a results table and excerpts of what failed. It covers formatting, Clippy and oxlint, type checks, targeted and full tests, slow statistical tests, wasm determinism, protocol bindings, and a plan task's acceptance commands. Use after changing code, before saying a task is done, or when the user asks to verify, check, test or run CI.
argument-hint: "[task-id] [--base <git-ref>] [--feature <plan-set>]"
context: fork
agent: validator
background: false
---

Validate the HYPERION working tree and report back as your instructions describe.

Arguments: `$ARGUMENTS`

A plan task ID adds its acceptance criteria. `--base <ref>` widens the diff from the default
`HEAD`, and `--feature <plan-set>` names the plan set when plan numbers are ambiguous.

First make the check plan from the repository root. Pass the arguments above to the script as
separate, shell-quoted words. It ignores anything it doesn't recognise and says so:

```bash
python3 .claude/skills/validate/scripts/select_checks.py <arguments>
```

Then run the plan's tiers in order. Tier 3 is the gate: work isn't done until it passes.
