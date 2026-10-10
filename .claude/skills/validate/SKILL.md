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

The gate is `just smart-ci`: the steps of `just ci` that the branch's change reaches, chosen from its
diff against the integration branch (`just smart-ci --plan` lists them, each with the paths that
caused it), and `just test-render` when a shader, the engine or the smoke harness changed. It runs
the full `just ci` by itself when the change touches an input of the whole workspace (the justfile,
a manifest or lock file, the toolchain) or a path no rule knows. A lane runs `just smart-ci`, not
`just ci`; integration runs the full `just ci`. A `just ci` in a task's acceptance criteria is met
by `just smart-ci`.
