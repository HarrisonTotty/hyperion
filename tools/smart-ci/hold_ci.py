#!/usr/bin/env python3
"""The next version's held tests on one tree (`just hold-ci`), so that a train's blockers surface the
day their code lands rather than at the train's gate.

Usage: hold_ci.py [--version N] [--since REV] [--plan] [--threads N]

Three sets, found from the code's graph (`rust_graph.py`) and run in this order:

1. held: every test whose code reaches version N's holds, by default the newest version that has a
   union of them: that union (`with_every_N_hold`, `GalaxyRules::every_N_hold`,
   `StellarRules::every_N_hold`) or one of the switches the planetary union sets (each `with_*`
   function its body calls). Its restated tests, its censuses and its "held only until the N bump"
   tests among them.
2. censuses: every slow test (`#[ignore = "slow: ..."]`) whose name says `census` and whose code
   reaches any version's union or switch, less those of 1.
3. touched: with `--since REV`, the ordinary tests (not ignored) of the crates that `REV..HEAD`
   changes, the round's new commits.

A reach here follows calls alone (functions, methods and imports; not types, impls, macros or
globs) and never passes through another union or switch. The graph's own reach (`tests_reaching`)
over-approximates through types and globs: each switch's local `Restore` reaches every other
switch by its name, and a `use super::*` every item of the module it imports, which together reach
most of the sim's tests.

Each test of 1 and 2 runs alone, by name, on `--threads` (16) test threads, so that a test that
runs threads of its own is granted up to that many (`hyperion_testkit::threads::granted`), under
nextest's `hold-ci` profile (`.config/nextest.toml`), which ends a test past 2 h and prints what it
wrote. Set 3 is one nextest run of its crates on as many test threads. Every run is at the slow-test
profile over the whole workspace, so that one build serves all three: a `-p` run would resolve the
dependencies' features otherwise (the sim's `testing`, from the server's dev-dependency) and build
the sim again.

A test that passed on the same Rust inputs (`crates/`, the manifests, the lock, the toolchain,
`.cargo/` and nextest's configuration, by their git tree ids at HEAD, when none of them has an
uncommitted change) is not run again, nor is set 3 where all of it passed: the record is
`<build dir>/hold-ci/results.json`. Each run's log is `<build dir>/hold-ci/logs/<n>-<name>.log`
(set 3's `touched.log`); a table of every test's outcome ends the output, and the status is 1 if
any test failed or timed out. `--plan` prints the sets and runs nothing.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rust_graph  # noqa: E402

# The inputs a test's outcome depends on, beside the toolchain's own binaries.
RUST_INPUTS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", ".config"]
# One test of sets 1 and 2 ends past this (`[profile.hold-ci]`'s `slow-timeout`); stated here for
# the table alone.
TIMEOUT_S = 7200
CARGO = ["cargo", "nextest", "run", "--workspace", "--cargo-profile", "slow-test"]

UNION = re.compile(r"#(?:fn:[\w:]*:with_every_(\d+)_hold|member:[\w:]*:impl \w+::every_(\d+)_hold)$")
SWITCH = re.compile(r"#fn:[\w:]*:with_\w+$")
CALL = re.compile(r"#(?:fn|use|member):")
IGNORE = re.compile(r'#\[ignore\s*=\s*"([^"]*)"')


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout


def build_graph(root: Path) -> rust_graph.Graph:
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=root,
                                     capture_output=True, text=True, check=True).stdout)
    files = [f for f in git(root, "ls-files", "-z").split("\0") if f]
    ids = rust_graph.index_ids(git(root, "ls-files", "-z", "--stage"), git(root, "diff-files", "-z", "--name-only"))

    def read(path: str) -> Optional[bytes]:
        try:
            return (root / path).read_bytes()
        except OSError:
            return None

    facts = rust_graph.Facts(build_dir(root) / "hold-ci" / "rust-facts.json", read, ids)
    graph = rust_graph.Graph(rust_graph.crates_from_metadata(meta), files, facts)
    facts.save()
    return graph


def build_dir(root: Path) -> Path:
    return Path(os.environ.get("CARGO_TARGET_DIR") or root / "target")


def unions_and_switches(graph: rust_graph.Graph) -> Tuple[Dict[int, Set[str]], Dict[int, Set[str]]]:
    """Each version's union nodes, and the switches its unions call."""
    unions: Dict[int, Set[str]] = {}
    for node in graph.edges:
        m = UNION.search(node)
        if m:
            unions.setdefault(int(m.group(1) or m.group(2)), set()).add(node)
    switches = {v: {d for n in ns for d in graph.edges.get(n, ()) if SWITCH.search(d) and not UNION.search(d)}
                for v, ns in unions.items()}
    return unions, switches


