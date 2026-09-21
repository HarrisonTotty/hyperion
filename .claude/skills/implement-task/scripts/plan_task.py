#!/usr/bin/env python3
"""Extract tasks from HYPERION action plans (docs/agent/plans/<feature>/NN-*.md).

Usage:
    plan_task.py P02.T5.a              Task text with the plan header, the ordering notes and
                                       every design note the task cites.
    plan_task.py P02.T5.a --context    The same, plus the plan's Generator version and Risks
                                       sections (earlier tasks' as-built records live there).
    plan_task.py P02.T5 --acceptance   Only the acceptance criteria (all subtasks) and the
                                       commands quoted in them.
    plan_task.py --list [P02]          Every task ID and title, marked where a commit names it.
    --feature <dir>                    Pick the plan set when an ID exists in several.

Task markers the parser understands: `### P01.T1 Title` / `#### P01.T1.a Title` headings,
`**P05.T1.a Title.** body` paragraphs and `- **P08.T2.a Title.** body` bullets. Design notes:
`28. **Title.**` numbered items and `**D4. Title.**` paragraphs, cited as "Design note 7",
"design notes 7 and 13" or "D4".
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ID_RE = re.compile(r"P(\d{2})\.T(\d+)(?:\.([a-z]+))?")
HEADING_TASK_RE = re.compile(r"^(#{2,6})\s+(P\d{2}\.T\d+(?:\.[a-z]+)?)\b\s*(.*)$")
INLINE_TASK_RE = re.compile(r"^(?:[-*]\s+)?\*\*(P\d{2}\.T\d+(?:\.[a-z]+)?)\b\s*([^*]*?)\.?\*\*")
HEADING_RE = re.compile(r"^(#{1,6})\s")
NOTE_START_RES = (
    re.compile(r"^(\d+)\.\s+\*\*"),  # 28. **Title.**
    re.compile(r"^\*\*D(\d+)\.\s"),  # **D4. Title.**
    re.compile(r"^[-*]\s+\*\*D(\d+)\.\s"),  # - **D4. Title.**
    re.compile(r"^#{3,5}\s+D(\d+)\b"),  # ### D4 Title
)
NOTE_REF_RES = (
    re.compile(r"\bD(\d{1,2})\b"),
    # "Design note 7", "design notes 7 and 13", and a bare "note 5" after a first citation.
    re.compile(r"\b(?:[Dd]esign\s+)?notes?\s+((?:D?\d{1,2}(?:\s*(?:,|and|or|–|-)\s*)?)+)"),
)
COMMAND_RE = re.compile(r"`((?:just|cargo|pnpm|grep|rg|git|python3?|uvx|wasmtime)\b[^`]*)`")


@dataclass
class Marker:
    line: int  # 0-based index into the plan's lines
    task_id: str
    title: str
    level: int  # heading level, or 7 for an inline paragraph/bullet marker


def repo_root() -> Path:
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True
        )
        return Path(out.stdout.strip())
    except (OSError, subprocess.CalledProcessError):
        # .claude/skills/implement-task/scripts/plan_task.py -> repository root
        return Path(__file__).resolve().parents[4]


def plan_sets(root: Path, feature: str | None) -> list[Path]:
    base = root / "docs" / "agent" / "plans"
    if not base.is_dir():
        sys.exit(f"no plans directory at {base}")
    sets = sorted(p for p in base.iterdir() if p.is_dir())
    if feature is not None:
        sets = [p for p in sets if p.name == feature]
        if not sets:
            names = ", ".join(p.name for p in sorted(base.iterdir()) if p.is_dir())
            sys.exit(f"no plan set named {feature!r}; available: {names}")
    return sets


def find_plan(root: Path, plan_no: str, feature: str | None) -> Path:
    matches = [f for s in plan_sets(root, feature) for f in sorted(s.glob(f"{plan_no}-*.md"))]
    if not matches:
        sys.exit(f"no plan numbered {plan_no} under docs/agent/plans/*/")
    if len(matches) > 1:
        listing = "\n  ".join(str(m.relative_to(root)) for m in matches)
        sys.exit(f"plan {plan_no} exists in several plan sets; pass --feature:\n  {listing}")
    return matches[0]


def markers(lines: list[str]) -> list[Marker]:
    found = []
    for i, line in enumerate(lines):
        if m := HEADING_TASK_RE.match(line):
            found.append(Marker(i, m.group(2), m.group(3).strip(), len(m.group(1))))
        elif m := INLINE_TASK_RE.match(line):
            found.append(Marker(i, m.group(1), m.group(2).strip(), 7))
    return found


def heading_level(line: str) -> int | None:
    m = HEADING_RE.match(line)
    return len(m.group(1)) if m else None


def section_end(lines: list[str], marker: Marker, all_markers: list[Marker]) -> int:
    """First line after the marker's section."""
    for j in range(marker.line + 1, len(lines)):
        level = heading_level(lines[j])
        if marker.level <= 6:
            if level is not None and level <= marker.level:
                return j
        else:
            if level is not None:
                return j
            if any(m.line == j and m.level == 7 for m in all_markers):
                return j
    return len(lines)


