#!/usr/bin/env python3
"""Split an action plan's as-built records out of its Risks section, one file per task.

Usage (from the repository root, as `python3 -I`):
    plan_split.py PLAN --check          Split in memory and verify it; write nothing.
    plan_split.py PLAN --dry-run DIR    Split, write the plan, its record files and a report
                                        (`<stem>.split.json`) under DIR/<plan-set>/, read them back
                                        and verify them. The plan and its records are not touched.
    plan_split.py PLAN --write          Split in place: rewrite the plan and write
                                        `<plan-dir>/<plan-stem>/records/<KEY>.md`, then read them
                                        back and verify them.
    plan_split.py PLAN --verify REV     Check a committed split: split PLAN as it was at git
                                        revision REV (with its record files then) and compare the
                                        result with the working tree, byte for byte, with the
                                        overrides the working tree's marks record (below) beside
                                        any given.
    --assign 'TEXT=ID'                  File the Risks bullet whose first line starts with TEXT
                                        (after its `- `) under task ID (repeatable), now and on
                                        every later split: it gains a file mark naming the task as
                                        its last line, which moves with it into its record file.
    --keep 'TEXT'                       Keep that bullet in Risks, now and on every later split:
                                        a bullet the rule would move gains a keep mark as its last
                                        line (repeatable). A bullet marked otherwise refuses
                                        either override: delete the mark's line first.
    --json                              Print the report as JSON; --verbose lists each bullet moved.

What moves: each top-level bullet of "Risks and open points" that belongs to one task of the plan
by plan_task.assign_records()'s rule, filed under plan_task.record_file_key(); a bullet stays in
Risks when it names no task of the plan (an open point, a ruling across tasks), when it is a
re-validation, when it is under a sub-heading of the section, and when it has a keep mark. The
marks are lines of their own inside a bullet, at its text's indent, which Prettier leaves as they
are: `<!-- plan-split: keep -->` (plan_task.KEEP_MARK), which `--keep` adds, keeps it in Risks, and
`<!-- plan-split: file under P06.T28.f -->` (plan_task.FILE_MARK), which `--assign` adds, files it
under that task, under a sub-heading too. So a bullet lies where its override put it on every
later split, run without the flag (as implement-task's records step runs it), wherever it is
moved or edited; and `--verify` reads a committed split's marks as the overrides it was made
with. The check takes a mark the split adds out again, from the plan or a record file, as it
does a link line. Every bullet that stays is
reported, with its first line and why, and so is a line that keeps Prettier from formatting the
rest of the plan (a warning, not a failure). Nothing outside Risks moves. The plan gains RECORDS_INDEX after the Risks
heading, and each task with records gains RECORD_LINE in its entry under Tasks: a paragraph of its
own under a heading, else a last line of the entry's first paragraph. Each record file opens with
RECORD_HEADER and holds the task's bullets verbatim, in the plan's order, each with the blank line
it had after it (Prettier's own: a bullet with a blank line inside is followed by one).

A plan already split is split again the same way: a bullet added to its Risks since is appended to
its task's file, which is created with its header and its task's link line if it is the first.

The check (every mode): the new plan and the record files are read back as written, the inserted
lines are taken out, and the bullets are put back in their places: the result must equal the
original byte for byte, every moved bullet in exactly one file, nothing outside Risks changed.
Then the layout: each record file is a task's, with its header and its link line, and each link
line points at a file. Exit status: 0 done, 1 a check failed, 2 usage. Line endings (LF or CRLF)
are kept as they are; new lines take the plan's. Standard library only; Python 3.9+.
Tests: test_plan_split.py beside this script.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import plan_task as pt  # noqa: E402  (the script's directory is put on the path first)

EXIT_OK, EXIT_FAIL, EXIT_USAGE = 0, 1, 2
BLOCK_START_RE = re.compile(r"^\s*(?:[-*+]\s|\d+[.)]\s|```|~~~|\||>|#)")
# A line that opens CommonMark's first kind of HTML block, which runs to its closing tag: with none,
# the rest of its container is raw HTML, which Prettier leaves unformatted and GitHub shows raw.
HTML_RUN_RE = re.compile(r"^\s*<(script|pre|style|textarea)(?=[\s>]|$)", re.I)


def html_hazards(name: str, p: list[str]) -> list[str]:
    """Lines that open an HTML block no later line closes (R07's `<style drawn>`, a code span
    wrapped so that a line starts with it)."""
    found = []
    for i, line in enumerate(p):
        if m := HTML_RUN_RE.match(line):
            close = f"</{m.group(1).lower()}>"
            if not any(close in later.lower() for later in p[i:]):
                found.append(f"{name}:{i + 1} starts with <{m.group(1)}, an HTML block no line closes: Prettier formats"
                             " nothing after it, so records moved from there may fail its check; rewrap the line"
                             " first and run Prettier on the plan")
    return found


class SplitError(Exception):
    """A check failed: the split would lose, duplicate or change text."""


class UsageError(SplitError):
    """An --assign or --keep that matches no bullet, or several, or names no task."""


def split_lines(text: str) -> list[str]:
    """The text's lines, each with its own ending (`\\n` or `\\r\\n`), split on `\\n` only (not on
    the other characters str.splitlines() takes as line breaks); a last line without one keeps
    none."""
    parts = text.split("\n")
    lines = [p + "\n" for p in parts[:-1]]
    if parts[-1]:
        lines.append(parts[-1])
    return lines


def plain(line: str) -> str:
    """A line without its ending."""
    if line.endswith("\n"):
        line = line[:-1]
    if line.endswith("\r"):
        line = line[:-1]
    return line


def newline_of(lines: list[str]) -> str:
    return "\r\n" if lines and lines[0].endswith("\r\n") else "\n"


def with_newline(text: str, eol: str) -> str:
    return text.replace("\n", eol) if eol != "\n" else text


def is_spread(lines: list[str], start: int, body_end: int) -> bool:
    """A bullet with a blank line inside (remark's loose item): Prettier puts a blank line after it."""
    return any(not lines[i].strip() for i in range(start + 1, body_end))


@dataclass
class Container:
    """Where a run of bullets goes: the plan's Risks or one record file."""

    key: str | None  # None for the plan
    prefix: str = ""  # an existing record file's text, kept as it is
    join: str = ""  # between that text and the first new bullet
    header: str = ""  # a new record file's header
    items: list[int] = field(default_factory=list)  # indices into the Risks items
    trails: dict[int, str] = field(default_factory=dict)  # position -> the blank lines it had after it, where changed
    marks: dict[int, str] = field(default_factory=dict)  # position -> the mark line the split gave it


@dataclass
class Result:
    plan: Path
    original: str
    text: str  # the new plan
    records: dict[str, str]  # key -> the record file's new text (only files that change)
    existing: dict[str, str]  # key -> an existing record file's text
    items: list[pt.RiskItem]
    first_lines: list[str]
    inserted: list[tuple[int, list[str]]]  # (line in the new plan, the lines inserted there)
    containers: dict[str | None, Container]
    order: list[str | None]  # each Risks item's container, in the plan's order
    links: dict[str, str]  # key -> "added" or "present"
    head_len: int  # the Risks heading and the lines before its first item
    marked: dict[int, str]  # Risks item index -> the mark `--keep` or `--assign` gave it

    @property
    def moved(self) -> list[pt.RiskItem]:
        return [it for it in self.items if it.key is not None]

    @property
    def kept(self) -> list[pt.RiskItem]:
        return [it for it in self.items if it.key is None]


def first_line(line: str) -> str:
    return re.sub(r"^(?:[-*+]|\d+[.)])\s+", "", plain(line)).strip()


def fence_open(p: list[str], item: pt.RiskItem) -> bool:
    """Whether a bullet ends inside a code fence, where a mark would be code."""
    return sum(p[i].lstrip().startswith(("```", "~~~")) for i in range(item.start, item.body_end)) % 2 == 1


def apply_overrides(p, result_items, firsts, assign, keep, ids) -> tuple[list[str], dict[int, str]]:
    """Apply `--assign` and `--keep` to the Risks items: the problems, and the mark each item that
    an override moves from where the rule puts it is to gain (by its index): a file mark naming the
    task for `--assign`, the keep mark for `--keep`. A bullet the rule puts there already, or marked
    so, needs none; one marked otherwise refuses the override, as does one that ends inside a code
    fence."""
    problems: list[str] = []
    marks: dict[int, str] = {}

    def find(text: str) -> int | None:
        hits = [k for k, (it, f) in enumerate(zip(result_items, firsts)) if it.kind == "bullet" and f.startswith(text)]
        if len(hits) != 1:
            problems.append(f"{len(hits)} Risks bullets start with {text!r}, not one")
            return None
        return hits[0]

    for text, task in assign:
        task = pt.normalise_id(task)
        key = pt.record_file_key(task, ids)
        if key is None:
            problems.append(f"--assign {text!r}: {task} has no entry under Tasks, nor has its parent")
            continue
        if (k := find(text)) is None:
            continue
        item, mark = result_items[k], pt.split_mark(p, result_items[k])
        filed = pt.FILE_MARK_RE.fullmatch(mark or "")
        if mark is not None and not (filed and pt.record_file_key(filed.group(1), ids) == key):
            problems.append(f"--assign {text!r}: the bullet has the mark `{mark}`; delete its line first")
        elif mark is None and item.key == key:
            pass  # the rule files it there already
        elif mark is None and fence_open(p, item):
            problems.append(f"--assign {text!r}: the bullet ends inside a code fence; close it first")
        elif mark is None:
            item.key, item.why = key, f"--assign {task}, marked"
            marks[k] = pt.FILE_MARK.format(task=task)
    for text in keep:
        if (k := find(text)) is None or result_items[k].key is None:
            continue  # the rule keeps it already (a keep mark among the reasons)
        item, mark = result_items[k], pt.split_mark(p, result_items[k])
        if mark is not None or k in marks:
            problems.append(f"--keep {text!r}: the bullet has the mark `{mark or marks[k]}` (or an --assign); "
                            "delete its line first")
        elif fence_open(p, item):
            problems.append(f"--keep {text!r}: the bullet ends inside a code fence; close it first")
        else:
            item.key, item.why = None, "--keep, marked"
            marks[k] = pt.KEEP_MARK
    return problems, marks


def link_insertion(p: list[str], marker: pt.Marker, marker_lines: set[int], line: str) -> tuple[int, list[str]]:
    """Where a task's link line goes (a line index of the original plan) and the lines inserted:
    under a heading, a paragraph of its own after the heading; in a bullet or paragraph, a last
    line of its first paragraph, at the paragraph's indent, before a closing lead-in such as
    `Acceptance:` that introduces a list."""
    i = marker.line
    if marker.level <= 6:
        if i + 1 < len(p) and not p[i + 1].strip():
            return i + 2, [line, ""]
        return i + 1, ["", line, ""]
    j = i + 1
    while j < len(p) and p[j].strip() and j not in marker_lines and not BLOCK_START_RE.match(p[j]):
        j += 1
    indent = re.match(r"^(?:[-*+]\s+)?", p[i]).end()
    if j - 1 > i and pt.LEAD_IN_RE.search(p[j - 1]):
        j -= 1
    return j, [" " * indent + line]


def split_text(plan: Path, original: str, existing: dict[str, str], assign=(), keep=()) -> Result:
    """Split a plan's text (pure: `existing` holds its record files as they are). Raises SplitError
    when the plan has no Risks section or an override is wrong."""
    lines = split_lines(original)
    p = [plain(line) for line in lines]
    eol = newline_of(lines)
    span = pt.named_section(p, "risks")
    if span is None:
        raise SplitError(f"{plan.name}: no `## Risks and open points` section")
    prefix = pt.plan_prefix(plan)
    if prefix is None:
        raise UsageError(f"{plan}: its plan set {plan.parent.name!r} has no task-ID prefix")
    all_markers = pt.markers(p)
    tasks = pt.task_markers(p, all_markers)
    ids = {m.task_id for m in tasks}
    items = pt.risk_items(p, span)
    pt.assign_records(p, items, prefix, ids)
    firsts = [first_line(lines[it.start]) for it in items]
    problems, marks = apply_overrides(p, items, firsts, assign, keep, ids)
    if problems:
        raise UsageError("; ".join(problems))
    index = pt.RECORDS_INDEX.format(stem=plan.stem, dir=pt.RECORDS_DIR)
    already = any(it.kind == "para" and p[it.start] == index for it in items)

    # The containers, in the order their first bullet comes.
    containers: dict[str | None, Container] = {None: Container(None)}
    order: list[str | None] = []
    for k, it in enumerate(items):
        if it.key not in containers:
            c = Container(it.key)
            if it.key in existing:
                c.prefix = existing[it.key]
                if not c.prefix.endswith("\n"):
                    raise SplitError(f"{pt.record_rel(plan, it.key)} does not end with a line break")
                ex = split_lines(c.prefix)
                ep = [plain(x) for x in ex]
                body = pt.top_items(ep, 0, len(ep))
                last = body[-1] if body else None
                c.join = eol if last is not None and last.kind == "bullet" and is_spread(ep, last.start, last.body_end) else ""
            else:
                c.header = with_newline(pt.RECORD_HEADER.format(key=it.key, plan=plan.name), eol)
            containers[it.key] = c
        containers[it.key].items.append(k)
        order.append(it.key)
        if it.key is not None and it.key in existing:
            text = "".join(lines[it.start : it.body_end])
            if text in existing[it.key]:
                raise SplitError(f"{firsts[k][:60]!r} is already in {pt.record_rel(plan, it.key)}")

    def run(c: Container, end_trail: list[str]) -> list[str]:
        """The container's items, each followed by the blank lines it had after it, except that a
        list ends with a blank line before a heading or a paragraph, and the last item takes
        end_trail. Each trail changed so is kept in c.trails, to be put back. An item an override
        marks gains its mark as its last line, at its text's indent, kept in c.marks."""
        out: list[str] = []
        for n, k in enumerate(c.items):
            it = items[k]
            trail = lines[it.body_end : it.end]
            if n + 1 == len(c.items):
                new = end_trail
            elif not trail and (it.kind != "bullet" or items[c.items[n + 1]].kind != "bullet"):
                new = [eol]
            else:
                new = trail
            if new != trail:
                c.trails[n] = "".join(trail)
            out += lines[it.start : it.body_end]
            if k in marks:
                c.marks[n] = " " * re.match(r"(?:[-*+]|\d+[.)])\s+", p[it.start]).end() + marks[k] + eol
                out.append(c.marks[n])
            out += new
        return out

    # The new Risks section: its heading and the blank lines after it, the index line, then the
    # items that stay, the last followed by what followed the section's last item.
    head_end = items[0].start if items else span[1]
    region_trail = lines[items[-1].body_end : items[-1].end] if items else []
    risks = lines[span[0] : head_end]
    inserted_risks: list[str] = []
    if not already and len(containers) > 1:
        pre_blank = head_end > span[0] + 1 and not p[head_end - 1].strip()
        inserted_risks = ([] if pre_blank else [""]) + [index] + ([""] if containers[None].items else [])
    region = risks + [x + eol for x in inserted_risks]
    region += run(containers[None], region_trail) if containers[None].items else region_trail

    # Link lines, for each record file that is new or whose task entry lacks its line.
    first_marker: dict[str, pt.Marker] = {}
    for m in tasks:
        first_marker.setdefault(m.task_id, m)
    marker_lines = {m.line for m in all_markers}
    insertions: list[tuple[int, list[str], str]] = []
    links: dict[str, str] = {}
    for key in containers:
        if key is None:
            continue
        line = pt.RECORD_LINE.format(rel=pt.record_rel(plan, key))
        marker = first_marker[key]
        section = range(marker.line, pt.section_end(p, marker, all_markers))
        if any(p[i].strip() == line for i in section):
            links[key] = "present"
            continue
        at, new = link_insertion(p, marker, marker_lines, line)
        if at > span[0]:
            raise SplitError(f"{key}'s link line would go at line {at + 1}, past the Risks heading")
        insertions.append((at, new, key))
        links[key] = "added"
    insertions.sort()

    out: list[str] = []
    inserted: list[tuple[int, list[str]]] = []
    cursor = 0
    for at, new, _ in insertions:
        out += lines[cursor:at]
        inserted.append((len(out), [x + eol for x in new]))
        out += [x + eol for x in new]
        cursor = at
    out += lines[cursor : span[0]]
    if inserted_risks:
        inserted.append((len(out) + len(risks), [x + eol for x in inserted_risks]))
    out += region
    out += lines[span[1] :]

    records = {}
    for key, c in containers.items():
        if key is not None:
            body = "".join(run(c, []))
            records[key] = (c.prefix + c.join + body) if c.prefix else (c.header + body)
    return Result(plan, original, "".join(out), records, dict(existing), items, firsts, inserted, containers, order,
                  links, head_end - span[0], marks)


def recombine(result: Result, text: str, records: dict[str, str]) -> str:
    """Put the plan back together from the new plan's text and the record files' texts as read back,
    with only the split's bookkeeping: where it inserted lines, how long the Risks section's head
    is, each item's container in the plan's order, and the blank lines after an item where the
    split changed them. Raises SplitError where the files do not hold what the split wrote."""
    lines = split_lines(text)
    for at, new in reversed(result.inserted):
        if lines[at : at + len(new)] != new:
            raise SplitError(f"the new plan's line {at + 1} is not the line the split inserted there")
        del lines[at : at + len(new)]
    p = [plain(x) for x in lines]
    span = pt.named_section(p, "risks")
    if span is None:
        raise SplitError("the new plan has no Risks section")
    body_start = span[0] + result.head_len
    kept = pt.top_items(p, body_start, span[1])
    if kept and kept[0].start != body_start:
        raise SplitError("the new plan's Risks section has text before its first item that the split did not put there")
    runs: dict[str | None, list[tuple[list[str], list[str]]]] = {
        None: [(lines[it.start : it.body_end], lines[it.body_end : it.end]) for it in kept]
    }
    for key, c in result.containers.items():
        if key is None:
            continue
        got = records.get(key)
        if got is None:
            raise SplitError(f"no record file for {key}")
        if c.prefix:
            if not got.startswith(c.prefix + c.join):
                raise SplitError(f"{key}'s record file does not keep its earlier text")
            body = got[len(c.prefix) + len(c.join) :]
        else:
            if not got.startswith(c.header):
                raise SplitError(f"{key}'s record file does not open with its header")
            body = got[len(c.header) :]
        bl = split_lines(body)
        bp = [plain(x) for x in bl]
        its = pt.top_items(bp, 0, len(bp))
        if any(it.kind != "bullet" for it in its):
            raise SplitError(f"{key}'s record file holds text that is not a bullet")
        runs[key] = [(bl[it.start : it.body_end], bl[it.body_end : it.end]) for it in its]
    for key, run in runs.items():
        c = result.containers[key]
        if len(run) != len(c.items):
            raise SplitError(f"{key or 'the plan'} holds {len(run)} items where the split put {len(c.items)}")
        for n, trail in c.trails.items():
            run[n] = (run[n][0], split_lines(trail))
        for n, mark in c.marks.items():
            if not run[n][0] or run[n][0][-1] != mark:
                raise SplitError(f"{key or 'the plan'}'s item {n + 1} does not end with the mark the split gave it")
            run[n] = (run[n][0][:-1], run[n][1])
    taken = {key: 0 for key in runs}
    region = lines[span[0] : body_start]
    for key in result.order:
        body, trail = runs[key][taken[key]]
        taken[key] += 1
        region += body + trail
    if not result.order:
        region = lines[span[0] : span[1]]
    return "".join(lines[: span[0]] + region + lines[span[1] :])


def check_layout(plan: Path, text: str, records: dict[str, str]) -> list[str]:
    """The split layout's own checks on the plan and all its record files (`records`: key -> text):
    each file is a task's, opens with its header and is linked from the task's entry; each link
    line points at a file; the Risks section has its index line."""
    problems = []
    lines = split_lines(text)
    p = [plain(x) for x in lines]
    all_markers = pt.markers(p)
    first: dict[str, pt.Marker] = {}
    for m in pt.task_markers(p, all_markers):
        first.setdefault(m.task_id, m)
    eol = newline_of(lines)
    for key, body in sorted(records.items()):
        rel = pt.record_rel(plan, key)
        if pt.record_key(key) != key or key not in first:
            problems.append(f"{rel}: {key} is not a task (or subtask) under Tasks")
            continue
        if not body.startswith(with_newline(pt.RECORD_HEADER.format(key=key, plan=plan.name), eol)):
            problems.append(f"{rel}: does not open with the record header")
        line = pt.RECORD_LINE.format(rel=rel)
        section = range(first[key].line, pt.section_end(p, first[key], all_markers))
        count = sum(p[i].strip() == line for i in section)
        if count != 1:
            problems.append(f"{rel}: {key}'s entry holds its link line {count} times, not once")
    link_re = re.compile(r"^\s*" + re.escape(pt.RECORD_LINE.split("[")[0]) + r"\[([^]]+)\]\(")
    for i, x in enumerate(p):
        if m := link_re.match(x):
            key = PurePosixPath(m.group(1)).stem
            if key not in records or m.group(1) != pt.record_rel(plan, key):
                problems.append(f"{plan.name}:{i + 1}: links {m.group(1)}, which is not one of its record files")
    if records:
        index = pt.RECORDS_INDEX.format(stem=plan.stem, dir=pt.RECORDS_DIR)
        span = pt.named_section(p, "risks")
        if span is None or index not in p[span[0] : span[1]]:
            problems.append(f"{plan.name}: the Risks section lacks its index line")
    return problems


def read_text(path: Path) -> str:
    return path.read_bytes().decode("utf-8")


def existing_records(plan: Path) -> dict[str, str]:
    folder = pt.records_dir(plan)
    if not folder.is_dir():
        return {}
    return {f.stem: read_text(f) for f in sorted(folder.glob("*.md"))}


def write_all(files: dict[Path, str]) -> None:
    """Write each file through a temporary name beside it, then move them all into place."""
    temps = []
    for path, text in sorted(files.items()):
        path.parent.mkdir(parents=True, exist_ok=True)
        temp = path.with_name(f".{path.name}.plan-split-tmp")
        temp.write_bytes(text.encode("utf-8"))
        temps.append((temp, path))
    for temp, path in temps:
        os.replace(temp, path)


def verify_files(result: Result, plan_path: Path, folder: Path) -> list[str]:
    """Read back the written plan and record files and recombine them."""
    text = read_text(plan_path)
    on_disk = {key: read_text(folder / f"{key}.md") for key in result.records}
    problems = []
    try:
        if recombine(result, text, on_disk) != result.original:
            problems.append("the files written do not recombine into the original plan")
    except SplitError as e:
        problems.append(str(e))
    return problems


def verify_in_memory(result: Result) -> list[str]:
    try:
        if recombine(result, result.text, result.records) != result.original:
            return ["the split does not recombine into the original plan"]
    except SplitError as e:
        return [str(e)]
    return []


def outside_risks_unchanged(result: Result) -> list[str]:
    """Nothing outside Risks moves: the new plan without its inserted lines, up to its Risks
    section, is the original's up to its own."""
    lines = split_lines(result.text)
    for at, new in reversed(result.inserted):
        del lines[at : at + len(new)]
    p_new = [plain(x) for x in lines]
    orig = split_lines(result.original)
    p_old = [plain(x) for x in orig]
    a, b = pt.named_section(p_new, "risks"), pt.named_section(p_old, "risks")
    if a is None or b is None or lines[: a[0]] != orig[: b[0]] or lines[a[1] :] != orig[b[1] :]:
        return ["text outside Risks changed"]
    return []


def report(result: Result, problems: list[str]) -> dict:
    lines_before = len(split_lines(result.original))
    lines_after = len(split_lines(result.text))
    new_files = sorted(k for k in result.records if k not in result.existing)
    appended = sorted(k for k in result.records if k in result.existing)
    kept = []
    for it, first in zip(result.items, result.first_lines):
        if it.key is None:
            kept.append({"line": it.start + 1, "lines": it.end - it.start, "first": first, "why": it.why})
    moved = [
        {"line": it.start + 1, "lines": it.end - it.start, "key": it.key, "first": first, "why": it.why}
        for it, first in zip(result.items, result.first_lines)
        if it.key is not None
    ]
    return {
        "plan": f"{result.plan.parent.name}/{result.plan.name}",
        "lines_before": lines_before,
        "lines_after": lines_after,
        "record_lines": sum(len(split_lines(t)) for t in result.records.values()),
        "record_files_new": new_files,
        "record_files_appended": appended,
        "bullets_moved": len(moved),
        "bullets_kept": len(kept),
        "marks_added": [f"{result.first_lines[k][:60]}: {mark}" for k, mark in sorted(result.marked.items())],
        "links": result.links,
        "kept": kept,
        "moved": moved,
        "check": "pass" if not problems else "FAIL",
        "problems": problems,
        "warnings": html_hazards(result.plan.name, [plain(x) for x in split_lines(result.original)]),
    }


def print_report(rep: dict, verbose: bool) -> None:
    print(f"{rep['plan']}: {rep['lines_before']} lines -> {rep['lines_after']}; "
          f"{len(rep['record_files_new'])} record files new, {len(rep['record_files_appended'])} appended "
          f"({rep['record_lines']} lines); {rep['bullets_moved']} bullets moved, {rep['bullets_kept']} kept"
          + (f" ({len(rep['marks_added'])} marks added)" if rep["marks_added"] else "")
          + f"; check {rep['check']}")
    for k in rep["kept"]:
        print(f"  kept {k['line']} ({k['lines']} lines, {k['why']}): {k['first'][:100]}")
    if verbose:
        for m in rep["moved"]:
            print(f"  moved {m['line']} -> {m['key']} ({m['lines']} lines, {m['why']}): {m['first'][:80]}")
    for problem in rep["problems"]:
        print(f"  problem: {problem}")
    for warning in rep["warnings"]:
        print(f"  warning: {warning}")


def git(root: Path, *args: str) -> str:
    out = subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=False)
    if out.returncode != 0:
        raise SplitError(f"git {' '.join(args)}: {out.stderr.decode('utf-8', 'replace').strip()[:300]}")
    return out.stdout.decode("utf-8")


