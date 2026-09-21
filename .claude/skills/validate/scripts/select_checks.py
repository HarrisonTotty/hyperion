#!/usr/bin/env python3
"""Choose HYPERION's checks for the changed files and print them as a tiered check plan.

Usage:
    select_checks.py [TASK_ID] [--base REF | --base A..B] [--feature PLAN_SET] [--args-stdin]

Changes are the working tree against REF (default HEAD) plus untracked files, or, for A..B, the
commits in that range. A bare word that names a commit is taken as the base. The plan has three
tiers: targeted checks for what changed (fast), the task's acceptance commands when a task ID is
given, and the gate that mirrors CI. `--feature` names the plan set when plan numbers are ambiguous.
`--args-stdin` reads the arguments from standard input as one line of free text.

Always exits 0. Problems are printed in the plan as `TOOLING ERROR:` lines, and ignored arguments
are listed, so the caller can report them.
"""

from __future__ import annotations

import os
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True

TASK_RE = re.compile(r"^P(\d{2})\.T(\d+)(?:\.([a-z]+))?$", re.IGNORECASE)
RUST_FILE = re.compile(r"(\.rs|Cargo\.(toml|lock)|clippy\.toml|rustfmt\.toml|rust-toolchain\.toml)$|^\.cargo/")
TS_FILE = re.compile(
    r"(\.(ts|tsx|mts|cts|js|mjs|cjs|jsx)|package\.json|tsconfig[^/]*\.json|pnpm-lock\.yaml|"
    r"\.oxlintrc\.json|pnpm-workspace\.yaml)$"
)
GENERATED = "packages/protocol/src/generated/"
PRETTIER_SKIP = re.compile(rf"(\.(rs|toml|lock)$|^{GENERATED}|^\.claude/|^target/)")
WORKSPACE_WIDE = re.compile(r"^(Cargo\.(toml|lock)|rust-toolchain\.toml|rustfmt\.toml|\.cargo/)")
INFRA = re.compile(r"^(justfile|\.github/|\.pre-commit-config\.yaml)")
DETERMINISM_CRATES = {"hyperion-sim", "hyperion-testkit", "hyperion-fit"}
# `cargo test` runs ts-rs's export tests, which rewrite the checked-in bindings through
# TS_RS_EXPORT_DIR (.cargo/config.toml). An environment value takes precedence over that config,
# so pointing it at a scratch directory keeps the tree untouched and `gen-protocol-check` honest.
NO_EXPORT = 'TS_RS_EXPORT_DIR="$(mktemp -d)" '


class ToolingError(Exception):
    pass


def git(root: Path, *args: str) -> list[str]:
    """Run git with NUL-separated output (`-z`), so unusual file names come back unquoted."""
    try:
        out = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True, check=True)
    except (OSError, subprocess.CalledProcessError) as err:
        detail = err.stderr.strip() if isinstance(err, subprocess.CalledProcessError) else str(err)
        raise ToolingError(f"git {' '.join(args)} failed: {detail}") from err
    return [p for p in out.stdout.split("\0") if p]


def is_commit(root: Path, ref: str) -> bool:
    proc = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"],
        capture_output=True,
        text=True,
    )
    return proc.returncode == 0


def is_base(root: Path, word: str) -> bool:
    if ".." in word:
        start, _, end = word.partition("..")
        return bool(start) and is_commit(root, start) and is_commit(root, end or "HEAD")
    return is_commit(root, word)


