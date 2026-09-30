#!/usr/bin/env python3
"""Extract tasks from HYPERION action plans (docs/agent/plans/<feature>/NN-*.md).

Usage:
    plan_task.py P02.T5.a              Task text with the plan header, the ordering notes, the
                                       phase intro and every design note the task cites.
    plan_task.py P02.T5.a --context    The same, plus the plan's Generator version and Risks
                                       sections (earlier tasks' as-built records live there).
    plan_task.py P02.T5 --acceptance   Only the acceptance criteria (all subtasks, each labelled),
                                       the commands quoted in them, and the Risks lines naming
                                       the task.
    plan_task.py --list [P02]          Every task ID and title, marked where a commit subject
                                       starts with the ID.
    --feature <dir>                    Pick the plan set when an ID exists in several.

IDs are case-insensitive: p02.t5.a is read as P02.T5.a.

Task markers the parser understands: `### P01.T1 Title` / `#### P01.T1.a Title` headings,
`**P05.T1.a Title.** body` paragraphs and `- **P08.T2.a Title.** body` bullets. A subtask also
gets its parent's intro, the intro of the enclosing `### Phase …` heading, and the paragraphs its
parent shares among all subtasks: a labelled paragraph (Files, Accept, Acceptance, Tests) that says
so ("Acceptance for T6", "Files (all of T2)"), and the labelled paragraphs after the last subtask
whose label no earlier subtask uses for a paragraph of its own.

Design notes: `28. **Title.**` numbered items and `**D4. Title.**` or `**D8a. Title.**`
paragraphs, cited as "Design note 7", "design notes 7 and 13", "notes 8–10", "D4" or "D8a". A
citation with a plan named next to it ("plan 04's design note 15", "(plan 04, note 12)", "D8 of
plan 02") or an "its D6" after a plan named in the same paragraph is looked up in that plan.
"""

from __future__ import annotations

import argparse
import re
import shlex
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

sys.dont_write_bytecode = True

