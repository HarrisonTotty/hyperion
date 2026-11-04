#!/usr/bin/env python3
"""`just repin`: re-pin the figures that tests pin (`hyperion_testkit::pins`), by test name.

Usage: repin.py <test name>...
       repin.py --check [<test name>...]

A test name is a test function's name, the last part of its path. Every test of the workspace so
named runs alone, one at a time, at 16 test threads (`--test-threads 16`, which a test that runs
threads of its own may take, `hyperion_testkit::threads`), under nextest: a slow one
(`#[ignore = "slow: …"]`) under the slow-test profile, as `just test-slow` runs it, and every other
under the test profile. Each runs with `HYPERION_BLESS=1`, so that each pin writes the value it
receives to its pins file, `crates/<crate>/tests/pins/<file>.pins`, where comparing would fail (the
file lacks the key, or holds a value the received one does not match within the pin's tolerance),
and with `HYPERION_PINS_LOG` naming a file each pin lists its key in. Once all a name's tests have
passed, the keys under that name that none of them pinned are dropped from every pins file, and a
file left with none is deleted. It ends by listing every pin it added, changed or dropped, old value
and new, and every golden file the tests blessed beside their pins (a test that also checks a golden
blesses it, as `HYPERION_BLESS=1 cargo test <name>` would).

`--check` runs the tests in the same way without blessing, so that a stale or missing pin fails, and
fails too on a key under a test's name that the test no longer pins. With no names it checks every
test that a pins file names, and fails on a name no test has.

The listing (`cargo nextest list`) builds the workspace's tests first. Standard library only,
Python 3.9 or later, as `tools/smart-ci` is.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Dict, List, NamedTuple, Optional, Sequence, Set, Tuple

TEST_THREADS = "16"
PINS_GLOB = "crates/*/tests/pins/**/*.pins"
HEADER_PREFIX = "# Pinned figures"


class Test(NamedTuple):
    binary_id: str
    name: str
    ignored: bool


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def listed_tests(root: Path) -> Dict[str, List[Test]]:
    """Every test of the workspace that nextest's default filter keeps, by its function's name."""
    out = subprocess.run(
        ["cargo", "nextest", "list", "--workspace", "--run-ignored", "all", "--message-format", "json"],
        cwd=root, stdout=subprocess.PIPE, check=True).stdout
    by_name: Dict[str, List[Test]] = {}
    for binary_id, suite in sorted(json.loads(out)["rust-suites"].items()):
        for name, case in sorted(suite["testcases"].items()):
            if case["filter-match"]["status"] == "matches":
                by_name.setdefault(name.rsplit("::", 1)[-1], []).append(Test(binary_id, name, case["ignored"]))
    return by_name


def pins_files(root: Path) -> List[Path]:
    return sorted(root.glob(PINS_GLOB))


def keys_of(path: Path) -> List[str]:
    """The keys of a pins file, `<test>.<key>`, in order (the testkit's tests hold its form)."""
    lines = path.read_text(encoding="utf-8").split("\n")
    return [line.split(" = ", 1)[0] for line in lines[1:] if line]


def run(root: Path, test: Test, bless: bool, log: Path) -> bool:
    profile = ["--cargo-profile", "slow-test", "--profile", "slow"] if test.ignored else []
    env = dict(os.environ, HYPERION_PINS_LOG=str(log))
    env.pop("HYPERION_BLESS", None)
    env.pop("HYPERION_PINS_KEEP", None)
    if bless:
        env["HYPERION_BLESS"] = "1"
    command = ["cargo", "nextest", "run", "--workspace", *profile, "--run-ignored", "all",
               "--test-threads", TEST_THREADS, "--no-tests", "fail",
               "-E", f"binary_id(={test.binary_id}) & test(={test.name})"]
    print(f"repin: {'blessing' if bless else 'checking'} {test.binary_id} {test.name}"
          f"{' (slow)' if test.ignored else ''}", file=sys.stderr, flush=True)
    return subprocess.run(command, cwd=root, env=env).returncode == 0


def logged(log: Path) -> Set[Tuple[str, str]]:
    """The (file, key) pairs the log lists, each file's path resolved."""
    if not log.exists():
        return set()
    pairs = set()
    for line in log.read_text(encoding="utf-8").splitlines():
        path, key = line.split("\t", 1)
        pairs.add((os.path.realpath(path), key))
    return pairs


def unpinned(root: Path, names: Sequence[str], read: Set[Tuple[str, str]]) -> Dict[Path, List[str]]:
    """The keys under `names`, in every pins file, that the log does not list."""
    out: Dict[Path, List[str]] = {}
    for path in pins_files(root):
        real = os.path.realpath(path)
        stale = [key for key in keys_of(path)
                 if key.split(".", 1)[0] in names and (real, key) not in read]
        if stale:
            out[path] = stale
    return out


def drop(path: Path, keys: Sequence[str]) -> None:
    """Removes the lines of `keys` from the pins file, deleting the file if no pin is left."""
    lines = path.read_text(encoding="utf-8").split("\n")
    kept = [line for line in lines[1:] if line and line.split(" = ", 1)[0] not in keys]
    if kept:
        path.write_bytes(("\n".join([lines[0], *kept]) + "\n").encode("utf-8"))
    else:
        path.unlink()


def pins_of(path: Path) -> Dict[str, str]:
    """A pins file's values' text by key, empty if there is no file."""
    if not path.exists():
        return {}
    lines = path.read_text(encoding="utf-8").split("\n")
    return dict(line.split(" = ", 1) for line in lines[1:] if " = " in line)


def goldens(root: Path) -> Dict[Path, Tuple[int, int]]:
    """Every golden file's size and modification time."""
    return {path: (path.stat().st_size, path.stat().st_mtime_ns)
            for path in root.glob("crates/*/tests/golden/**/*.golden")}


def report_moves(root: Path, before: Dict[Path, Dict[str, str]]) -> None:
    """Lists each pin that the run added, changed or dropped, with its old and new values."""
    for path in sorted(set(before) | set(pins_files(root))):
        old, new = before.get(path, {}), pins_of(path)
        if old == new:
            continue
        print(f"repin: {path.relative_to(root).as_posix()}:", file=sys.stderr)
        for key in sorted(set(old) | set(new)):
            if old.get(key) != new.get(key):
                print(f"  {key}: {old.get(key, '(none)')} -> {new.get(key, '(dropped)')}", file=sys.stderr)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    check = "--check" in args
    names = [a for a in args if a != "--check"]
    if any(n.startswith("-") for n in names) or (not names and not check):
        print(__doc__.split("\n\n", 2)[1], file=sys.stderr)
        return 2
    root = repo_root()
    pinned_in: Dict[str, Set[str]] = {}
    for path in pins_files(root):
        for key in keys_of(path):
            pinned_in.setdefault(key.split(".", 1)[0], set()).add(path.relative_to(root).as_posix())
    names = names or sorted(pinned_in)
    try:
        tests = listed_tests(root)
    except subprocess.CalledProcessError as err:
        print(f"error: listing the workspace's tests failed (exit {err.returncode})", file=sys.stderr)
        return 2
    missing = [n for n in names if n not in tests]
    for name in missing:
        files = ", ".join(sorted(pinned_in.get(name, ())))
        print(f"error: no test is named {name}{f' (it has pins in {files})' if files else ''}", file=sys.stderr)
    if missing:
        return 2

    before = {path: pins_of(path) for path in pins_files(root)}
    goldens_before = goldens(root)
    passed, failed = [], []
    fd, log_name = tempfile.mkstemp(prefix="hyperion-pins-", suffix=".log")
    os.close(fd)
    log = Path(log_name)
    try:
        for name in dict.fromkeys(names):
            ok = True
            for test in tests[name]:
                ok = run(root, test, not check, log) and ok
            (passed if ok else failed).append(name)
        stale = unpinned(root, passed, logged(log))
    finally:
        log.unlink(missing_ok=True)

    for path, keys in stale.items():
        shown = path.relative_to(root).as_posix()
        if check:
            for key in keys:
                print(f"error: {key} is pinned in {shown}, but its test pins it no more; "
                      f"`just repin {key.split('.', 1)[0]}` drops it", file=sys.stderr)
        else:
            drop(path, keys)
            print(f"repin: dropped {len(keys)} key(s) no test pins from {shown}: {', '.join(keys)}",
                  file=sys.stderr)
    report_moves(root, before)
    reblessed = sorted(p.relative_to(root).as_posix() for p, stamp in goldens(root).items()
                       if goldens_before.get(p) != stamp)
    for path in reblessed:
        print(f"warning: {path} was blessed too, by a test of these names; account for it with "
              "`.claude/skills/sim-determinism/scripts/golden_diff.py`", file=sys.stderr)
    for name in failed:
        print(f"error: {name} failed{'' if check else '; its keys were not pruned'}", file=sys.stderr)
    verb = "checked" if check else "re-pinned"
    print(f"repin: {verb} {len(passed)} of {len(passed) + len(failed)} test name(s)", file=sys.stderr)
    return 1 if failed or (check and stale) else 0


if __name__ == "__main__":
    sys.exit(main())
