#!/usr/bin/env python3
"""Choose HYPERION's checks for the changed files and print them as a tiered check plan.

Usage:
    select_checks.py [TASK_ID] [--base REF] [other words are ignored]

Changes are the working tree against REF (default HEAD) plus untracked files. The plan has three
tiers: targeted checks for what changed (fast), the task's acceptance commands when a task ID is
given, and the gate that mirrors CI. Always exits 0, so a skill that injects its output never
aborts; problems are printed in the plan instead.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

TASK_RE = re.compile(r"^P\d{2}\.T\d+(?:\.[a-z]+)?$")
RUST_FILE = re.compile(r"(\.rs|Cargo\.(toml|lock)|clippy\.toml|rustfmt\.toml|rust-toolchain\.toml)$")
TS_FILE = re.compile(
    r"(\.(ts|tsx|mts|cts|js|mjs|cjs|jsx)|package\.json|tsconfig[^/]*\.json|pnpm-lock\.yaml|"
    r"\.oxlintrc\.json|pnpm-workspace\.yaml)$"
)
PRETTIER_SKIP = re.compile(r"(\.(rs|toml|lock)$|^packages/protocol/src/generated/|^\.claude/|^target/)")
WORKSPACE_WIDE = re.compile(r"^(Cargo\.(toml|lock)|rust-toolchain\.toml|rustfmt\.toml|\.cargo/)")
INFRA = re.compile(r"^(justfile|\.github/|\.pre-commit-config\.yaml)")
DETERMINISM_CRATES = {"hyperion-sim", "hyperion-testkit"}


def git(root: Path, *args: str) -> list[str]:
    try:
        out = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True, check=True)
    except (OSError, subprocess.CalledProcessError) as err:
        print(f"> git {' '.join(args)} failed: {err}")
        return []
    return [line for line in out.stdout.splitlines() if line]


def repo_root() -> Path:
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True)
        return Path(out.stdout.strip())
    except (OSError, subprocess.CalledProcessError):
        return Path(__file__).resolve().parents[4]


def parse_args(argv: list[str]) -> tuple[str | None, str]:
    # `--args-stdin` reads the words from standard input instead, so a skill can pass free-form
    # user arguments through a quoted heredoc without the shell ever parsing them.
    if "--args-stdin" in argv:
        argv = [a for a in argv if a != "--args-stdin"] + sys.stdin.read().split()
    task, base = None, "HEAD"
    it = iter(argv)
    for word in it:
        if word == "--base":
            base = next(it, "HEAD")
        elif word.startswith("--base="):
            base = word.split("=", 1)[1]
        elif TASK_RE.match(word):
            task = word
    return task, base


def crate_of(path: str) -> str | None:
    parts = path.split("/")
    return parts[1] if len(parts) > 2 and parts[0] == "crates" else None


def wasm_available() -> tuple[bool, str]:
    runner = os.environ.get("WASMTIME", "wasmtime")
    if shutil.which(runner) is None:
        return False, "wasmtime is not on PATH (CI runs it)"
    try:
        targets = subprocess.run(
            ["rustup", "target", "list", "--installed"], capture_output=True, text=True, check=True
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return False, "cannot query rustup targets"
    if "wasm32-wasip1" not in targets:
        return False, "target wasm32-wasip1 not installed (`rustup target add wasm32-wasip1`)"
    return True, ""


def main() -> None:
    root = repo_root()
    task, base = parse_args(sys.argv[1:])

    changed = git(root, "diff", "--name-only", base) + git(root, "ls-files", "--others", "--exclude-standard")
    changed = sorted(set(changed))
    deleted = set(git(root, "diff", "--name-only", "--diff-filter=D", base))

    print(f"Repository: {root}")
    print(f"Changes: working tree against {base}, untracked files included")
    if task:
        print(f"Task: {task}")
    print()

    if not changed and task is None:
        print("No changed files. Nothing to validate unless the caller asked for the full gate:")
        print("- `just ci`")
        return

    rust = [f for f in changed if RUST_FILE.search(f)]
    ts = [f for f in changed if TS_FILE.search(f) and not f.startswith("packages/protocol/src/generated/")]
    prettier = [f for f in changed if not PRETTIER_SKIP.search(f)]
    crates = sorted({c for f in rust if (c := crate_of(f))})
    workspace_wide = any(WORKSPACE_WIDE.search(f) for f in changed)
    infra = [f for f in changed if INFRA.search(f)]
    protocol = [f for f in changed if f.startswith(("crates/hyperion-protocol/", "packages/protocol/src/generated/"))]
    determinism = [f for f in changed if crate_of(f) in DETERMINISM_CRATES] or (workspace_wide and bool(rust))
    goldens = [f for f in changed if "/tests/golden/" in f]
    benches = sorted({f for f in changed if re.match(r"crates/[^/]+/benches/[^/]+\.rs$", f)})
    ux_guide = "docs/frontend/ux-guidelines.md" in changed

    areas = []
    if rust:
        areas.append(f"Rust ({', '.join(crates) or 'workspace'})")
    if ts:
        areas.append("TypeScript")
    if protocol:
        areas.append("protocol")
    if goldens:
        areas.append(f"{len(goldens)} golden file(s)")
    if infra:
        areas.append("CI/build config")
    docs = [f for f in changed if f.endswith(".md")]
    if docs:
        areas.append(f"{len(docs)} Markdown file(s)")
    print(f"Changed files ({len(changed)}): {'; '.join(areas) or 'other'}")
    for f in changed[:40]:
        print(f"  {f}{'  (deleted)' if f in deleted else ''}")
    if len(changed) > 40:
        print(f"  … and {len(changed) - 40} more")
    print()

    tier1: list[tuple[str, str]] = []
    if rust:
        tier1.append(("cargo fmt --all -- --check", "Rust files changed"))
        tier1.append(("cargo clippy --workspace --all-targets -- -D warnings", "Rust files changed; pedantic, warnings are errors"))
        if workspace_wide or not crates:
            tier1.append(("cargo test --workspace", "workspace-wide Rust config changed"))
        else:
            for crate in crates:
                tier1.append((f"cargo test -p {crate}", f"{crate} changed"))
        for bench in benches:
            crate = crate_of(bench)
            tier1.append((f"cargo bench -p {crate} --bench {Path(bench).stem} -- --test", "benchmark changed; runs each once"))
    if protocol:
        tier1.append(("just gen-protocol-check", "protocol types or generated bindings changed"))
    if prettier:
        tier1.append(("pnpm format:check", "Prettier owns TS, CSS, JSON, YAML and Markdown formatting"))
    if ts:
        tier1.append(("pnpm typecheck", "TypeScript changed"))
        tier1.append(("pnpm lint", "oxlint --type-aware --deny-warnings"))
        tier1.append(("pnpm test", "TypeScript changed"))

    print("## Tier 1: targeted (run all; stop before tier 2 if any fail)\n")
    if tier1:
        for i, (cmd, why) in enumerate(tier1, 1):
            print(f"{i}. `{cmd}`  # {why}")
    else:
        print("(nothing targeted for these files)")
    print()

    print("## Tier 2: task acceptance\n")
    if task:
        script = root / ".claude" / "skills" / "implement-task" / "scripts" / "plan_task.py"
        if script.exists():
            out = subprocess.run(
                [sys.executable, "-B", str(script), task, "--acceptance"], capture_output=True, text=True, cwd=root
            )
            if out.returncode == 0:
                print(out.stdout.strip())
                print("\n(A command listed here and in tier 3, such as `just ci`, runs once, in tier 3.)")
            else:
                print(f"TOOLING ERROR: plan_task.py exited {out.returncode}; report it, and read the task's")
                print(f"acceptance criteria in its plan by hand. Its message: {(out.stderr or out.stdout).strip()}")
        else:
            print(f"(plan_task.py not found; read {task}'s acceptance criteria in its plan)")
    else:
        print("(no task ID given)")
    print()

    gate: list[tuple[str, str]] = [("just ci", "everything CI's Rust job runs: fmt, check, clippy, tests, slow tests, bindings")]
    skipped: list[str] = []
    if ts or infra:
        gate.append(("pnpm build", "CI's frontend job also builds the client"))
    if determinism:
        ok, reason = wasm_available()
        if ok:
            gate.append(("just test-wasm", "sim or testkit changed; goldens must hold bit for bit on wasm32"))
        else:
            skipped.append(f"`just test-wasm`: {reason}")
        skipped.append("AArch64 golden run: CI only (`rust-aarch64` job)")
    print("## Tier 3: gate (mirrors CI; run after tiers 1 and 2 pass)\n")
    for i, (cmd, why) in enumerate(gate, 1):
        print(f"{i}. `{cmd}`  # {why}")
    if skipped:
        print("\nNot runnable here (report as skipped, never as passed):")
        for s in skipped:
            print(f"- {s}")
    print()

    notes = []
    if goldens:
        notes.append(
            "Golden files changed: `python3 .claude/skills/sim-determinism/scripts/golden_diff.py` "
            "shows whether GENERATOR_VERSION was bumped and which goldens changed beyond their header."
        )
    if ux_guide:
        notes.append("The UX guide changed: plan tasks that edit it usually give `grep` strings as acceptance.")
    if infra:
        notes.append("CI or hook config changed: the gate above is the minimum; compare with .github/workflows/ci.yml.")
    if notes:
        print("## Notes\n")
        for n in notes:
            print(f"- {n}")


if __name__ == "__main__":
    main()
