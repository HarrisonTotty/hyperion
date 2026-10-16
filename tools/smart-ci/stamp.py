#!/usr/bin/env python3
"""Gate stamps: the record that `just smart-ci` passed on an exact tree, and the git hooks that honour one.

Usage: stamp.py check <commit-or-tree>
       stamp.py hook <check> -- <command...>

When `just smart-ci` (`smart_ci.py`) passes, it writes a stamp keyed by the tree it checked: the git
tree id of the working tree as it stood, uncommitted and untracked files included and ignored ones
not, which `git add -A` into a copy of the index and `git write-tree` give (`worktree_tree`; the id
that gh-collab's stamped gate log records as `tree=`, so that `gate run` stamps through smart-ci).
Any change of content, an untracked file's included, gives another id. The stamp,
`<git common dir>/smart-ci/stamps/<tree>.json`, is shared by every worktree of the clone and records
each passing run: HEAD, the base it diffed against (the merge-base and the ref named), the plan's
steps with every path that caused each, the changed paths, and, read when the run started, the
toolchain (rustc, cargo, node, pnpm), the environment that can change what the checks find, cargo's
own configuration and the installed node_modules' lock, and the time. A failed run writes none and
withdraws the tree's earlier stamp; a run during which the working tree changed writes none.

`hook` runs one of the git hooks' expensive checks (`.pre-commit-config.yaml`). It skips the check,
printing why in one line, when a valid stamp of the tree being committed (the index's) or pushed (the
pushed commit's, for `test`) covers it and the working tree that the check would read is that tree,
and otherwise runs the command exactly as the hook did. Its output is printed only if it fails, as
pre-commit shows a passing hook's only when it is verbose, which the wrapped hooks are for the line
saying which. Any error in reading the stamps runs the command. What covers each check:

  clippy             a full plan, or one that ran the hook's own command, the workspace's Clippy
                     (`_clippy-for --workspace`, which `just smart-ci --hook-checks` runs in place
                     of the changed crates'): a `-p` selection is not the workspace's build (outside
                     `--workspace` the dependencies resolve with fewer features), nor does
                     smart-ci's lint the crates that depend on the change
  oxlint, typecheck  a plan that ran `_oxlint` or `_typecheck-ts`, which are the hooks' whole-tree runs
  protocol-bindings  a full plan, or one that ran the hook's own command, the bindings' check over the
                     workspace (`_gen-protocol-check-for --workspace`, with `--hook-checks`), which
                     builds, links and runs every crate's library tests
  test               any passing plan (`just test`, on push): the tests that reach the change, by
                     smart-ci's graph, ran, and the rest are as the base's earlier gates left them

A stamp is valid when its base is an ancestor of `origin/rendering-and-planets` (SMART_CI_BASE names
another branch): pushed by integration after its smart-ci gate, and checked later by the owner's
full CI, so that what the plan did not check is as those left it; any base for a full plan. It is
valid with the toolchain, the environment and the configuration it recorded, and when its change
edits no file of smart-ci's own, whose rules would decide their own coverage. `SMART_CI_NO_STAMPS=1` runs every check whatever the
stamps (`just pre-commit` sets it).

`check` prints whether a valid passing stamp exists for a commit's or a tree's tree, against which
base, and which hooks it covers: exit 0 if one does, 1 if not.

Standard library only, Python 3.9 or later, and nothing imported from beside it, since the hooks
run it as `python3 -I`.
"""

from __future__ import annotations

import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, List, Mapping, Optional, Sequence, Tuple

sys.dont_write_bytecode = True

VERSION = 1
# The runs a tree's stamp keeps, newest last, and the age past which stamps are pruned.
KEEP_RUNS = 8
KEEP_DAYS = 30
# The integration branch, as smart-ci's default base (`smart_ci.INTEGRATION_BRANCH`), and the remote
# whose copy of it is integrated work: what the orchestrator pushed after its gate.
INTEGRATION_DEFAULT = "rendering-and-planets"
REMOTE = "origin"
OFF_VAR = "SMART_CI_NO_STAMPS"
TREE_ID = re.compile(r"^[0-9a-f]{40}([0-9a-f]{24})?$")
# smart-ci's own files: a change to them decides its own coverage with the rules it changes.
OWN_FILES = "tools/smart-ci/"