ID_RE = re.compile(r"P(\d{2})\.T(\d+)(?:\.([a-z]+))?")
LOOSE_ID_RE = re.compile(r"[Pp](\d{2})\.[Tt](\d+)(?:\.([A-Za-z]+))?")
HEADING_TASK_RE = re.compile(r"^(#{2,6})\s+(P\d{2}\.T\d+(?:\.[a-z]+)?)\b\s*(.*)$")
INLINE_TASK_RE = re.compile(r"^(?:[-*]\s+)?\*\*(P\d{2}\.T\d+(?:\.[a-z]+)?)\b\s*([^*]*?)\.?\*\*")
HEADING_RE = re.compile(r"^(#{1,6})\s")
BULLET_RE = re.compile(r"^\s*[-*]\s")
NOTE_START_RES = (
    re.compile(r"^(\d+)\.\s+\*\*"),  # 28. **Title.**
    re.compile(r"^\*\*D(\d+[a-z]?)\.\s"),  # **D4. Title.**, **D8a. Title.**
    re.compile(r"^[-*]\s+\*\*D(\d+[a-z]?)\.\s"),  # - **D4. Title.**
    re.compile(r"^#{3,5}\s+D(\d+[a-z]?)\b"),  # ### D4 Title
)
NOTE_KEY = r"\d{1,2}[a-z]?(?!\w)"
NOTE_REF_RES = (
    # "D4", "D8a", "D8–D10"
    re.compile(r"\bD" + NOTE_KEY + r"(?:\s*[–-]\s*D" + NOTE_KEY + r")?"),
    # "Design note 7", "design notes 7 and 13", "notes 8–10", and a bare "note 5" after a first citation.
    re.compile(r"\b(?:[Dd]esign\s+)?[Nn]otes?\s+(D?" + NOTE_KEY + r"(?:\s*(?:,|and|or|–|-)\s*D?" + NOTE_KEY + r")*)"),
)
NOTE_ITEM_RE = re.compile(r"D?(\d{1,2}[a-z]?)(?:\s*[–-]\s*D?(\d{1,2}[a-z]?))?")
# Another plan named right before or after a citation, or "its" referring back to one.
PLAN_BEFORE_RE = re.compile(r"\b[Pp]lans?\s+(\d{1,2})(?:['’]s)?\s*,?\s*$")
PLAN_AFTER_RE = re.compile(r"\s+(?:of|in|from)\s+[Pp]lan\s+(\d{1,2})\b")
ITS_BEFORE_RE = re.compile(r"\b[Ii]ts\s+$")
PLAN_NAME_RE = re.compile(r"\b[Pp]lan\s+(\d{1,2})\b")
PARAGRAPH_BREAK_RE = re.compile(r"\n\s*\n|\n\s*[-*]\s")
COMMAND_RE = re.compile(r"`((?:just|cargo|pnpm|grep|rg|git|python3?|uvx|wasmtime)\b[^`]*)`")
ACCEPT_RE = re.compile(r"(?<![A-Za-z])Accept(?:ance)?\b")
# A paragraph or bullet that starts with one of these labels: "Files:", "- **Accept:**", "_Tests:_".
LABEL_RE = re.compile(r"^\s*(?:[-*]\s+)?(?:\*\*|_)?(Files|Accept(?:ance)?|Tests?)\b")
SHARED_RE = re.compile(r"\((?:all of|for all of) T\d+\)|\bfor T\d+\b(?!\.)|\beach subtask\b|\ball subtasks\b", re.I)
CLAUSE_SPLIT_RE = re.compile(r"(?<=;)\s+|(?<=\.)\s+(?=[A-Z`])|(?<=\.\*\*)\s+")
CARGO_VALUE_FLAGS = {
    "-p", "--package", "--test", "--bench", "--example", "--bin", "--exclude", "-F", "--features",
    "--target", "--target-dir", "--profile", "--manifest-path", "-j", "--jobs", "--color",
    "--message-format", "--config", "-Z",
}
GREP_VALUE_FLAGS = {"-m", "--max-count", "-A", "-B", "-C", "-d", "-D", "-g", "--glob", "-t", "--type", "-T"}


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


def normalise_id(task_id: str) -> str:
    """P and T upper case, the subtask letter lower case: `p02.t5.A` -> `P02.T5.a`."""
    m = LOOSE_ID_RE.fullmatch(task_id.strip())
    if m is None:
        return task_id
    return f"P{m.group(1)}.T{m.group(2)}" + (f".{m.group(3).lower()}" if m.group(3) else "")


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


def design_notes(lines: list[str]) -> dict[str, str]:
    """This plan's design notes by key: "7", or "8a" for a lettered note."""
    span = named_section(lines, "design notes")
    if span is None:
        return {}
    start, end = span
    starts: list[tuple[int, str]] = []
    for i in range(start + 1, end):
        for pattern in NOTE_START_RES:
            if m := pattern.match(lines[i]):
                starts.append((i, m.group(1)))
                break
    notes = {}
    for k, (i, key) in enumerate(starts):
        stop = starts[k + 1][0] if k + 1 < len(starts) else end
        # A sub-heading between two notes (plan 04 groups its notes) belongs to neither.
        stop = next((j for j in range(i + 1, stop) if heading_level(lines[j]) is not None), stop)
        notes[key] = "\n".join(lines[i:stop]).rstrip()
    return notes


def note_order(key: str) -> tuple[int, str]:
    return int(key.rstrip("abcdefghijklmnopqrstuvwxyz")), key.lstrip("0123456789")


def note_keys(listed: str) -> list[str]:
    """Keys of a citation's list: "7 and 13" -> 7, 13; "8–10" -> 8, 9, 10; "D8a" -> 8a."""
    keys = []
    for m in NOTE_ITEM_RE.finditer(listed):
        first, last = m.group(1), m.group(2)
        if last and first.isdigit() and last.isdigit() and 0 < int(last) - int(first) <= 20:
            keys += [str(n) for n in range(int(first), int(last) + 1)]
        else:
            keys += [k for k in (first, last) if k]
    return keys