def reaching(graph: rust_graph.Graph, reverse: Dict[str, Set[str]], starts: Set[str],
             blocked: Set[str]) -> Set[str]:
    """The nodes whose calls reach one of `starts`, through no node of `blocked` but those."""
    seen = set(starts)
    todo = list(starts)
    while todo:
        node = todo.pop()
        for source in reverse.get(node, ()):
            if source not in seen and source not in blocked and CALL.search(source):
                seen.add(source)
                todo.append(source)
    return seen


def ignore_reason(root: Path, node: str, cache: Dict[str, str]) -> str:
    """The reason of the `#[ignore = "..."]` on the test function `node`, or ""."""
    path, _, ident = node.partition("#")
    name = ident.rsplit(":", 1)[-1]
    if path not in cache:
        cache[path] = (root / path).read_text(encoding="utf-8", errors="replace")
    text = cache[path]
    at = re.search(rf"\bfn\s+{re.escape(name)}\b", text)
    if at is None:
        return ""
    before = [m for m in IGNORE.finditer(text, 0, at.start())]
    if not before or re.search(r"\bfn\s", text[before[-1].end():at.start()]):
        return ""
    return before[-1].group(1)


def choose(root: Path, graph: rust_graph.Graph, version: Optional[int]
           ) -> Tuple[int, List[Tuple[str, str]], List[Tuple[str, str]]]:
    """The version, and sets 1 and 2 as (binary id, test name)."""
    unions, switches = unions_and_switches(graph)
    if not unions:
        raise SystemExit("error: no version's union of holds found")
    version = max(unions) if version is None else version
    if version not in unions:
        raise SystemExit(f"error: no union of version {version}'s holds (found {sorted(unions)})")
    reverse: Dict[str, Set[str]] = {}
    for source, targets in graph.edges.items():
        for target in targets:
            reverse.setdefault(target, set()).add(source)
    every = set().union(*unions.values(), *switches.values())
    mine = unions[version] | switches[version]
    held_nodes = reaching(graph, reverse, mine, every - mine)
    any_nodes = reaching(graph, reverse, every, set())
    held: List[Tuple[str, str]] = []
    censuses: List[Tuple[str, str]] = []
    texts: Dict[str, str] = {}
    for unit in sorted(graph.tests):
        for test in sorted(graph.tests[unit], key=lambda t: t.name):
            if test.node in held_nodes:
                held.append((unit, test.name))
            elif test.ignored and test.node in any_nodes and "census" in test.name \
                    and ignore_reason(root, test.node, texts).startswith("slow"):
                censuses.append((unit, test.name))
    return version, held, censuses


def touched_crates(root: Path, graph: rust_graph.Graph, since: str) -> List[str]:
    changed = [p for p in git(root, "diff", "--name-only", f"{since}..HEAD").splitlines() if p]
    return sorted({c for c in (graph.crate_of(p) for p in changed) if c})


def inputs_key(root: Path) -> Optional[str]:
    """The Rust inputs' tree ids at HEAD, or None where one has an uncommitted change."""
    if git(root, "status", "--porcelain", "--", *RUST_INPUTS).strip():
        return None
    return " ".join(git(root, "rev-parse", f"HEAD:{p}").strip() for p in RUST_INPUTS if (root / p).exists())


def run_logged(args: Sequence[str], root: Path, log: Path) -> Tuple[int, float]:
    log.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with log.open("w", encoding="utf-8") as out:
        out.write("$ " + " ".join(args) + "\n")
        out.flush()
        status = subprocess.run(list(args), cwd=root, stdout=out, stderr=subprocess.STDOUT).returncode
    return status, time.monotonic() - started


def outcome(status: int, log: Path) -> str:
    if status == 0:
        return "PASS"
    text = log.read_text(encoding="utf-8", errors="replace")
    return "TIMEOUT" if re.search(r"^\s*TIMEOUT \[", text, re.M) else "FAIL"