def at_revision(root: Path, plan: Path, rev: str) -> tuple[str, dict[str, str]]:
    rel = PurePosixPath(plan.resolve().relative_to(root.resolve()).as_posix())
    text = git(root, "show", f"{rev}:{rel}")
    folder = rel.parent / rel.stem / pt.RECORDS_DIR
    names = git(root, "ls-tree", "--name-only", f"{rev}:{folder}").split() if git_has(root, rev, folder) else []
    return text, {PurePosixPath(n).stem: git(root, "show", f"{rev}:{folder / n}") for n in names if n.endswith(".md")}


def git_has(root: Path, rev: str, path: PurePosixPath) -> bool:
    return subprocess.run(["git", "-C", str(root), "cat-file", "-e", f"{rev}:{path}"], capture_output=True).returncode == 0


def records_dirty(root: Path, plan: Path) -> bool:
    out = subprocess.run(["git", "-C", str(root), "status", "--porcelain", "--", str(pt.records_dir(plan))],
                         capture_output=True, check=False)
    return out.returncode == 0 and bool(out.stdout.strip())


def recorded_overrides(rev_text: str, text: str, records: dict[str, str]) -> tuple[list[tuple[str, str]], list[str]]:
    """The overrides a committed split records in its marks, for `--verify` to split REV's plan as
    it was split: `--assign` for each record file's bullet (`records`, the working tree's) with a
    file mark, `--keep` for each bullet of the plan's Risks (`text`) with a keep mark, each by its
    first line, where REV's Risks has a bullet of that first line (one filed before REV has not)."""
    def bullets(q: list[str], start: int, stop: int) -> list[pt.RiskItem]:
        return [it for it in pt.top_items(q, start, stop) if it.kind == "bullet"]

    rp = [plain(x) for x in split_lines(rev_text)]
    span = pt.named_section(rp, "risks")
    at_rev = {first_line(rp[it.start]) for it in bullets(rp, span[0] + 1, span[1])} if span else set()
    assign: list[tuple[str, str]] = []
    keep: list[str] = []
    p = [plain(x) for x in split_lines(text)]
    span = pt.named_section(p, "risks")
    for it in bullets(p, span[0] + 1, span[1]) if span else []:
        if pt.split_mark(p, it) == pt.KEEP_MARK and first_line(p[it.start]) in at_rev:
            keep.append(first_line(p[it.start]))
    for body in records.values():
        q = [plain(x) for x in split_lines(body)]
        for it in bullets(q, 0, len(q)):
            filed = pt.FILE_MARK_RE.fullmatch(pt.split_mark(q, it) or "")
            if filed and first_line(q[it.start]) in at_rev:
                assign.append((first_line(q[it.start]), filed.group(1)))
    return assign, keep