def note_owner(text: str, start: int, end: int, plan_no: str) -> str | None:
    """The number of the plan whose note a citation names, or None for an "its" that refers to no
    plan named earlier in its paragraph."""
    before = text[max(0, start - 40) : start]
    if m := PLAN_BEFORE_RE.search(before):
        return m.group(1).zfill(2)
    if m := PLAN_AFTER_RE.match(text, end):
        return m.group(1).zfill(2)
    if ITS_BEFORE_RE.search(before):
        para_start = max((m.end() for m in PARAGRAPH_BREAK_RE.finditer(text, 0, start)), default=0)
        named = PLAN_NAME_RE.findall(text, para_start, start)
        return named[-1].zfill(2) if named else None
    return plan_no


def cited_notes(text: str, plan_no: str) -> list[tuple[str, str]]:
    """(plan number, note key) for every design note the text cites, this plan's first."""
    found: set[tuple[str, str]] = set()
    for pattern in NOTE_REF_RES:
        for m in pattern.finditer(text):
            owner = note_owner(text, m.start(), m.end(), plan_no)
            if owner is None:
                continue
            for key in note_keys(m.group(0) if pattern is NOTE_REF_RES[0] else m.group(1)):
                found.add((owner, key))
    return sorted(found, key=lambda c: (c[0] != plan_no, c[0], note_order(c[1])))


def paragraph_spans(lines: list[str], start: int, end: int) -> list[tuple[int, int]]:
    """(first, stop) line ranges of the paragraphs in lines[start:end], split on blank lines and on
    bullets, so each bullet is its own paragraph."""
    spans: list[list[int]] = []
    open_paragraph = False
    for i in range(start, end):
        if not lines[i].strip():
            open_paragraph = False
        elif BULLET_RE.match(lines[i]) or not open_paragraph:
            spans.append([i, i + 1])
            open_paragraph = True
        else:
            spans[-1][1] = i + 1
    return [(a, b) for a, b in spans]


def paragraphs(block: list[str]) -> list[str]:
    """Split on blank lines and on bullets, so each bullet is its own paragraph."""
    return ["\n".join(block[a:b]) for a, b in paragraph_spans(block, 0, len(block))]


def span_lines(lines: list[str], spans: list[tuple[int, int]]) -> list[str]:
    """The spans' lines in order, with one blank line where the plan had a gap between them."""
    out: list[str] = []
    for k, (a, b) in enumerate(spans):
        if k and a > spans[k - 1][1]:
            out.append("")
        out += lines[a:b]
    return out


def label_of(line: str) -> str | None:
    """The label a paragraph's first line starts with, as "file", "acce" or "test" (Accept and
    Acceptance are one label, as are Test and Tests), or None."""
    m = LABEL_RE.match(line)
    return m.group(1)[:4].lower() if m else None


def load(root: Path, task_id: str, feature: str | None):
    m = ID_RE.fullmatch(task_id)
    if m is None:
        sys.exit(f"{task_id!r} is not a task ID like P02.T5 or P05.T1.a")
    plan = find_plan(root, m.group(1), feature)
    lines = plan.read_text(encoding="utf-8").splitlines()
    all_markers = markers(lines)
    target = next((mk for mk in all_markers if mk.task_id == task_id), None)
    if target is None:
        ids = [mk.task_id for mk in all_markers]
        known = ", ".join(ids[:60])
        if len(ids) > 60:
            known += f", … and {len(ids) - 60} more (`--list P{m.group(1)}` prints them all)"
        sys.exit(f"{task_id} not found in {plan.relative_to(root)}. Tasks there: {known}")
    return plan, lines, all_markers, target


