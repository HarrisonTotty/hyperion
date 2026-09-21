---
name: validate
description: Runs HYPERION's checks for the current changes in an isolated context and reports only failures with their key output. It covers formatting, Clippy and oxlint, type checks, targeted and full tests, slow statistical tests, wasm determinism, protocol bindings, and a plan task's acceptance commands. Use after changing code, before saying a task is done, or when the user asks to verify, check, test or run CI.
argument-hint: "[task-id] [--base <git-ref>]"
context: fork
agent: validator
background: false
---

Validate the HYPERION working tree and report back as your instructions describe.

Arguments: `$ARGUMENTS` (a plan task ID adds its acceptance criteria; `--base <ref>` widens the
diff from the default `HEAD`).

## Check plan

The plan below was chosen from the changed files when this skill was invoked. Run the tiers in
order. Tier 3 is the gate: work isn't done until it passes.

```!
python3 "${CLAUDE_SKILL_DIR}/scripts/select_checks.py" --args-stdin <<'HYPERION_VALIDATE_ARGS'
$ARGUMENTS
HYPERION_VALIDATE_ARGS
```

If this plan has no acceptance tier but a task ID appears in the arguments above, regenerate it:
`python3 .claude/skills/validate/scripts/select_checks.py <task-id>`.
