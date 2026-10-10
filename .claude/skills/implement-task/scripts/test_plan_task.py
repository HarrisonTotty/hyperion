#!/usr/bin/env python3
"""Tests for plan_task.py's task markers and acceptance lists:
`python3 -B .claude/skills/implement-task/scripts/test_plan_task.py`.

Standard library only. The fixture tests build a small plan in a temporary directory; the last
class reads the repository's own plans under docs/agent/plans/.
"""

from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import plan_task as pt  # noqa: E402  (the script's directory is put on the path first)

PLAN = """\
# R06 The sky

## Tasks

### R06.T8 The census

- **R06.T8.a Query and plan.** `SkyQuery` and `census_plan`. Acceptance:
  `cargo test -p hyperion-sim sky::census::query`.

  Files: `sky/census/query.rs`.

- **R06.T8.g Census cost: a bound star by star (new; after T8.f, T8.m, P11.T16 and
  P11.T17.c).** Its landing leaves the server's sky off by default.

  Files: `sky/census/cell.rs`.

- **R06.T8.h The visibility caps' safety with the illumination (new; test only; after T11.d;
  before the default switch serves eye-only skies by visibility; decided 2026-10-08,
  `decision-r06-t11d-first-sky.md` §4).** T7.b's safety test.

  Files: `sky/census/cache.rs`.

Files: `sky/census/{mod,query,cell,cache}.rs`. Bench: `sky/census_near_sun`.

### R06.T9 The band

**R06.T9.a One line.** The band.

- **R07.T16.i's follow-up: labels inside the picture.** A note that names its task.

- **R06.T9.b A title that never closes
  on the next line

  nor after a blank line.**

- **R06.T9.c A title cut by a bullet
  - **nested.** Body.

- **R06.T9.d A title past the wrap limit
  one
  two
  three
  four.** Body.

## Risks and open points

- **R06.T8.g's slow test, as built (lane `sky`, 2026-10-09; no output
  moves).** Body.
- **R06.T8.h, as built (2026-10-09; for
  "main").** Body.
  - **R06.T8.g's nested record** (indented, so never a marker).
"""

WRAPPED_G = "Census cost: a bound star by star (new; after T8.f, T8.m, P11.T16 and P11.T17.c)"
WRAPPED_H = (
    "The visibility caps' safety with the illumination (new; test only; after T11.d; before the "
    "default switch serves eye-only skies by visibility; decided 2026-10-08, "
    "`decision-r06-t11d-first-sky.md` §4)"
)


def fixture_markers() -> tuple[list[str], list[pt.Marker]]:
    lines = PLAN.splitlines()
    return lines, pt.markers(lines)


class Markers(unittest.TestCase):
    def test_single_line_markers_keep_their_titles(self) -> None:
        _, found = fixture_markers()
        titles = {(m.task_id, m.title, m.level) for m in found}
        self.assertIn(("R06.T8", "The census", 3), titles)
        self.assertIn(("R06.T8.a", "Query and plan", 7), titles)
        self.assertIn(("R06.T9.a", "One line", 7), titles)
        # A one-line `ID's …` bullet was a marker before wrapped titles were read, and stays one.
        self.assertIn(("R07.T16.i", "'s follow-up: labels inside the picture", 7), titles)

    def test_a_wrapped_title_is_found_and_joined(self) -> None:
        lines, found = fixture_markers()
        by_id = {m.task_id: m for m in found}
        self.assertEqual(by_id["R06.T8.g"].title, WRAPPED_G)
        self.assertTrue(lines[by_id["R06.T8.g"].line].startswith("- **R06.T8.g Census cost"))
        self.assertEqual(by_id["R06.T8.g"].level, 7)

    def test_a_title_wrapped_over_three_lines_is_found(self) -> None:
        _, found = fixture_markers()
        self.assertEqual({m.task_id: m.title for m in found}["R06.T8.h"], WRAPPED_H)

    def test_only_titles_that_close_within_their_paragraph_count(self) -> None:
        _, found = fixture_markers()
        ids = {m.task_id for m in found}
        self.assertNotIn("R06.T9.b", ids)  # a blank line comes before the bold closes
        self.assertNotIn("R06.T9.c", ids)  # a bullet comes before the bold closes
        self.assertNotIn("R06.T9.d", ids)  # it closes past WRAP_LINES

    def test_wrapped_records_and_nested_bullets_are_not_tasks(self) -> None:
        _, found = fixture_markers()
        self.assertEqual(
            [m.task_id for m in found],
            ["R06.T8", "R06.T8.a", "R06.T8.g", "R06.T8.h", "R06.T9", "R06.T9.a", "R07.T16.i"],
        )

    def test_a_task_before_a_wrapped_one_ends_at_it(self) -> None:
        lines, found = fixture_markers()
        by_id = {m.task_id: m for m in found}
        self.assertEqual(pt.section_end(lines, by_id["R06.T8.a"], found), by_id["R06.T8.g"].line)

    def test_the_parent_keeps_its_paragraph_at_the_margin(self) -> None:
        lines, found = fixture_markers()
        parent = next(m for m in found if m.task_id == "R06.T8")
        _, children, own, shared = pt.subtasks(lines, found, parent)
        self.assertEqual([c.task_id for c in children], ["R06.T8.a", "R06.T8.g", "R06.T8.h"])
        margin = "Files: `sky/census/{mod,query,cell,cache}.rs`. Bench: `sky/census_near_sun`."
        self.assertEqual(["\n".join(lines[a:b]) for a, b in shared], [margin])
        self.assertEqual(lines[own["R06.T8.h"][-1][0]], "  Files: `sky/census/cache.rs`.")

    def test_an_indented_label_no_earlier_subtask_uses_is_still_shared(self) -> None:
        # P14.T6's form: the last subtask's indented `- _Accept:_` binds every subtask.
        lines = [
            "### P14.T6 Spacing",
            "",
            "- **P14.T6.a The floor.** Body.",
            "- **P14.T6.b The redraw.** Body.",
            "  - _Accept:_ `cargo test -p hyperion-sim planetary::placement::spacing`.",
        ]
        found = pt.markers(lines)
        _, _, _, shared = pt.subtasks(lines, found, found[0])
        self.assertEqual(shared, [(4, 5)])