def parent_of(task_id: str, all_markers: list[Marker]) -> Marker | None:
    parts = task_id.split(".")
    if len(parts) < 3:
        return None
    return next((mk for mk in all_markers if mk.task_id == ".".join(parts[:2])), None)


def phase_intro(lines: list[str], all_markers: list[Marker], top: Marker) -> tuple[str, list[str]]:
    """The enclosing `### Phase …` or `### Track …` heading of a task heading, and the intro under
    it that binds every task of the phase. Empty when there is no such heading or no intro."""
    tasks = named_section(lines, "tasks")
    if top.level > 6 or tasks is None:
        return "", []
    for i in range(top.line - 1, tasks[0], -1):
        level = heading_level(lines[i])
        if level is None or level >= top.level:
            continue
        if HEADING_TASK_RE.match(lines[i]):
            return "", []
        first = next((mk.line for mk in all_markers if mk.line > i), top.line)
        stop = next((j for j in range(i + 1, first) if heading_level(lines[j]) is not None), first)
        intro = "\n".join(lines[i + 1 : stop]).strip()
        return (lines[i], intro.splitlines()) if intro else ("", [])
    return "", []


def subtasks(lines: list[str], all_markers: list[Marker], parent: Marker):
    """A parent task's subtasks, each subtask's own paragraphs (line spans), and the paragraphs
    shared by all of them. A paragraph that starts with a task marker is never shared. A labelled
    paragraph is shared when it says so; so is everything from the first labelled paragraph after
    the last subtask whose label no earlier subtask uses for a paragraph of its own (P01.T4's
    `Files: … Acceptance: …`, P06.T5's `- **Files:** … **Accept:** …`, P14.T6's `- _Accept:_`).
    Plans that give each subtask its own labelled paragraphs (plan 05) keep the last one's as its own.
    """
    parent_end = section_end(lines, parent, all_markers)
    children = [
        mk for mk in all_markers if parent.line < mk.line < parent_end and mk.task_id.startswith(parent.task_id + ".")
    ]
    if not children:
        return parent_end, children, {}, []
    regions = []
    for k, child in enumerate(children):
        stop = children[k + 1].line if k + 1 < len(children) else parent_end
        regions.append(paragraph_spans(lines, child.line, stop))
    shared = {
        span
        for spans in regions
        for span in spans[1:]
        if label_of(lines[span[0]]) and SHARED_RE.search("\n".join(lines[span[0] : span[1]]))
    }
    earlier = {label_of(lines[a]) for spans in regions[:-1] for a, _ in spans[1:]} - {None}
    last = regions[-1]
    for k in range(1, len(last)):
        label = label_of(lines[last[k][0]])
        if label and (last[k] in shared or label not in earlier):
            shared.update(last[k:])
            break
    own = {child.task_id: [s for s in spans if s not in shared] for child, spans in zip(children, regions)}
    return parent_end, children, own, sorted(shared)


def task_blocks(lines, all_markers, target) -> tuple[list[str], tuple[int, int], tuple[int, int] | None]:
    """The text to show: the phase intro, then for a subtask the parent's intro, the subtask, and
    the parent's paragraphs shared by all subtasks ("Acceptance for T6, and for each subtask",
    "Files (all of T2)", or the labelled paragraphs after the last subtask)."""
    end = section_end(lines, target, all_markers)
    parent = parent_of(target.task_id, all_markers)
    heading, intro_lines = phase_intro(lines, all_markers, parent or target)
    phase = [heading, "", *intro_lines, ""] if heading else []
    if parent is None:
        return phase + lines[target.line : end], (target.line, end), None
    parent_end, children, own, shared = subtasks(lines, all_markers, parent)
    if target.task_id not in own:
        return phase + lines[target.line : end], (target.line, end), (parent.line, parent_end)
    mine = own[target.task_id]
    intro = "\n".join(lines[parent.line : children[0].line]).rstrip().splitlines()
    block = phase + intro + ["", "[…]", ""] + span_lines(lines, mine)
    if shared:
        block += ["", "[Shared by all subtasks of " + parent.task_id + "]", ""] + span_lines(lines, shared)
    return block, (target.line, mine[-1][1]), (parent.line, parent_end)