# The tools whose versions a stamp records, each with the command that prints its version.
TOOLS: Dict[str, Tuple[str, ...]] = {
    "rustc": ("rustc", "--version"),
    "cargo": ("cargo", "--version"),
    "node": ("node", "--version"),
    "pnpm": ("pnpm", "--version"),
    # `gen-surface`, which the TypeScript checks and `just test` run first, and `just test`'s runner.
    "wasm-bindgen": ("wasm-bindgen", "--version"),
    "nextest": ("cargo", "nextest", "--version"),
}
# The environment that can change what a check finds: every variable of cargo, rustc, Clippy,
# nextest, proptest and HYPERION's own, and Node's, vitest's and the locale's, less those that change
# only how fast, where or how verbosely they run, the toolchain's choice (its version is compared
# instead) and credentials. Recorded by a digest of each value, never the value.
ENVIRONMENT = re.compile(r"^(RUST|CARGO|CLIPPY|NEXTEST|PROPTEST|HYPERION_|VITEST|LC_)|^(NODE_OPTIONS|NODE_ENV|CI|TZ|LANG)$")
HARMLESS = re.compile(r"^(CARGO_TERM_\w*|CARGO_BUILD_JOBS|CARGO_TARGET_DIR|CARGO_BUILD_TARGET_DIR|CARGO_INCREMENTAL|"
                      r"CARGO_HTTP_\w*|CARGO_NET_\w*|CARGO_REGISTRY_TOKEN|CARGO_REGISTRIES_\w*_TOKEN|RUSTUP_\w*|"
                      r"RUST_BACKTRACE|RUST_LIB_BACKTRACE|RUST_LOG|RUST_TEST_THREADS|NEXTEST_TEST_THREADS|"
                      r"HYPERION_HEAVY_\w*)$")


@dataclass(frozen=True)
class Check:
    """One of the hooks' checks: what it is, its git stage, and the tools it runs."""

    what: str
    stage: str
    tools: Tuple[str, ...]


CHECKS: Dict[str, Check] = {
    "clippy": Check("the workspace's Clippy", "pre-commit", ("rustc", "cargo")),
    # The TypeScript checks run `gen-surface` first, which builds the surface module with cargo.
    "oxlint": Check("oxlint", "pre-commit", ("rustc", "cargo", "wasm-bindgen", "node", "pnpm")),
    "typecheck": Check("tsc", "pre-commit", ("rustc", "cargo", "wasm-bindgen", "node", "pnpm")),
    "protocol-bindings": Check("the protocol bindings' check", "pre-commit", ("rustc", "cargo")),
    "test": Check("`just test`", "pre-push", tuple(TOOLS)),
}
# The recipes of a smart-ci plan that run a hook's check as the hook runs it: oxlint and tsc are
# whole-tree runs, and Clippy and the bindings' check the workspace's (`_clippy` is
# `_clippy-for "--workspace"`, `gen-protocol-check` `_gen-protocol-check-for "--workspace"`).
HOOK_STEPS: Dict[str, List[str]] = {"oxlint": ["_oxlint"], "typecheck": ["_typecheck-ts"],
                                    "clippy": ["_clippy-for", "--workspace"],
                                    "protocol-bindings": ["_gen-protocol-check-for", "--workspace"]}


class StampError(Exception):
    pass


def integration_branch(env: Mapping[str, str] = os.environ) -> str:
    return env.get("SMART_CI_BASE") or INTEGRATION_DEFAULT


