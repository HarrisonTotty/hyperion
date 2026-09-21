#!/usr/bin/env python3
"""Report which golden files changed, separating version-header churn from moved output.

Usage:
    golden_diff.py [--base REF] [--lines N]

Compares crates/*/tests/golden/**/*.golden in the working tree (untracked included) with REF
(default HEAD). Every golden starts with `# generator_version = N`, so a GENERATOR_VERSION bump
touches every file; this report hides that and shows only goldens whose content moved, with the
first N changed labels of each (default 5). It also compares GENERATOR_VERSION in
crates/hyperion-sim/src/version.rs, and states whether the pair is consistent:

- content moved, version not bumped   -> bump required (or the change is a bug)
- version bumped, goldens not blessed -> run `just bless`
- version bumped twice since REF      -> one bump per change is enough
"""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path

HEADER_RE = re.compile(r"^#\s*generator_version\s*=\s*(\d+)\s*$")
VERSION_RE = re.compile(r"GeneratorVersion::new\((\d+)\)")
VERSION_FILE = "crates/hyperion-sim/src/version.rs"
# Plain pathspecs match `*` literally at directory boundaries; `:(glob)` makes it one level.
GOLDEN_SPEC = ":(glob)crates/*/tests/golden/**"


def run(root: Path, *args: str) -> tuple[int, str]:
    proc = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)
    return proc.returncode, proc.stdout


def repo_root() -> Path:
    proc = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if proc.returncode == 0:
        return Path(proc.stdout.strip())
    return Path(__file__).resolve().parents[4]


def at_ref(root: Path, ref: str, path: str) -> str | None:
    code, out = run(root, "show", f"{ref}:{path}")
    return out if code == 0 else None


def version_in(text: str | None) -> int | None:
    if text is None:
        return None
    m = VERSION_RE.search(text)
    return int(m.group(1)) if m else None


def split_header(text: str) -> tuple[int | None, list[str]]:
    lines = text.splitlines()
    version = None
    body = []
    for line in lines:
        m = HEADER_RE.match(line)
        if m and version is None:
            version = int(m.group(1))
        else:
            body.append(line)
    return version, body


def label(line: str) -> str:
    return line.split("=", 1)[0].strip() if "=" in line else line.strip()[:60]


def by_label(body: list[str]) -> dict[str, str]:
    """Map each `label = value` line to its label. Unlabelled lines (comments, section markers) key
    on their full text, and repeats get an occurrence number, so a line that only shifted position
    still matches."""
    out: dict[str, str] = {}
    for line in body:
        base = label(line) if "=" in line else line.strip()
        key, n = base, 1
        while key in out:
            n += 1
            key = f"{base} (#{n})"
        out[key] = line
    return out


def describe(entry: tuple[str, list[str], list[str], list[str]], limit: int) -> str:
    path, changed, gained, lost = entry
    parts = []
    for name, items in (("changed", changed), ("new", gained), ("gone", lost)):
        if items:
            shown = ", ".join(items[:limit]) + (" …" if len(items) > limit else "")
            parts.append(f"{len(items)} {name} ({shown})")
    return f"  {path}: " + "; ".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", default="HEAD", help="git ref to compare against (default HEAD)")
    parser.add_argument("--lines", type=int, default=5, help="changed labels to show per golden")
    args = parser.parse_args()
    root = repo_root()

    old_version = version_in(at_ref(root, args.base, VERSION_FILE))
    version_path = root / VERSION_FILE
    new_version = version_in(version_path.read_text(encoding="utf-8")) if version_path.exists() else None
    print(f"GENERATOR_VERSION: {old_version} at {args.base} -> {new_version} in the working tree")

    _, tracked = run(root, "diff", "--name-status", args.base, "--", GOLDEN_SPEC)
    _, untracked = run(root, "ls-files", "--others", "--exclude-standard", "--", GOLDEN_SPEC)
    entries: list[tuple[str, str]] = []
    for line in tracked.splitlines():
        status, _, path = line.partition("\t")
        entries.append((status[:1], path.split("\t")[-1]))
    entries += [("A", p) for p in untracked.splitlines() if p]

    header_only, moved, extended, added, deleted, stale_header = [], [], [], [], [], []
    for status, path in sorted(set(entries), key=lambda e: e[1]):
        if not path.endswith(".golden"):
            continue
        current = root / path
        if status == "D" or not current.exists():
            deleted.append(path)
            continue
        new_v, new_body = split_header(current.read_text(encoding="utf-8"))
        if new_version is not None and new_v != new_version:
            stale_header.append((path, new_v))
        old_text = at_ref(root, args.base, path)
        if old_text is None:
            added.append(path)
            continue
        _, old_body = split_header(old_text)
        if old_body == new_body:
            header_only.append(path)
            continue
        old_map, new_map = by_label(old_body), by_label(new_body)
        changed = [k for k in new_map if k in old_map and new_map[k] != old_map[k]]
        gained = [k for k in new_map if k not in old_map]
        lost = [k for k in old_map if k not in new_map]
        (moved if changed or lost else extended).append((path, changed, gained, lost))

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
        print(f"\nDeleted goldens ({len(deleted)}): a golden is removed only with the test that wrote it")
        for path in deleted:
            print(f"  {path}")
    if header_only:
        print(f"\nHeader only ({len(header_only)}): version line changed, output identical")

    print("\nVerdict:")
    rise = new_version - old_version if old_version is not None and new_version is not None else 0
    bumped = rise != 0
    problems = False
    if old_version is None or new_version is None:
        problems = True
        print(f"  - Could not read GENERATOR_VERSION from {VERSION_FILE} at both ends; compare by hand.")
    if moved and not bumped:
        problems = True
        print("  - Pinned values changed but GENERATOR_VERSION was not bumped: bump it and run `just bless`,")
        print("    or, if the change was not meant to move output, find what moved it.")
    if (extended or added) and not moved and not bumped:
        print("  - Only new labels were pinned, and no previously pinned value changed. That does not prove")
        print("    output held: a new label may pin a value the base already generated without pinning it.")
        print("    For each new label, check whether its value existed at the base; if its computation changed")
        print("    since, it is moved output and needs the bump. Only a value that did not exist at all is new.")
    if rise > 1:
        problems = True
        print(f"  - GENERATOR_VERSION rose by {rise}; one bump per change is enough.")
    if stale_header:
        problems = True
        print("  - Goldens whose header differs from GENERATOR_VERSION (run `just bless`):")
        for path, v in stale_header:
            print(f"      {path}: header {v}")
    if bumped and not (moved or extended or added) and not stale_header:
        print("  - Version bumped but no golden content moved: fine before the first release, but check")
        print("    that the bump was needed (a change that moves no pinned value may still move unpinned ones).")
    if not problems and moved:
        print("  - Consistent. Now account for every changed golden above against the task.")
    elif not problems and not (moved or extended or added):
        print("  - Nothing to reconcile.")


if __name__ == "__main__":
    main()