def acceptance_groups(lines, all_markers, target) -> list[tuple[str, list[str]]]:
    """(label, paragraphs) in plan order: the phase intro, the parent's intro, each subtask's own
    paragraphs (only the target's, for a subtask) and the paragraphs shared by all subtasks."""
    parent = parent_of(target.task_id, all_markers)
    top = parent or target
    groups = []
    heading, intro_lines = phase_intro(lines, all_markers, top)
    if heading:
        groups.append((f"[{heading.lstrip('#').strip()}, intro]", paragraphs(intro_lines)))
    _, children, own, shared = subtasks(lines, all_markers, top)
    if not children or (parent is not None and target.task_id not in own):
        end = section_end(lines, target, all_markers)
        return groups + [(f"[{target.task_id}]", paragraphs(lines[target.line : end]))]
    groups.append((f"[{top.task_id}, intro]", paragraphs(lines[top.line : children[0].line])))
    for child in children:
        if parent is None or child.task_id == target.task_id:
            groups.append((f"[{child.task_id}]", ["\n".join(lines[a:b]) for a, b in own[child.task_id]]))
    groups.append((f"[Shared by all subtasks of {top.task_id}]", ["\n".join(lines[a:b]) for a, b in shared]))
    return groups


def mentions(root: Path, plan: Path, lines: list[str], task_id: str, span: tuple[int, int], limit: int = 20) -> list[str]:
    """Lines elsewhere in the plan set that name this task: the full ID in any plan, and the short
    form (T6.e, or T6 for a parent) in its own plan, outside the task's own section. A short form in
    backticks is code (the spectral type `T6`), not a task."""
    short = task_id.split(".", 1)[1]
    short_re = re.compile(r"(?<![\w.`])" + re.escape(short) + r"(?![\w`])")
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


def tasks_intro(lines: list[str], all_markers: list[Marker]) -> str:
    """The Tasks section's text before the first task, without its headings (plan 04's `### Order
    and parallelism`) and stopping at the first heading after the text (plan 06's `### Phase A`)."""
    tasks = named_section(lines, "tasks")
    if tasks is None:
        return ""
    first = next((mk.line for mk in all_markers if mk.line > tasks[0]), tasks[1])
    out: list[str] = []
    for line in lines[tasks[0] + 1 : first]:
        if heading_level(line) is not None:
            if any(o.strip() for o in out):
                break
            continue
        out.append(line)
    return "\n".join(out).strip()


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

    intro = tasks_intro(lines, all_markers)
    if intro:
        print("## Order and parallelism (from the Tasks intro)\n")
        print(intro)
        print()

    print("## Task\n")
    print("\n".join(block).rstrip())
    print()

    plan_no = task_id[1:3]
    notes = {plan_no: (plan, design_notes(lines))}
    cited = []
    for owner, key in cited_notes("\n".join(block), plan_no):
        if owner not in notes:
            other = next(iter(sorted(plan.parent.glob(f"{owner}-*.md"))), None)
            if other is None:
                continue
            notes[owner] = (other, design_notes(other.read_text(encoding="utf-8").splitlines()))
        if key in notes[owner][1]:
            cited.append((owner, key))
    if cited:
        print("## Design notes cited by the task (this plan's, unless labelled with another plan)\n")
        for owner, key in cited:
            if owner != plan_no:
                print(f"[Plan {owner}'s design note {key}, {notes[owner][0].name}]")
            print(notes[owner][1][key])
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


