#!/usr/bin/env python3
"""Report which golden files changed, separating version-header churn from moved output.

Usage:
    golden_diff.py [--base REF] [--head REF] [--lines N]

Compares crates/*/tests/golden/**/*.golden at HEAD (default: the working tree, untracked
included) with BASE (default HEAD). To check one commit C, use `--base C^ --head C`. Every golden
starts with `# generator_version = N`, so a GENERATOR_VERSION bump touches every file; this report
hides that and shows only goldens whose content moved, with the first N changed labels of each
(default 5). It also compares GENERATOR_VERSION in crates/hyperion-sim/src/version.rs, checks the
header of every golden at HEAD against it, and states whether the pair is consistent:

- content moved, version not bumped    -> bump required (or the change is a bug)
- version bumped, goldens not blessed  -> run `just bless`
- version rose by more than one        -> fine only if the range spans that many tasks

Exit status: 0 when nothing is inconsistent (checks to make by hand may remain), 1 when the verdict
lists a problem, 2 on bad arguments.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True

HEADER_RE = re.compile(r"^#\s*generator_version\s*=\s*(\d+)\s*$")
VERSION_RE = re.compile(r"GeneratorVersion::new\((\d+)\)")
VERSION_FILE = "crates/hyperion-sim/src/version.rs"
# Plain pathspecs match `*` literally at directory boundaries; `:(glob)` makes it one level.
GOLDEN_SPEC = ":(glob)crates/*/tests/golden/**"
GOLDEN_PATH_RE = re.compile(r"^crates/[^/]+/tests/golden/.+\.golden$")


def run(root: Path, *args: str) -> tuple[int, str]:
    proc = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)
    return proc.returncode, proc.stdout


def repo_root() -> Path:
    proc = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if proc.returncode == 0:
        return Path(proc.stdout.strip())
    return Path(__file__).resolve().parents[4]


def usage_error(message: str) -> None:
    print(f"golden_diff.py: {message}", file=sys.stderr)
    sys.exit(2)


def check_ref(root: Path, ref: str, flag: str) -> None:
    if ".." in ref:
        usage_error(f"{flag} takes one commit, not a range. For the commits A..B, use --base A --head B; "
                    "for one commit C, --base C^ --head C.")
    code, _ = run(root, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}")
    if code != 0:
        usage_error(f"{flag} {ref!r} is not a commit in {root}.")


def at_ref(root: Path, ref: str, path: str) -> str | None:
    code, out = run(root, "show", f"{ref}:{path}")
    return out if code == 0 else None


def version_in(text: str | None) -> int | None:
    if text is None:
        return None
    m = VERSION_RE.search(text)
    return int(m.group(1)) if m else None


def split_header(text: str) -> tuple[int | None, list[str]]:
    version = None
    body = []
    for line in text.splitlines():
        m = HEADER_RE.match(line)
        if m and version is None:
            version = int(m.group(1))
        else:
            body.append(line)
    return version, body


def by_label(body: list[str]) -> dict[str, str]:
    """Map each line of a golden body to a stable key, so that a line which only shifted position
    still matches. A `label = value` line keys on its label within its section (the last unlabelled
    line above it), because sections repeat labels such as `word[0]`. An unlabelled line keys on its
    full text. Repeats within a section get an occurrence number."""
    out: dict[str, str] = {}
    section = ""
    for line in body:
        text = line.strip()
        if not text:
            continue
        if "=" in text:
            name = text.split("=", 1)[0].strip()
            base = f"{name} [in {section}]" if section else name
        else:
            section = text
            base = text[:80]
        key, n = base, 1
        while key in out:
            n += 1
            key = f"{base} (#{n})"
        out[key] = line
    return out


def shorten(key: str) -> str:
    """A key for display: the section part cut to 40 characters."""
    name, sep, section = key.partition(" [in ")
    return f"{name} [in {section[:40]}…]" if sep and len(section) > 41 else key


def describe(entry: tuple[str, list[str], list[str], list[str]], limit: int) -> str:
    path, changed, gained, lost = entry
    parts = []
    for name, items in (("changed", changed), ("new", gained), ("gone", lost)):
        if items:
            shown = ", ".join(shorten(k) for k in items[:limit]) + (" …" if len(items) > limit else "")
            parts.append(f"{len(items)} {name} ({shown})")
    return f"  {path}: " + "; ".join(parts)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", default="HEAD", help="commit to compare against (default HEAD)")
    parser.add_argument("--head", help="commit to compare (default: the working tree)")
    parser.add_argument("--lines", type=int, default=5, help="changed labels to show per golden")
    args = parser.parse_args()
    root = repo_root()
    check_ref(root, args.base, "--base")
    if args.head:
        check_ref(root, args.head, "--head")
    head_name = args.head or "the working tree"

    def read_head(path: str) -> str | None:
        if args.head:
            return at_ref(root, args.head, path)
        file = root / path
        return file.read_text(encoding="utf-8") if file.exists() else None

    old_version = version_in(at_ref(root, args.base, VERSION_FILE))
    new_version = version_in(read_head(VERSION_FILE))
    print(f"GENERATOR_VERSION: {old_version} at {args.base} -> {new_version} in {head_name}")

    entries: list[tuple[str, str]] = []
    if args.head:
        _, tracked = run(root, "diff", "--name-status", "--no-renames", args.base, args.head, "--", GOLDEN_SPEC)
        _, listed = run(root, "ls-tree", "-r", "--name-only", args.head, "--", "crates")
        every_golden = [p for p in listed.splitlines() if GOLDEN_PATH_RE.match(p)]
    else:
        _, tracked = run(root, "diff", "--name-status", "--no-renames", args.base, "--", GOLDEN_SPEC)
        _, untracked = run(root, "ls-files", "--others", "--exclude-standard", "--", GOLDEN_SPEC)
        entries += [("A", p) for p in untracked.splitlines() if p]
        every_golden = [p.relative_to(root).as_posix() for p in root.glob("crates/*/tests/golden/**/*.golden")]
    for line in tracked.splitlines():
        status, _, path = line.partition("\t")
        entries.append((status[:1], path))

    header_only, moved, extended, added, deleted = [], [], [], [], []
    for status, path in sorted(set(entries), key=lambda e: e[1]):
        if not path.endswith(".golden"):
            continue
        new_text = read_head(path)
        if status == "D" or new_text is None:
            deleted.append(path)
            continue
        old_text = at_ref(root, args.base, path)
        if old_text is None:
            added.append(path)
            continue
        _, old_body = split_header(old_text)
        _, new_body = split_header(new_text)
        if old_body == new_body:
            header_only.append(path)
            continue
        old_map, new_map = by_label(old_body), by_label(new_body)
        changed = [k for k in new_map if k in old_map and new_map[k] != old_map[k]]
        gained = [k for k in new_map if k not in old_map]
        lost = [k for k in old_map if k not in new_map]
        (moved if changed or lost else extended).append((path, changed, gained, lost))

    stale_header = []
    if new_version is not None:
        for path in sorted(every_golden):
            text = read_head(path)
            header = split_header(text)[0] if text is not None else None
            if header != new_version:
                stale_header.append((path, header))

    if not (header_only or moved or extended or added or deleted):
        print("No golden files changed.")
    if moved:
        print(f"\nPinned values changed ({len(moved)}): existing output moved; each must be explained by the change")
        for entry in moved:
            print(describe(entry, args.lines))
    if extended:
        print(f"\nExtended only ({len(extended)}): new values pinned, every existing value unchanged")
        for entry in extended:
            print(describe(entry, args.lines))
    if added:
        print(f"\nNew goldens ({len(added)}):")
        for path in added:
            print(f"  {path}")
    if deleted:
        print(f"\nDeleted goldens ({len(deleted)}):")
        for path in deleted:
            print(f"  {path}")
    if header_only:
        print(f"\nHeader only ({len(header_only)}): version line changed, output identical")

    problems: list[str] = []
    checks: list[str] = []
    readable = old_version is not None and new_version is not None
    rise = new_version - old_version if old_version is not None and new_version is not None else 0
    if not readable:
        problems.append(f"Could not read GENERATOR_VERSION from {VERSION_FILE} at both ends; compare by hand.")
    if rise < 0:
        problems.append(f"GENERATOR_VERSION went down by {-rise}. A version number is never reused.")
    if moved and readable and rise == 0:
        problems.append("Pinned values changed but GENERATOR_VERSION was not bumped: bump it and run `just bless`,\n"
                        "    or, if the change was not meant to move output, find what moved it.")
    if stale_header:
        shown = "\n".join(f"      {path}: header {v}" for path, v in stale_header)
        problems.append(f"Goldens whose header is not GENERATOR_VERSION {new_version} (run `just bless`):\n{shown}")
    if rise > 1:
        checks.append(f"GENERATOR_VERSION rose by {rise}. That is right only if the range spans {rise} tasks that\n"
                      "    each moved output; one task bumps once.")
    if (extended or added) and readable and rise == 0:
        checks.append("Only new labels were pinned, and no previously pinned value changed. That does not prove\n"
                      "    output held: a new label may pin a value the base already generated without pinning it.\n"
                      "    For each new label, check whether its value existed at the base; if its computation changed\n"
                      "    since, it is moved output and needs the bump. Only a value that did not exist at all is new.")
    if deleted:
        checks.append("Goldens were deleted. A golden goes only with the test that wrote it; confirm that test went too.")
    if rise > 0 and not (moved or extended or added or deleted):
        checks.append("Version bumped but no pinned value moved. Check that the bump was needed: a change that moves\n"
                      "    no pinned value may still move unpinned ones.")

    print("\nVerdict:")
    for item in problems:
        print(f"  - Problem: {item}")
    for item in checks:
        print(f"  - Check: {item}")
    if problems:
        print("  Inconsistent: resolve every problem above.")
    elif moved:
        print("  Consistent" + (", after the checks above" if checks else "")
              + ". Now account for every changed value above against the task.")
    elif checks:
        print("  No inconsistency found; make the checks above.")
    elif extended or added or header_only:
        print("  Consistent.")
    else:
        print("  Nothing to reconcile.")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