def named_section(lines: list[str], title_prefix: str) -> tuple[int, int] | None:
    for i, line in enumerate(lines):
        if line.startswith("## ") and line[3:].strip().lower().startswith(title_prefix):
            end = next((j for j in range(i + 1, len(lines)) if lines[j].startswith("## ")), len(lines))
            return i, end
    return None


def design_notes(lines: list[str]) -> dict[int, str]:
    span = named_section(lines, "design notes")
    if span is None:
        return {}
    start, end = span
    starts: list[tuple[int, int]] = []
    for i in range(start + 1, end):
        for pattern in NOTE_START_RES:
            if m := pattern.match(lines[i]):
                starts.append((i, int(m.group(1))))
                break
    notes = {}
    for k, (i, number) in enumerate(starts):
        stop = starts[k + 1][0] if k + 1 < len(starts) else end
        notes[number] = "\n".join(lines[i:stop]).rstrip()
    return notes


def cited_notes(text: str) -> list[int]:
    numbers: set[int] = set()
    for m in NOTE_REF_RES[0].finditer(text):
        numbers.add(int(m.group(1)))
    for m in NOTE_REF_RES[1].finditer(text):
        for part in re.findall(r"D?(\d{1,2})", m.group(1)):
            numbers.add(int(part))
    return sorted(numbers)


def paragraphs(block: list[str]) -> list[str]:
    """Split on blank lines and on bullets, so each bullet is its own paragraph."""
    out: list[list[str]] = []
    for line in block:
        if not line.strip():
            out.append([])
        elif re.match(r"^\s*[-*]\s", line) or not out:
            out.append([line])
        else:
            out[-1].append(line)
    return ["\n".join(p) for p in out if p]


def load(root: Path, task_id: str, feature: str | None):
    m = ID_RE.fullmatch(task_id)
    if m is None:
        sys.exit(f"{task_id!r} is not a task ID like P02.T5 or P05.T1.a")
    plan = find_plan(root, m.group(1), feature)
    lines = plan.read_text(encoding="utf-8").splitlines()
    all_markers = markers(lines)
    target = next((mk for mk in all_markers if mk.task_id == task_id), None)
    if target is None:
        known = ", ".join(mk.task_id for mk in all_markers[:60])
        sys.exit(f"{task_id} not found in {plan.relative_to(root)}. Tasks there: {known}")
    return plan, lines, all_markers, target


def parent_of(task_id: str, all_markers: list[Marker]) -> Marker | None:
    parts = task_id.split(".")
    if len(parts) < 3:
        return None
    return next((mk for mk in all_markers if mk.task_id == ".".join(parts[:2])), None)


SHARED_RE = re.compile(r"\((?:all of|for all of) T\d+\)|\bfor T\d+\b(?!\.)|\beach subtask\b|\ball subtasks\b", re.I)


