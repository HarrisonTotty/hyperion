#!/usr/bin/env python3
"""Tests for plan_task.py's task markers: `python3 -B .claude/skills/implement-task/scripts/test_plan_task.py`.

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


if __name__ == "__main__":
    unittest.main()