def git(root: Path, *args: str, env: Optional[Mapping[str, str]] = None) -> str:
    try:
        proc = subprocess.run(["git", *args], cwd=str(root), capture_output=True, text=True,
                              env=dict(env) if env is not None else None)
    except OSError as err:
        raise StampError(f"git: {err}") from err
    if proc.returncode != 0:
        raise StampError(f"`git {' '.join(args)}` failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def _absolute(root: Path, text: str) -> Path:
    path = Path(text)
    return path if path.is_absolute() else root / path


def stamps_dir(root: Path) -> Path:
    """`<git common dir>/smart-ci/stamps`, which every worktree of the clone shares."""
    return _absolute(root, git(root, "rev-parse", "--git-common-dir")) / "smart-ci" / "stamps"


def worktree_tree(root: Path) -> str:
    """The tree of the working tree as it stands, uncommitted and untracked files included, ignored ones
    not: `git add -A` into a copy of the index, then `git write-tree`, so that the index is untouched.
    gh-collab's gate log computes the same (`ghcollab.worktree_tree`)."""
    index = _absolute(root, git(root, "rev-parse", "--git-path", "index"))
    temp = _absolute(root, git(root, "rev-parse", "--git-path", f"smart-ci-index-{os.getpid()}"))
    env = {**os.environ, "GIT_INDEX_FILE": str(temp)}
    try:
        if index.exists():
            shutil.copyfile(index, temp)
        git(root, "add", "-A", env=env)
        return git(root, "write-tree", env=env)
    except OSError as err:
        raise StampError(f"the working tree's id: {err}") from err
    finally:
        try:
            temp.unlink()
        except OSError:
            pass


def hidden_entries(root: Path) -> List[str]:
    """The index's entries whose working-tree edits git does not see (assume-unchanged, skip-worktree),
    which the tree's id would leave out although cargo and node read them."""
    hidden = []
    for line in git(root, "ls-files", "-v").splitlines():
        tag, _, path = line.partition(" ")
        if tag == "S" or tag.islower():
            hidden.append(path)
    return hidden


def index_tree(root: Path) -> str:
    """The tree a commit would record: the index's (`GIT_INDEX_FILE`'s, inside a hook)."""
    return git(root, "write-tree")


def tree_of(root: Path, rev: str) -> str:
    """The tree of a commit, a tree or any name of one."""
    try:
        return git(root, "rev-parse", "--verify", "--quiet", f"{rev}^{{tree}}")
    except StampError:
        raise StampError(f"{rev!r} names no commit or tree here") from None


def tool_version(root: Path, name: str) -> str:
    """What `<tool> --version` prints from the repository's root (where rust-toolchain.toml applies); empty
    if it is missing or fails."""
    command = TOOLS[name]
    exe = shutil.which(command[0])
    if exe is None:
        return ""
    try:
        proc = subprocess.run([exe, *command[1:]], cwd=str(root), capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError):
        return ""
    return proc.stdout.strip() if proc.returncode == 0 else ""


def toolchain(root: Path, names: Sequence[str] = tuple(TOOLS)) -> Dict[str, str]:
    return {name: tool_version(root, name) for name in names}


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:16]


def environment(env: Mapping[str, str] = os.environ) -> Dict[str, str]:
    """The variables that can change what a check finds (`ENVIRONMENT`), each with its value's digest."""
    return {name: _digest(value.encode("utf-8", "surrogateescape")) for name, value in sorted(env.items())
            if ENVIRONMENT.match(name) and not HARMLESS.match(name)}


def _read(path: Path) -> Optional[bytes]:
    try:
        return path.read_bytes()
    except OSError:
        return None


def configuration(root: Path, env: Mapping[str, str] = os.environ) -> Dict[str, str]:
    """The digests of what the tree does not hold but the checks read: cargo's configuration in
    CARGO_HOME and in the directories above the tree, which cargo merges with the tree's own
    (rustflags, a target, profiles), and pnpm's record of the node_modules installed. A configuration
    above the tree with the bytes of the tree's own counts as none, so that worktrees nested under
    the primary checkout match it while both have the same."""
    home = Path(env.get("CARGO_HOME") or Path.home() / ".cargo")
    found = {}
    for key, path in (("cargo home config.toml", home / "config.toml"), ("cargo home config", home / "config"),
                      ("node_modules", root / "node_modules" / ".pnpm" / "lock.yaml")):
        data = _read(path)
        if data is not None:
            found[key] = _digest(data)
    try:
        home_dir = home.resolve()
        parents = [directory / ".cargo" for directory in root.resolve().parents]
    except OSError:
        return {**found, "cargo configs above the tree": "unreadable"}
    above = []
    for directory in parents:
        if directory == home_dir:
            continue
        for name in ("config.toml", "config"):
            data = _read(directory / name)
            if data is not None and data != _read(root / ".cargo" / name):
                above.append(_digest(data))
    if above:
        found["cargo configs above the tree"] = ",".join(sorted(above))
    return found


def now_text() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def start(root: Path) -> Dict[str, object]:
    """What a run is checked with, read before it starts: the working tree's id, and the toolchain,
    environment and configuration that the stamp records. Raises StampError if the tree cannot be
    stamped: an id that cannot be read, or entries whose edits git does not see."""
    hidden = hidden_entries(root)
    if hidden:
        raise StampError(f"{len(hidden)} index entries are assume-unchanged or skip-worktree ({hidden[0]}), "
                         "whose edits the tree's id would leave out")
    return {"tree": worktree_tree(root), "toolchain": toolchain(root), "environment": environment(),
            "configuration": configuration(root)}