def repo_root() -> Path:
    here = Path(__file__).resolve().parent
    proc = subprocess.run(["git", "-C", str(here), "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    return Path(proc.stdout.strip()) if proc.returncode == 0 else here.parents[3]


def normal_task(word: str) -> str | None:
    m = TASK_RE.match(word.strip(",;:."))
    if not m:
        return None
    return f"P{m[1]}.T{m[2]}" + (f".{m[3].lower()}" if m[3] else "")


def parse_args(root: Path, argv: list[str]) -> tuple[str | None, str, str | None, list[str]]:
    """Returns the task ID, the base, the plan set and warnings about the arguments."""
    if "--args-stdin" in argv:
        text = sys.stdin.read()
        try:
            extra = shlex.split(text)
        except ValueError:
            extra = text.split()
        argv = [a for a in argv if a != "--args-stdin"] + extra
    task, base, feature = None, None, None
    warnings: list[str] = []
    ignored: list[str] = []
    it = iter(argv)
    for word in it:
        flag, eq, value = word.partition("=")
        if flag in ("--base", "--feature"):
            if not eq:
                value = next(it, "")
            if not value:
                warnings.append(f"`{flag}` was given without a value; ignored.")
            elif flag == "--base":
                base = value
            else:
                feature = value
        elif found := normal_task(word):
            if task and found != task:
                warnings.append(f"Several task IDs given; using {task}, ignoring {found}.")
            else:
                task = found
        elif base is None and is_base(root, word):
            base = word
        else:
            ignored.append(word)
    if ignored:
        warnings.append("Ignored arguments: " + " ".join(ignored))
    return task, base or "HEAD", feature, warnings


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


def changes(root: Path, base: str) -> tuple[list[str], set[str], str]:
    """The changed paths, the deleted ones, and a description of what was compared."""
    if not is_base(root, base):
        raise ToolingError(f"--base {base!r} is not a commit or an A..B range of commits in {root}.")
    if ".." in base:
        start, _, end = base.partition("..")
        end = end or "HEAD"
        changed = git(root, "diff", "-z", "--name-only", start, end)
        deleted = set(git(root, "diff", "-z", "--name-only", "--diff-filter=D", start, end))
        return sorted(set(changed)), deleted, f"the commits {start}..{end} (checks still run on the working tree)"
    changed = git(root, "diff", "-z", "--name-only", base) + git(root, "ls-files", "-z", "--others", "--exclude-standard")
    deleted = set(git(root, "diff", "-z", "--name-only", "--diff-filter=D", base))
    return sorted(set(changed)), deleted, f"the working tree against {base}, untracked files included"


def main() -> None:
    root = repo_root()
    task, base, feature, warnings = parse_args(root, sys.argv[1:])
    print(f"Repository: {root}")
    try:
        changed, deleted, compared = changes(root, base)
    except ToolingError as err:
        print(f"TOOLING ERROR: {err}")
        print("No check plan was made. Report this; the caller must fix the arguments.")
        return
    print(f"Changes: {compared}")
    if task:
        print(f"Task: {task}" + (f" (plan set {feature})" if feature else ""))
    for warning in warnings:
        print(f"Note: {warning}")
    print()

    if not changed and task is None:
        print("No changed files. Nothing to validate unless the caller asked for the full gate:")
        print(f"- `{NO_EXPORT}just ci`")
        return

    rust = [f for f in changed if RUST_FILE.search(f)]
    ts = [f for f in changed if TS_FILE.search(f) and not f.startswith(GENERATED)]
    prettier = [f for f in changed if not PRETTIER_SKIP.search(f)]
    goldens = [f for f in changed if "/tests/golden/" in f]
    crates = sorted({c for f in rust + goldens if (c := crate_of(f))})
    workspace_wide = any(WORKSPACE_WIDE.search(f) for f in changed)
    infra = [f for f in changed if INFRA.search(f)]
    protocol = [f for f in changed if f.startswith(("crates/hyperion-protocol/", GENERATED))]
    client = bool(ts or protocol)
    determinism = [f for f in changed if crate_of(f) in DETERMINISM_CRATES] or (workspace_wide and bool(rust))
    benches = sorted({f for f in changed if re.match(r"crates/[^/]+/benches/[^/]+\.rs$", f)})
    manifests = [f for f in changed if f.endswith(("package.json", "pnpm-lock.yaml"))]
    ux_guide = "docs/frontend/ux-guidelines.md" in changed

    areas = []
    if rust or goldens:
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
    if protocol:
        tier1.append(("just gen-protocol-check", "protocol types or bindings changed; runs before any test can rewrite them"))
    if rust:
        tier1.append(("cargo fmt --all -- --check", "Rust files changed"))
        tier1.append(("cargo clippy --workspace --all-targets -- -D warnings", "Rust files changed; pedantic, warnings are errors"))
    if workspace_wide and rust:
        tier1.append((f"{NO_EXPORT}cargo test --workspace", "workspace-wide Rust config changed"))
    else:
        for crate in crates:
            prefix = NO_EXPORT if crate == "hyperion-protocol" else ""
            tier1.append((f"{prefix}cargo test -p {crate}", f"{crate} changed"))
    for bench in benches:
        tier1.append((f"cargo bench -p {crate_of(bench)} --bench {Path(bench).stem} -- --test", "benchmark changed; runs each once"))
    if prettier:
        tier1.append(("pnpm format:check", "Prettier owns TS, CSS, JSON, YAML and Markdown formatting"))
    if client:
        tier1.append(("pnpm typecheck", "TypeScript or the bindings it imports changed"))
    if ts:
        tier1.append(("pnpm lint", "oxlint --type-aware --deny-warnings"))
    if client:
        tier1.append(("pnpm test", "TypeScript or the bindings it imports changed"))

    if any(cmd.startswith(NO_EXPORT) for cmd, _ in tier1):
        print(f"Commands starting `{NO_EXPORT.strip()}` send ts-rs's binding export to a scratch")
        print("directory, so the tests don't rewrite the checked-in bindings. Run them exactly as written.\n")
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
            cmd = [sys.executable, "-B", str(script), task, "--acceptance"] + (["--feature", feature] if feature else [])
            out = subprocess.run(cmd, capture_output=True, text=True, cwd=root)
            if out.returncode == 0:
                print(out.stdout.strip())
                print("\n(A command listed here and in tier 3, such as `just ci`, runs once, in tier 3.)")
            else:
                print(f"TOOLING ERROR: plan_task.py exited {out.returncode}; report it, and read the task's")
                print(f"acceptance criteria in its plan by hand. Its message: {(out.stderr or out.stdout).strip()}")
        else:
            print(f"TOOLING ERROR: {script} not found; read {task}'s acceptance criteria in its plan by hand.")
    else:
        print("(no task ID given)")
    print()

    gate: list[tuple[str, str]] = [
        (f"{NO_EXPORT}just ci", "CI's Rust job (fmt, check, clippy, tests, slow tests, bindings) and the frontend "
         "job's format, typecheck, lint and test")
    ]
    skipped: list[str] = []
    if client or infra:
        gate.append(("pnpm build", "CI's frontend job also builds the client"))
    if determinism:
        ok, reason = wasm_available()
        if ok:
            gate.append(("just test-wasm", "sim, testkit or fit changed; goldens must hold bit for bit on wasm32"))
        else:
            skipped.append(f"`just test-wasm`: {reason}")
        skipped.append("AArch64 golden run: CI only (`rust-aarch64` job)")
    if manifests:
        skipped.append("`pnpm install --frozen-lockfile` (CI's frontend job): it would change node_modules. Check "
                       "instead that pnpm-lock.yaml changed together with any package.json dependency change")
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