def task_blocks(lines, all_markers, target) -> tuple[list[str], tuple[int, int], tuple[int, int] | None]:
    """The text to show: for a subtask, the parent's intro, the subtask, and any paragraph of the
    parent that applies to every subtask ("Acceptance for T6, and for each subtask", "Files (all
    of T2)"), which plans put after the last subtask."""
    end = section_end(lines, target, all_markers)
    parent = parent_of(target.task_id, all_markers)
    if parent is None:
        return lines[target.line : end], (target.line, end), None
    parent_end = section_end(lines, parent, all_markers)
    first_child = next(
        (mk.line for mk in all_markers if mk.line > parent.line and mk.task_id.startswith(parent.task_id + ".")),
        parent_end,
    )
    intro = lines[parent.line : first_child]
    own = lines[target.line : end]
    own_text = "\n".join(own)
    shared = [
        p for p in paragraphs(lines[first_child:parent_end]) if SHARED_RE.search(p) and p not in own_text
    ]
    block = intro + ["", "[…]", ""] + own
    if shared:
        block += ["", "[Shared by all subtasks of " + parent.task_id + "]", ""] + "\n\n".join(shared).splitlines()
    return block, (target.line, end), (parent.line, parent_end)


def mentions(root: Path, plan: Path, lines: list[str], task_id: str, span: tuple[int, int], limit: int = 20) -> list[str]:
    """Lines elsewhere in the plan set that name this task: the full ID in any plan, and the short
    form (T6.e, or T6 for a parent) in its own plan, outside the task's own section."""
    short = task_id.split(".", 1)[1]
    short_re = re.compile(r"(?<![\w.])" + re.escape(short) + r"(?![\w])")
    full_re = re.compile(re.escape(task_id) + r"(?![\w])")
    hits: list[str] = []
    for other in sorted(plan.parent.glob("[0-9][0-9]-*.md")):
        text = lines if other == plan else other.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(text):
            if other == plan and span[0] <= i < span[1]:
                continue
            if full_re.search(line) or (other == plan and short_re.search(line)):
                hits.append(f"{other.relative_to(root)}:{i + 1}: {line.strip()[:140]}")
    return hits[:limit] + ([f"… {len(hits) - limit} more"] if len(hits) > limit else [])


def show_task(root: Path, task_id: str, feature: str | None, context: bool = False) -> None:
    plan, lines, all_markers, target = load(root, task_id, feature)
    rel = plan.relative_to(root)
    block, (start, end), parent_span = task_blocks(lines, all_markers, target)

    print(f"# {task_id} {target.title}".rstrip())
    print(f"Plan: {rel} (task lines {start + 1}–{end})")
    if parent_span:
        print(f"Parent section: lines {parent_span[0] + 1}–{parent_span[1]} (read it for files or tests shared by all subtasks)")
    readme = plan.parent / "README.md"
    if readme.exists():
        print(f"Roadmap: {readme.relative_to(root)}")
    print()

    goal = named_section(lines, "goal")
    print("## Plan header\n")
    print("\n".join(lines[: goal[0] if goal else 12]).rstrip())
    print()

    tasks = named_section(lines, "tasks")
    if tasks:
        first = next((mk.line for mk in all_markers if mk.line > tasks[0]), tasks[1])
        intro = "\n".join(lines[tasks[0] + 1 : first]).strip()
        if intro:
            print("## Order and parallelism (from the Tasks intro)\n")
            print(intro)
            print()

    print("## Task\n")
    print("\n".join(block).rstrip())
    print()

    notes = design_notes(lines)
    cited = [n for n in cited_notes("\n".join(block)) if n in notes]
    if cited:
        print("## Design notes cited by the task (this plan's numbering; a citation of another plan's note will also match)\n")
        for number in cited:
            print(notes[number])
            print()

    refs = mentions(root, plan, lines, task_id, parent_span or (start, end))
    if refs:
        print("## Mentioned elsewhere (ordering, later checks that depend on this task)\n")
        for ref in refs:
            print(f"- {ref}")
        print()

    if context:
        for title in ("generator version", "risks"):
            span = named_section(lines, title)
            if span:
                print("\n".join(lines[span[0] : span[1]]).rstrip())
                print()

    print("## Also read\n")
    print("- The plan's Provides and Consumes entries for what this task builds or uses.")
    print("- The brainstorm sections the header lists that this task touches (the brainstorm is the specification).")
    if not context:
        print("- The plan's 'Generator version' and 'Risks and open points' sections (or rerun with --context).")