class Listing(unittest.TestCase):
    def test_list_marks_a_wrapped_task_with_a_commit(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            plans = root / "docs" / "agent" / "plans" / "rendering-and-planets"
            plans.mkdir(parents=True)
            (plans / "06-the-sky.md").write_text(PLAN, encoding="utf-8")
            log = "feat(server): R06.T8.g Census cost\nfeat(sim): R06.T8.a Query and plan\n"
            out = io.StringIO()
            with mock.patch.object(pt, "commit_subjects", return_value=log), contextlib.redirect_stdout(out):
                pt.show_list(root, "R06", None)
            listing = out.getvalue().splitlines()
        self.assertIn(f"  R06.T8.g  {WRAPPED_G}  [commit]", listing)
        self.assertIn(f"  R06.T8.h  {WRAPPED_H}", listing)  # no commit names it: unbuilt
        self.assertEqual(sum(line.split()[:1] == ["R06.T8.g"] for line in listing), 1)

    def test_load_finds_a_wrapped_task(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            plans = root / "docs" / "agent" / "plans" / "rendering-and-planets"
            plans.mkdir(parents=True)
            (plans / "06-the-sky.md").write_text(PLAN, encoding="utf-8")
            _, _, _, target = pt.load(root, "R06.T8.h", None)
        self.assertEqual(target.title, WRAPPED_H)


# Each form of acceptance list the plans use: R08.T1's margin lead-in with a blank line before its
# list, R06.T8.g's indented one with a command wrapped back to the margin, R05.T12.e's nested
# `- Acceptance:` bullet, P14.T14's `- _Accept:_` and P11.T17.a's `- **Acceptance.**`.
ACCEPT_PLAN = """\
# R08 Atmospheres

## Tasks

### R08.T1 The asks

Files: `plan 14`. Acceptance:

- `npx prettier --check plan.md` passes;
- the tasks appear, with their tests and
  sources;

- a loose item after a blank line, `just loose`.

The closing paragraph, `just after-the-list`.

### R08.T2 The census

- **R08.T2.a A bound.** Body.

  Files: `cell.rs`. Acceptance:
  - `cargo test -p hyperion-sim --lib -- sky::census`;
  - `just test-slow first_test
second_test`;
  - `just ci`.

  No generator bump, `just after-the-list`.

- **R08.T2.b Nested.** Body.
  - **Acceptance:**
    - `pnpm test`;
    - by hand, recorded:
      - the timing.
  - Suggested subject: `just sibling`.

- **R08.T2.c Italic.** Body.
  - _Accept:_
    - `just gpu-replay-check`.
- **R08.T2.d Full stop.** Body.
  - **Acceptance.**
    - `just fit-check`.

### R08.T3 Inline

Acceptance: `just inline`.
- `just apart`.

Acceptance:

### R08.T4 A heading ends the list
"""


def accept_fixture() -> tuple[list[str], list[pt.Marker]]:
    lines = ACCEPT_PLAN.splitlines()
    return lines, pt.markers(lines)


def criteria_of(task_id: str) -> list[str]:
    """The acceptance paragraphs `--acceptance` prints for a task of ACCEPT_PLAN, in order."""
    lines, found = accept_fixture()
    target = next(m for m in found if m.task_id == task_id)
    groups = pt.acceptance_groups(lines, found, target)
    return [p for _, paras in groups for p in paras if pt.ACCEPT_RE.search(p)]


def acceptance_output(task_id: str) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        plans = root / "docs" / "agent" / "plans" / "rendering-and-planets"
        plans.mkdir(parents=True)
        (plans / "08-atmospheres.md").write_text(ACCEPT_PLAN, encoding="utf-8")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            pt.show_acceptance(root, task_id, None)
    return out.getvalue()


def quoted_commands(output: str) -> list[str]:
    section = output.split("## Commands quoted in the acceptance criteria\n", 1)
    return [line[3:].split("`")[0] for line in section[1].splitlines() if line.startswith("- `")] if len(section) > 1 else []


class AcceptanceLists(unittest.TestCase):
    def test_the_lead_in_forms(self) -> None:
        for line in (
            "Files: `cell.rs`. Acceptance:",
            "  Acceptance:",
            "- Acceptance:",
            "  - **Acceptance:**",
            "  - **Acceptance.**",
            "- _Accept:_",
            "  - **Acceptance as built:**",
            "  - **Acceptance as run.**",
            "Acceptance for T5.b and T5.c:",
        ):
            self.assertIsNotNone(pt.LEAD_IN_RE.search(line), line)
        for line in (
            "Acceptance: `just ci`.",
            "- **Acceptance:** `pnpm test`.",
            "the owner's Acceptance.",
            "and this prints nothing:",
            "Unacceptable:",
        ):
            self.assertIsNone(pt.LEAD_IN_RE.search(line), line)

    def test_a_margin_lead_in_keeps_its_list_across_blank_lines(self) -> None:
        self.assertEqual(
            criteria_of("R08.T1"),
            [
                "Files: `plan 14`. Acceptance:\n\n- `npx prettier --check plan.md` passes;\n"
                "- the tasks appear, with their tests and\n  sources;\n\n"
                "- a loose item after a blank line, `just loose`."
            ],
        )

    def test_an_indented_lead_in_keeps_its_list_and_a_command_wrapped_to_the_margin(self) -> None:
        [lead_in] = criteria_of("R08.T2.a")
        self.assertTrue(lead_in.startswith("  Files: `cell.rs`. Acceptance:\n  - `cargo test"))
        self.assertTrue(lead_in.endswith("  - `just ci`."))
        self.assertEqual(
            quoted_commands(acceptance_output("R08.T2.a")),
            ["cargo test -p hyperion-sim --lib -- sky::census", "just test-slow first_test second_test", "just ci"],
        )

    def test_a_bullet_lead_in_keeps_only_the_bullets_nested_under_it(self) -> None:
        [lead_in] = criteria_of("R08.T2.b")
        self.assertEqual(
            lead_in,
            "  - **Acceptance:**\n    - `pnpm test`;\n    - by hand, recorded:\n      - the timing.",
        )
        output = acceptance_output("R08.T2.b")
        self.assertEqual(quoted_commands(output), ["pnpm test"])
        self.assertIn("## Contains manual steps", output)

    def test_the_italic_and_full_stop_markers(self) -> None:
        self.assertEqual(criteria_of("R08.T2.c"), ["  - _Accept:_\n    - `just gpu-replay-check`."])
        self.assertEqual(criteria_of("R08.T2.d"), ["  - **Acceptance.**\n    - `just fit-check`."])

    def test_the_parent_prints_each_subtasks_list_once(self) -> None:
        commands = quoted_commands(acceptance_output("R08.T2"))
        self.assertEqual(len(commands), len(set(commands)))
        self.assertIn("just test-slow first_test second_test", commands)
        self.assertIn("just fit-check", commands)
        self.assertNotIn("just sibling", commands)
        self.assertNotIn("just after-the-list", commands)

    def test_a_lead_in_with_its_criterion_inline_keeps_the_bullets_apart(self) -> None:
        self.assertEqual(criteria_of("R08.T3"), ["Acceptance: `just inline`.", "Acceptance:"])
        self.assertEqual(quoted_commands(acceptance_output("R08.T3")), ["just inline"])

    def test_the_list_ends_at_a_task_a_heading_a_paragraph_or_a_gap(self) -> None:
        lines = [
            "Acceptance:",  # 0
            "- `just one`;",  # 1
            "- **R08.T9.b Next.** Body.",  # 2: a task marker
            "",
            "Acceptance:",  # 4
            "",
            "### A heading",  # 6
            "",
            "Acceptance:",  # 8
            "",
            "Text.",  # 10
        ]
        spans = pt.paragraph_spans(lines, 0, len(lines))
        self.assertEqual(spans, [(0, 1), (1, 2), (2, 3), (4, 5), (6, 7), (8, 9), (10, 11)])
        self.assertEqual(pt.with_lists(lines, spans, {2}), [(0, 2), (2, 3), (4, 5), (6, 7), (8, 9), (10, 11)])
        # A span the caller left out (a paragraph shared by all subtasks) ends the list too.
        self.assertEqual(pt.with_lists(lines, [(0, 1), (2, 3)], set()), [(0, 1), (2, 3)])

    def test_a_list_at_the_lead_ins_indent_ends_at_a_shallower_bullet(self) -> None:
        lines = ["  Acceptance:", "  - `just one`;", "    - nested;", "- `just out`."]
        spans = pt.paragraph_spans(lines, 0, len(lines))
        self.assertEqual(pt.with_lists(lines, spans, set()), [(0, 3), (3, 4)])


REPO = Path(__file__).resolve().parents[4]
PLANS = REPO / "docs" / "agent" / "plans"


@unittest.skipUnless(PLANS.is_dir(), "no docs/agent/plans in this checkout")
class RepositoryPlans(unittest.TestCase):
    def test_every_bold_task_title_at_the_margin_is_a_marker(self) -> None:
        missed = []
        for plan in sorted(PLANS.glob("*/[0-9][0-9]-*.md")):
            lines = plan.read_text(encoding="utf-8").splitlines()
            found = {m.line for m in pt.markers(lines)}
            missed += [
                f"{plan.relative_to(REPO)}:{i + 1}: {line[:90]}"
                for i, line in enumerate(lines)
                if pt.WRAPPED_OPEN_RE.match(line) and i not in found
            ]
        self.assertEqual(missed, [])

    def test_the_skys_wrapped_census_tasks_are_found(self) -> None:
        plan = next(PLANS.glob("rendering-and-planets/06-*.md"), None)
        if plan is None:
            self.skipTest("no plan R06")
        ids = {m.task_id for m in pt.markers(plan.read_text(encoding="utf-8").splitlines())}
        for task_id in ("R06.T8.g", "R06.T8.h", "R06.T8.i", "R06.T8.l", "R06.T8.m", "R06.T8.n"):
            self.assertIn(task_id, ids)

    def test_every_acceptance_lead_in_prints_its_first_bullet(self) -> None:
        """A lead-in in a Tasks section followed by a bullet (nested under it, for a bullet lead-in)
        is printed with that bullet for the task it belongs to (R06.T8.g, R08.T1, P11.T16, …)."""
        missed, checked = [], 0
        for plan in sorted(PLANS.glob("*/[0-9][0-9]-*.md")):
            lines = plan.read_text(encoding="utf-8").splitlines()
            found = pt.markers(lines)
            task_lines = {m.line for m in found}
            tasks = pt.named_section(lines, "tasks")
            for i in range(*tasks) if tasks else ():
                if not pt.LEAD_IN_RE.search(lines[i]):
                    continue
                nxt = next((j for j in range(i + 1, tasks[1]) if lines[j].strip()), None)
                floor = pt.indent(lines[i]) + (1 if pt.BULLET_RE.match(lines[i]) else 0)
                if nxt is None or nxt in task_lines or not pt.BULLET_RE.match(lines[nxt]) or pt.indent(lines[nxt]) < floor:
                    continue
                owner = [m for m in found if m.line < i]
                paras = [p for _, ps in pt.acceptance_groups(lines, found, owner[-1]) for p in ps] if owner else []
                checked += 1
                if not any(f"{lines[i]}\n" in p and lines[nxt] in p for p in paras):
                    missed.append(f"{plan.relative_to(REPO)}:{i + 1}: {lines[i].strip()[:80]}")
        self.assertEqual(missed, [])
        self.assertGreater(checked, 0)


if __name__ == "__main__":
    unittest.main()