def cargo_filters(cmd: str) -> tuple[list[str], list[str], list[str]] | None:
    """For `cargo test` or `cargo bench`: the options, the positional test-name filters, and the
    arguments after `--`."""
    try:
        words = shlex.split(re.split(r"\s(?:&&|\|\||\||;)\s", cmd)[0])
    except ValueError:
        return None
    if words[:2] not in (["cargo", "test"], ["cargo", "bench"]):
        return None
    options, filters, rest = words[:2], [], words[2:]
    i = 0
    while i < len(rest):
        if rest[i] == "--":
            return options, filters, rest[i + 1 :]
        if rest[i] in CARGO_VALUE_FLAGS:
            options += rest[i : i + 2]
            i += 2
            continue
        (options if rest[i].startswith("-") else filters).append(rest[i])
        i += 1
    return options, filters, []


def reads_stdin(cmd: str) -> bool:
    """A `grep` or `rg` given a pattern and no file reads standard input."""
    try:
        words = shlex.split(cmd)
    except ValueError:
        return False
    if not words or words[0] not in ("grep", "rg") or {"|", "<"} & set(words):
        return False
    pattern_given, operands, i = False, [], 1
    while i < len(words):
        word = words[i]
        if word in ("-e", "--regexp", "-f", "--file"):
            pattern_given = True
            i += 2
            continue
        if word in GREP_VALUE_FLAGS:
            i += 2
            continue
        if word.startswith("-") and len(word) > 1:
            if words[0] == "grep" and re.fullmatch(r"-[A-Za-z]*[rR][A-Za-z]*|--(?:dereference-)?recursive", word):
                return False  # grep -r searches the working directory
            i += 1
            continue
        operands.append(word)
        i += 1
    return len(operands) < (1 if pattern_given else 2)


def risk_notes(rel: Path, lines: list[str], task_id: str) -> list[str]:
    """Bullets of the plan's Risks and open points that name the task, or, for a subtask, its parent
    as a whole: the whole bullet when its title names it, else only the clauses that do."""
    span = named_section(lines, "risks")
    if span is None:
        return []
    short = task_id.split(".", 1)[1]
    names = [re.escape(short)]
    if short.count("."):
        names.append(re.escape(short.split(".")[0]) + r"(?!\.[a-z])")
    name_re = re.compile(r"(?<![\w.`])(?:P" + task_id[1:3] + r"\.)?(?:" + "|".join(names) + r")(?![\w`])")
    found = []
    for a, b in paragraph_spans(lines, span[0] + 1, span[1]):
        text = re.sub(r"^[-*]\s+", "", " ".join(" ".join(lines[a:b]).split()))
        clauses = CLAUSE_SPLIT_RE.split(text)  # the first is the bold title, or its first sentence
        title = re.match(r"\*\*.+?\*\*", text)
        if name_re.search(clauses[0]):
            found.append(f"{rel}:{a + 1}: {text}")
        elif hits := [c for c in clauses[1:] if name_re.search(c)]:
            found.append(f"{rel}:{a + 1}: " + (title.group(0) + " … " if title else "") + " … ".join(hits))
    return found