def show_acceptance(root: Path, task_id: str, feature: str | None) -> None:
    plan, lines, all_markers, target = load(root, task_id, feature)
    block, _, _ = task_blocks(lines, all_markers, target)
    if target.level <= 6 and parent_of(task_id, all_markers) is None:
        block = lines[target.line : section_end(lines, target, all_markers)]
    hits = [p for p in paragraphs(block) if re.search(r"\bAcceptance\b", p)]
    print(f"# Acceptance for {task_id} ({plan.relative_to(root)})\n")
    if not hits:
        print("(no acceptance paragraph found; the roadmap still requires `just ci` green)")
        return
    for p in hits:
        print(p.strip())
        print()
    commands = []
    for p in hits:
        for cmd in COMMAND_RE.findall(p):
            if cmd not in commands:
                commands.append(cmd)
    if commands:
        print("## Commands quoted in the acceptance criteria\n")
        for cmd in commands:
            print(f"- `{cmd}`")
    bare = [c for c in commands if re.fullmatch(r"cargo test -p \S+ [\w:]+", c)]
    if bare:
        print("\nNote: a bare filter (`cargo test -p <crate> <name>`) matches test *names*, not files.")
        print("Check that it selects the task's tests; `--test <file>` runs one integration-test file,")
        print("and `--lib <module path>` the unit tests of a module.")
    manual = [p for p in hits if re.search(r"by hand|by eye|do not commit|manually", p, re.I)]
    if manual:
        print("\n## Contains manual steps (run them, revert any temporary edit, report the result)")


def commit_messages(root: Path) -> str:
    try:
        out = subprocess.run(
            ["git", "-C", str(root), "log", "--format=%s%n%b"], capture_output=True, text=True, check=True
        )
        return out.stdout
    except (OSError, subprocess.CalledProcessError):
        return ""


def show_list(root: Path, plan_filter: str | None, feature: str | None) -> None:
    log = commit_messages(root)
    plan_no = None
    if plan_filter:
        m = re.fullmatch(r"P?(\d{1,2})", plan_filter)
        if m is None:
            sys.exit(f"{plan_filter!r} is not a plan number like P02 or 2")
        plan_no = m.group(1).zfill(2)
    for plan_set in plan_sets(root, feature):
        files = sorted(plan_set.glob(f"{plan_no}-*.md" if plan_no else "[0-9][0-9]-*.md"))
        for plan in files:
            lines = plan.read_text(encoding="utf-8").splitlines()
            print(f"## {plan.relative_to(root)}")
            for mk in markers(lines):
                named = re.search(re.escape(mk.task_id) + r"(?![\w.])", log) is not None
                indent = "  " if mk.task_id.count(".") == 2 else ""
                flag = "  [commit]" if named else ""
                print(f"{indent}{mk.task_id}  {mk.title}{flag}")
            print()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("task_id", nargs="?", help="task ID such as P02.T5 or P05.T1.a")
    parser.add_argument("--acceptance", action="store_true", help="print only the acceptance criteria")
    parser.add_argument("--context", action="store_true", help="also print the Generator version and Risks sections")
    parser.add_argument("--list", nargs="?", const="", metavar="PLAN", help="list tasks, optionally of one plan")
    parser.add_argument("--feature", help="plan set directory under docs/agent/plans/")
    args = parser.parse_args()
    root = repo_root()

    if args.list is not None:
        show_list(root, args.list or args.task_id, args.feature)
    elif args.task_id is None:
        parser.print_help()
    elif args.acceptance:
        show_acceptance(root, args.task_id, args.feature)
    else:
        show_task(root, args.task_id, args.feature, args.context)


if __name__ == "__main__":
    main()