def parse_assign(values: list[str]) -> list[tuple[str, str]]:
    pairs = []
    for value in values:
        text, sep, task = value.rpartition("=")
        if not sep or not text or not pt.ID_RE.fullmatch(pt.normalise_id(task)):
            raise UsageError(f"--assign {value!r}: give TEXT=ID, such as 'Ruled by \"main\"=P14.T51.a'")
        pairs.append((text, task))
    return pairs


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("plan", type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--dry-run", type=Path, metavar="DIR")
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--verify", metavar="REV")
    parser.add_argument("--assign", action="append", default=[], metavar="TEXT=ID")
    parser.add_argument("--keep", action="append", default=[], metavar="TEXT")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--verbose", action="store_true", help="also list each moved bullet")
    args = parser.parse_args(argv)
    plan: Path = args.plan
    if not plan.is_file():
        print(f"no plan at {plan}", file=sys.stderr)
        return EXIT_USAGE
    try:
        assign, keep = parse_assign(args.assign), list(args.keep)
        if args.verify:
            root = pt.repo_root()
            original, existing = at_revision(root, plan, args.verify)
            # The overrides the committed split made, from its marks, beside any given.
            marked_assign, marked_keep = recorded_overrides(original, read_text(plan), existing_records(plan))
            assign += [a for a in marked_assign if a not in assign]
            keep += [k for k in marked_keep if k not in keep]
        else:
            original, existing = read_text(plan), existing_records(plan)
        result = split_text(plan, original, existing, assign, keep)
    except SplitError as e:
        print(f"plan_split: {e}", file=sys.stderr)
        return EXIT_USAGE if isinstance(e, UsageError) else EXIT_FAIL

    problems = verify_in_memory(result) + outside_risks_unchanged(result)
    all_records = {**result.existing, **result.records}
    problems += check_layout(plan, result.text, all_records)
    if args.dry_run and not problems:
        out_dir = args.dry_run / plan.parent.name
        folder = out_dir / plan.stem / pt.RECORDS_DIR
        write_all({out_dir / plan.name: result.text, **{folder / f"{k}.md": t for k, t in result.records.items()}})
        problems += verify_files(result, out_dir / plan.name, folder)
    elif args.write and not problems:
        root = pt.repo_root()
        if records_dirty(root, plan):
            problems.append(f"{pt.records_dir(plan)} has uncommitted changes: commit or restore them first")
        elif result.records or result.text != result.original:
            folder = pt.records_dir(plan)
            write_all({plan: result.text, **{folder / f"{k}.md": t for k, t in result.records.items()}})
            problems += verify_files(result, plan, folder)
    elif args.verify and not problems:
        if read_text(plan) != result.text:
            problems.append(f"{plan.name} differs from the split of {args.verify}'s")
        on_disk = existing_records(plan)
        for key, text in sorted(all_records.items()):
            if on_disk.get(key) != text:
                problems.append(f"{pt.record_rel(plan, key)} differs from the split of {args.verify}'s")
        for key in sorted(set(on_disk) - set(all_records)):
            problems.append(f"{pt.record_rel(plan, key)} is not in the split of {args.verify}'s")
    rep = report(result, problems)
    if args.dry_run:
        out_dir = args.dry_run / plan.parent.name
        out_dir.mkdir(parents=True, exist_ok=True)
        (out_dir / f"{plan.stem}.split.json").write_bytes((json.dumps(rep, indent=1, sort_keys=True, ensure_ascii=False) + "\n").encode("utf-8"))
    if args.json:
        print(json.dumps(rep, indent=1, sort_keys=True, ensure_ascii=False))
    else:
        print_report(rep, args.verbose)
    return EXIT_FAIL if problems else EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