# --------------------------------------------------------------------------------------------------
# The stamps.


def _checked(tree: str) -> str:
    if not TREE_ID.match(tree):
        raise StampError(f"not a tree id: {tree!r}")
    return tree


def stamp_path(root: Path, tree: str) -> Path:
    return stamps_dir(root) / f"{_checked(tree)}.json"


def runs_of(root: Path, tree: str) -> List[dict]:
    """The passing runs a tree's stamp records, oldest first; none if it has no stamp."""
    try:
        data = json.loads(stamp_path(root, tree).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []
    if not isinstance(data, dict) or data.get("version") != VERSION or data.get("tree") != tree:
        return []
    return [run for run in data.get("runs", []) if isinstance(run, dict) and run.get("tree") == tree
            and run.get("exit") == 0]


def prune(directory: Path, now: Optional[float] = None) -> None:
    cutoff = (time.time() if now is None else now) - KEEP_DAYS * 86400
    for path in [*directory.glob("*.json"), *directory.glob(".*.tmp")]:
        try:
            if path.stat().st_mtime < cutoff:
                path.unlink()
        except OSError:
            pass


def write(root: Path, run: dict) -> Path:
    """Add a passing run to its tree's stamp, which keeps the last KEEP_RUNS."""
    tree = _checked(run["tree"])
    directory = stamps_dir(root)
    directory.mkdir(parents=True, exist_ok=True)
    prune(directory)
    runs = (runs_of(root, tree) + [run])[-KEEP_RUNS:]
    path = directory / f"{tree}.json"
    temp = directory / f".{tree}.{os.getpid()}.tmp"
    try:
        temp.write_text(json.dumps({"version": VERSION, "tree": tree, "runs": runs}, indent=1, sort_keys=True) + "\n",
                        encoding="utf-8")
        os.replace(temp, path)
    finally:
        try:
            temp.unlink()
        except OSError:
            pass
    return path


def withdraw(root: Path, tree: str) -> bool:
    """Remove a tree's stamp; whether there was one."""
    try:
        stamp_path(root, tree).unlink()
    except FileNotFoundError:
        return False
    return True


def record(root: Path, before: Optional[str], status: int, make_run: Callable[[str], dict],
           tree: Callable[[Path], str] = worktree_tree) -> str:
    """After a smart-ci run over the working tree whose id was `before`: stamp it if the run passed and
    the tree is as it was, withdraw its stamp if the run failed. The line to print."""
    if before is None:
        return "smart-ci: no stamp: the tree it checked could not be stamped"
    if status != 0:
        try:
            withdrawn = withdraw(root, before)
        except (StampError, OSError):
            withdrawn = False
        return f"smart-ci: no stamp (exit {status})" + (f"; withdrew tree {before[:10]}'s earlier one" if withdrawn else "")
    try:
        after = tree(root)
        if after != before:
            return f"smart-ci: no stamp: the working tree changed during the run ({before[:10]}, then {after[:10]})"
        path = write(root, make_run(before))
    except (StampError, OSError) as err:
        return f"smart-ci: no stamp: {err}"
    return f"smart-ci: stamped tree {before[:10]}, so the git hooks skip the checks this run covers ({path})"


# --------------------------------------------------------------------------------------------------
# What a stamp covers, and whether it is valid.


def _recipes(run: Mapping[str, object]) -> List[List[str]]:
    """The recipes, with their arguments, of a run's steps."""
    steps = run.get("steps")
    if not isinstance(steps, list):
        return []
    return [list(step["recipe"]) for step in steps if isinstance(step, dict) and isinstance(step.get("recipe"), list)
            and step["recipe"]]


def covers(run: Mapping[str, object], check: str) -> Tuple[bool, str]:
    """Whether a passing run's plan covers a hook's check, and why."""
    if run.get("full"):
        return True, "the full `just ci` passed on it"
    changes = run.get("changes")
    if not isinstance(changes, list):
        return False, "its stamp does not list the change"
    own = sorted(str(c[0]) for c in changes if isinstance(c, list) and c and str(c[0]).startswith(OWN_FILES))
    if own:
        return False, f"its change edits smart-ci's own rules ({own[0]}), which covers nothing but a full plan"
    if check == "test":
        return True, "smart-ci passed on it, with the tests that reach the change"
    recipes = _recipes(run)
    wanted = HOOK_STEPS[check]
    if any(recipe[:len(wanted)] == wanted for recipe in recipes):
        return True, f"its plan ran {CHECKS[check].what}" + (" as the hook does" if len(wanted) > 1 else "")
    if any(recipe[0] == wanted[0] for recipe in recipes):
        return False, (f"its plan ran {CHECKS[check].what} over its own selection, not the workspace as the hook "
                       "does (`just smart-ci --hook-checks` runs the hook's)")
    return False, f"its plan ran no {CHECKS[check].what}"


def integrated(root: Path, base: str, env: Mapping[str, str] = os.environ) -> bool:
    """Whether a commit is an ancestor of the remote's integration branch: pushed, so checked."""
    if not TREE_ID.match(base or ""):
        return False
    ref = f"refs/remotes/{REMOTE}/{integration_branch(env)}"
    try:
        return subprocess.run(["git", "merge-base", "--is-ancestor", base, ref], cwd=str(root),
                              capture_output=True).returncode == 0
    except OSError:
        return False


@dataclass
class Now:
    """What a check would run with now, read once and only when needed."""

    root: Path
    env: Mapping[str, str]
    probe: Callable[[Path, Sequence[str]], Dict[str, str]] = toolchain
    _tools: Optional[Dict[str, str]] = None

    def tools(self) -> Dict[str, str]:
        if self._tools is None:
            self._tools = self.probe(self.root, tuple(TOOLS))
        return self._tools


def problem_with(run: Mapping[str, object], tools: Sequence[str], now: Now) -> Optional[str]:
    """Why a run's stamp is not valid for a check that uses `tools` now; None if it is."""
    if not run.get("full"):
        base = str(run.get("base") or "")
        if not integrated(now.root, base, now.env):
            return (f"its base {base[:10] or '(none)'} is not an ancestor of {REMOTE}/{integration_branch(now.env)} "
                    "(not yet integrated)")
    recorded = run.get("toolchain")
    recorded = recorded if isinstance(recorded, dict) else {}
    current = now.tools()
    missing = [name for name in tools if not current.get(name)]
    if missing:
        return f"{', '.join(missing)} cannot be run here"
    changed = [name for name in tools if recorded.get(name) != current.get(name)]
    if changed:
        return "the toolchain has changed since: " + "; ".join(
            f"{name} {recorded.get(name) or 'none'}, now {current.get(name)}" for name in changed)
    then = run.get("environment")
    then = then if isinstance(then, dict) else None
    current_environment = environment(now.env)
    if then != current_environment:
        names = sorted(n for n in set(then or {}) | set(current_environment)
                       if (then or {}).get(n) != current_environment.get(n))
        return f"the environment differs from its run's ({', '.join(names) or 'its record is unreadable'})"
    if run.get("configuration") != configuration(now.root, now.env):
        return "cargo's configuration in CARGO_HOME, or the node_modules installed, differ from its run's"
    return None


@dataclass
class Decision:
    covered: bool
    reason: str


def describe_run(run: Mapping[str, object]) -> str:
    when = str(run.get("time", "")).replace("T", " ").replace("Z", " UTC")
    if run.get("full"):
        return f"the full `just ci`, {when}"
    return f"against {str(run.get('base', ''))[:10]} (its merge-base with {run.get('base_ref', '?')}), {when}"


def decide(root: Path, check: str, env: Mapping[str, str] = os.environ,
           probe: Callable[[Path, Sequence[str]], Dict[str, str]] = toolchain) -> Decision:
    """Whether a valid stamp of the tree being committed or pushed covers the check."""
    spec = CHECKS[check]
    if env.get(OFF_VAR) == "1":
        return Decision(False, f"{OFF_VAR}=1")
    if spec.stage == "pre-push":
        pushed = env.get("PRE_COMMIT_TO_REF", "")
        if not pushed:
            return Decision(False, "no pushed commit is named (PRE_COMMIT_TO_REF)")
        tree, what = tree_of(root, pushed), f"the pushed commit {pushed[:10]}'s tree"
    else:
        tree, what = index_tree(root), "the tree being committed"
    runs = runs_of(root, tree)
    if not runs:
        return Decision(False, f"smart-ci has not passed on {what}, {tree[:10]}")
    now = Now(root, env, probe)
    reasons = []
    read: Dict[str, object] = {}
    for run in reversed(runs):
        ok, why = covers(run, check)
        if not ok:
            reasons.append(why)
            continue
        if "worktree" not in read:
            # The check reads the working tree (pre-commit stashes only unstaged edits, and only on a
            # commit): it is the stamped tree, or the check runs.
            hidden = hidden_entries(root)
            read["worktree"] = "hidden" if hidden else worktree_tree(root)
        if read["worktree"] != tree:
            reasons.append("the working tree the check would read is not that tree" if read["worktree"] != "hidden"
                           else "some index entries are assume-unchanged or skip-worktree")
            continue
        problem = problem_with(run, spec.tools, now)
        if problem is None:
            return Decision(True, f"smart-ci passed on {what}, {tree[:10]}, {describe_run(run)}: {why}")
        reasons.append(problem)
    return Decision(False, f"smart-ci passed on {what}, {tree[:10]}, but {reasons[0]}")


def run_hook(root: Path, check: str, command: Sequence[str], env: Mapping[str, str] = os.environ) -> int:
    """The hook: skip the check if a stamp covers it, else run `command` as the hook did."""
    try:
        decision = decide(root, check, env)
    except Exception as err:  # noqa: BLE001  (whatever goes wrong here, the check runs)
        decision = Decision(False, f"the stamps could not be read: {err}")
    if decision.covered:
        print(f"{check}: skipped, {decision.reason}", flush=True)
        return 0
    print(f"{check}: not covered by a gate stamp ({decision.reason}), so it runs: {' '.join(command)}", flush=True)
    exe = shutil.which(command[0]) or command[0]
    with subprocess.Popen([exe, *command[1:]], cwd=str(root), stdout=subprocess.PIPE, stderr=subprocess.STDOUT) as proc:
        try:
            output, _ = proc.communicate()
        except KeyboardInterrupt:
            proc.wait()  # it had the same interrupt: let it clean up (the heavy-test lock among it)
            raise
    if proc.returncode != 0:
        sys.stdout.flush()
        out = getattr(sys.stdout, "buffer", None)
        if out is not None:
            out.write(output)
            out.flush()
        else:
            sys.stdout.write(output.decode("utf-8", "replace"))
    return proc.returncode


def check_tree(root: Path, rev: str, env: Mapping[str, str] = os.environ,
               probe: Callable[[Path, Sequence[str]], Dict[str, str]] = toolchain) -> Tuple[int, List[str]]:
    """`stamp.py check`: whether a valid passing stamp exists for `rev`'s tree, one line a run, each with
    the hooks it covers here, as each would decide on that tree."""
    tree = tree_of(root, rev)
    named = f"tree {tree[:10]}" + ("" if tree.startswith(rev) else f" ({rev})")
    runs = runs_of(root, tree)
    if not runs:
        return 1, [f"no stamp: smart-ci has not passed on {named}"]
    now = Now(root, env, probe)
    lines, status = [], 1
    for run in reversed(runs):
        problem = problem_with(run, tuple(TOOLS), now)
        hooks = [name for name, spec in CHECKS.items() if covers(run, name)[0]
                 and problem_with(run, spec.tools, now) is None]
        if problem is None:
            status = 0
        lines.append(f"{'pass' if problem is None else 'stale'}: smart-ci passed on {named} {describe_run(run)}; "
                     f"it covers the hooks' {', '.join(hooks) or 'none'}" + (f"; but {problem}" if problem else ""))
    return status, lines


def repo_root() -> Path:
    return Path(git(Path.cwd(), "rev-parse", "--show-toplevel"))


USAGE = "usage: stamp.py check <commit-or-tree> | stamp.py hook <check> -- <command...>"


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if args[:1] == ["check"] and len(args) == 2:
        try:
            status, lines = check_tree(repo_root(), args[1])
        except StampError as err:
            print(f"error: {err}", file=sys.stderr)
            return 2
        print("\n".join(lines))
        return status
    if args[:1] == ["hook"] and len(args) >= 4 and args[1] in CHECKS and args[2] == "--":
        try:
            root = repo_root()
        except StampError:
            root = Path.cwd()
        return run_hook(root, args[1], args[3:])
    print(USAGE + "\n  checks: " + ", ".join(CHECKS), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