def failures_of(log: Path) -> List[str]:
    """The tests a nextest run's summary lists as failed or timed out."""
    text = log.read_text(encoding="utf-8", errors="replace")
    summary = text[text.rfind("Summary ["):] if "Summary [" in text else text
    return sorted(set(re.findall(r"^\s*(?:FAIL|TIMEOUT|SIGSEGV|SIGABRT) \[[^\]]*\]\s+(?:\(\d+/\d+\)\s+)?(\S+ \S+)",
                                 summary, re.M)))


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument("--version", type=int, help="the held version (default: the newest with a union)")
    parser.add_argument("--since", help="the previous tip: set 3 runs the crates REV..HEAD touches")
    parser.add_argument("--plan", action="store_true", help="print the sets and run nothing")
    parser.add_argument("--threads", type=int, default=16, help="nextest's test threads (default 16)")
    args = parser.parse_args(argv)
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())

    graph = build_graph(root)
    version, held, censuses = choose(root, graph, args.version)
    crates = touched_crates(root, graph, args.since) if args.since else []
    print(f"hold-ci at {git(root, 'rev-parse', '--short', 'HEAD').strip()}, version {version}'s holds")
    for title, tests in ((f"1. held at {version}", held), ("2. slow censuses that read holds", censuses)):
        print(f"{title}: {len(tests)}")
        for unit, name in tests:
            print(f"  {unit} {name}")
    touched_filter = " | ".join(f"package({c})" for c in crates)
    print(f"3. touched crates: {', '.join(crates) or 'none'}" + (f" (-E '{touched_filter}')" if crates else ""))
    if args.plan:
        return 0

    out_dir = build_dir(root) / "hold-ci"
    record_path = out_dir / "results.json"
    key = inputs_key(root)
    try:
        record = json.loads(record_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        record = {}
    passed: Set[str] = set(record.get("passed", [])) if key is not None and record.get("key") == key else set()

    status = subprocess.run(CARGO + ["--no-run"], cwd=root).returncode
    if status != 0:
        print(f"error: the slow-test build failed (exit {status})", file=sys.stderr)
        return status

    rows: List[Tuple[str, str, str, str]] = []  # set, test, outcome, seconds
    failed = False
    for n, (unit, name) in enumerate(held + censuses, 1):
        which = "1" if n <= len(held) else "2"
        label = f"{unit} {name}"
        if label in passed:
            rows.append((which, label, "PASS (earlier, same inputs)", "-"))
            continue
        log = out_dir / "logs" / f"{n:02d}-{name.rsplit('::', 1)[-1][:80]}.log"
        status, took = run_logged(CARGO + ["--profile", "hold-ci", "--run-ignored", "all", "--no-fail-fast",
                                           "--test-threads", str(args.threads),
                                           "-E", f"binary_id(={unit}) & test(={name})"], root, log)
        result = outcome(status, log)
        print(f"[{which}] {result} {took:.0f} s {label} ({log})", flush=True)
        rows.append((which, label, result, f"{took:.0f}"))
        failed |= result != "PASS"
        if result == "PASS":
            passed.add(label)
        if key is not None:
            record_path.parent.mkdir(parents=True, exist_ok=True)
            record_path.write_text(json.dumps({"key": key, "passed": sorted(passed)}, indent=1), encoding="utf-8")

    if crates:
        label = "touched " + ",".join(crates)
        if label in passed:
            rows.append(("3", label, "PASS (earlier, same inputs)", "-"))
        else:
            log = out_dir / "logs" / "touched.log"
            status, took = run_logged(CARGO + ["--no-fail-fast", "--test-threads", str(args.threads),
                                               "-E", touched_filter], root, log)
            if status == 0:
                rows.append(("3", label, "PASS", f"{took:.0f}"))
                passed.add(label)
            else:
                failed = True
                lost = failures_of(log)
                rows += [("3", t, "FAIL", "-") for t in lost] or [("3", label, f"FAIL (exit {status})", f"{took:.0f}")]
            print(f"[3] exit {status} {took:.0f} s {label} ({log})", flush=True)
            if key is not None:
                record_path.write_text(json.dumps({"key": key, "passed": sorted(passed)}, indent=1), encoding="utf-8")

    print(f"\nhold-ci: {sum(r[2].startswith('PASS') for r in rows)} passed of {len(rows)} "
          f"(a test of sets 1 and 2 ends past {TIMEOUT_S} s)")
    for which, label, result, took in rows:
        print(f"  [{which}] {result:<8} {took:>6} s  {label}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