def show_acceptance(root: Path, task_id: str, feature: str | None) -> None:
    plan, lines, all_markers, target = load(root, task_id, feature)
    rel = plan.relative_to(root)
    groups = [
        (label, [p for p in paras if ACCEPT_RE.search(p)])
        for label, paras in acceptance_groups(lines, all_markers, target)
    ]
    groups = [(label, hits) for label, hits in groups if hits]
    risks = risk_notes(rel, lines, task_id)
    print(f"# Acceptance for {task_id} ({rel})\n")
    if not groups:
        print("(no acceptance paragraph found; the roadmap still requires `just ci` green)")
    for label, hits in groups:
        print(label)
        for p in hits:
            print(p.strip())
            print()
    commands = []
    for _, hits in groups:
        for p in hits:
            for cmd in COMMAND_RE.findall(p):
                cmd = " ".join(cmd.split())
                if cmd not in commands:
                    commands.append(cmd)
    multi = {}
    for cmd in commands:
        parsed = cargo_filters(cmd)
        if parsed and len(parsed[1]) > 1:
            options, filters, after = parsed
            multi[cmd] = shlex.join(options + ["--"] + filters + after)
    if commands:
        print("## Commands quoted in the acceptance criteria\n")
        for cmd in commands:
            if cmd in multi:
                print(f"- `{cmd}`  (invalid: more than one filter; run `{multi[cmd]}`)")
            elif reads_stdin(cmd):
                print(f"- `{cmd}`  (no file given; run against the files the plan names)")
            else:
                print(f"- `{cmd}`")
    if multi:
        print("\nNote: cargo takes one positional test-name filter, so a second one is an \"unexpected argument\".")
        print("Several filters go after `--`, where the test harness runs every test that matches any of them.")
    bare = [c for c in commands if re.fullmatch(r"cargo test -p \S+ [\w:]+", c)]
    if bare:
        print("\nNote: a bare filter (`cargo test -p <crate> <name>`) matches test *names*, not files.")
        print("Check that it selects the task's tests; `--test <file>` runs one integration-test file,")
        print("and `--lib <module path>` the unit tests of a module.")
    manual = [p for _, hits in groups for p in hits if re.search(r"by hand|by eye|do not commit|manually", p, re.I)]
    if manual:
        print("\n## Contains manual steps (run them, revert any temporary edit, report the result)")
    if risks:
        if commands or manual or not groups:  # otherwise the last paragraph ended with a blank line
            print()
        print(f"## Risks and open points naming {task_id} (as-built corrections; they override the task text)\n")
        for note in risks:
            print(f"- {note}")


def commit_subjects(root: Path) -> str:
    try:
        out = subprocess.run(
            ["git", "-C", str(root), "log", "--format=%s"], capture_output=True, text=True, check=True
        )
        return out.stdout
    except (OSError, subprocess.CalledProcessError):
        return ""


def show_list(root: Path, plan_filter: str | None, feature: str | None) -> None:
    log = commit_subjects(root)
    plan_no = None
    if plan_filter:
        m = re.fullmatch(r"[Pp]?(\d{1,2})", plan_filter)
        if m is None:
            sys.exit(f"{plan_filter!r} is not a plan number like P02 or 2")
        plan_no = m.group(1).zfill(2)
    found = False
    for plan_set in plan_sets(root, feature):
        files = sorted(plan_set.glob(f"{plan_no}-*.md" if plan_no else "[0-9][0-9]-*.md"))
        for plan in files:
            found = True
            lines = plan.read_text(encoding="utf-8").splitlines()
            print(f"## {plan.relative_to(root)}")
            for mk in markers(lines):
                named = re.search(r"^" + re.escape(mk.task_id) + r"(?![\w.])", log, re.M) is not None
                indent = "  " if mk.task_id.count(".") == 2 else ""
                flag = "  [commit]" if named else ""
                print(f"{indent}{mk.task_id}  {mk.title}{flag}")
            print()
    if plan_no and not found:
        sys.exit(f"no plan numbered {plan_no} under docs/agent/plans/*/")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("task_id", nargs="?", help="task ID such as P02.T5 or P05.T1.a")
    parser.add_argument("--acceptance", action="store_true", help="print only the acceptance criteria")
    parser.add_argument("--context", action="store_true", help="also print the Generator version and Risks sections")
    parser.add_argument("--list", nargs="?", const="", metavar="PLAN", help="list tasks, optionally of one plan")
    parser.add_argument("--feature", help="plan set directory under docs/agent/plans/")
    args = parser.parse_args()
    root = repo_root()
    task_id = normalise_id(args.task_id) if args.task_id else None

    if args.list is not None:
        show_list(root, args.list or args.task_id, args.feature)
    elif task_id is None:
        parser.print_help()
    elif args.acceptance:
        show_acceptance(root, task_id, args.feature)
    else:
        show_task(root, task_id, args.feature, args.context)


if __name__ == "__main__":
    main()
