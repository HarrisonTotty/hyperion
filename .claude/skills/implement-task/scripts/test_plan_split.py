#!/usr/bin/env python3
"""Tests for plan_split.py, and for plan_task.py's reading of a split plan:
`python3 -B .claude/skills/implement-task/scripts/test_plan_split.py`.

Standard library only. Temporary files go under $TMPDIR (point it at a scratch directory on disk).
The fixture tests build a small plan; the last class splits every plan of the repository in
memory and checks that each recombines exactly (it writes nothing).
"""

from __future__ import annotations

import contextlib
import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import plan_split as ps  # noqa: E402  (the script's directory is put on the path first)
import plan_task as pt  # noqa: E402

PLAN = """\
# Plan 07: A fixture

## Goal

What exists when it is done.

## Tasks

### P07.T1 A heading task

Body of T1.

- **P07.T2.a A bullet task.** Its first paragraph
  goes on here.
  - A nested point.
- **P07.T2.b A tight bullet.** One line.

**P07.T3 A paragraph task.** Its body
goes on.

**P07.T3.a A paragraph subtask.** Body.

### P07.T4 A parent

- **P07.T4.a The first.** Body.
- **P07.T4.b The second.** Body.

## Risks and open points

- **An open point.** It names no task.
- **Deviations in T1, as built.** T1's record.
- **Deviations in T2.a and T3, as built.** Two tasks built together.
  - A nested detail.

  A second paragraph, so a blank line follows the bullet.

- **Deferred findings (2026-10-13 rule)**, from P07.T2.b: one finding.
- **Ruled by "main"** (decision-p07-t3a-thing, 2026-10-16), on the record above.
- **Re-validated at abc1234 for P07.T1.** A re-validation of the plan stays in Risks.
- **Ruling 9, as built.** T1, T2.a and T4.a follow it.
- **Deviations in T4.a–b, as built.** Both subtasks.
- **Deviations in P07.T9, as built.** No such task.
- **The `T1` constant** names no task: it is code.
- **P06.T3's seam, as built.** Another plan's task,
lazily continued at the margin.
- **P07.T4.c as built.** A subtask added as built, with no entry of its own.

### Asked by plan R01

- **Deviations in T1, under a sub-heading.** It stays.
"""

PLAN_PATH = Path("galaxy-generation") / "07-a-fixture.md"


def split_fixture(text: str = PLAN, existing: dict[str, str] | None = None, **kw) -> ps.Result:
    return ps.split_text(PLAN_PATH, text, existing or {}, **kw)


def by_first(result: ps.Result) -> dict[str, pt.RiskItem]:
    return {first[:30]: it for it, first in zip(result.items, result.first_lines)}


class Assignment(unittest.TestCase):
    def test_each_rule(self) -> None:
        items = by_first(split_fixture())
        expect = {
            "**An open point.** It names no": None,
            "**Deviations in T1, as built.*": "P07.T1",
            "**Deviations in T2.a and T3, a": "P07.T2.a",  # the title's first named
            "**Deferred findings (2026-10-1": "P07.T2.b",  # its first sentence names it
            '**Ruled by "main"** (decision-': "P07.T3.a",  # its decision file names it
            "**Re-validated at abc1234 for ": None,  # a re-validation of the plan
            "**Ruling 9, as built.** T1, T2": None,  # its first sentence names several tasks
            "**Deviations in T4.a–b, as bui": "P07.T4",  # two subtasks: the parent's file
            "**Deviations in P07.T9, as bui": None,  # no such task under Tasks
            "**P07.T4.c as built.** A subta": "P07.T4",  # no entry of its own: its parent's
            "**The `T1` constant** names no": None,  # a code span names no task
            "**P06.T3's seam, as built.** A": None,  # another plan's task
            "**Deviations in T1, under a su": None,  # under a sub-heading
        }
        self.assertEqual({k: items[k].key for k in expect}, expect)
        self.assertIn("several tasks", items["**Ruling 9, as built.** T1, T2"].why)
        self.assertIn("re-validation", items["**Re-validated at abc1234 for "].why)
        self.assertIn("no task under Tasks", items["**Deviations in P07.T9, as bui"].why)
        self.assertEqual(items["### Asked by plan R01"].kind, "heading")

    def test_a_lazy_margin_line_stays_in_its_bullet(self) -> None:
        result = split_fixture()
        lines = ps.split_lines(PLAN)
        item = by_first(result)["**P06.T3's seam, as built.** A"]
        self.assertEqual(ps.plain(lines[item.body_end - 1]), "lazily continued at the margin.")

    def test_named_tasks_and_ranges(self) -> None:
        self.assertEqual(pt.named_tasks("T11.a–c and T12", "P14"), ["P14.T11.a", "P14.T11.b", "P14.T11.c", "P14.T12"])
        self.assertEqual(pt.named_tasks("P14.T41–T43, T5.b–T5.c", "P14"),
                         ["P14.T41", "P14.T42", "P14.T43", "P14.T5.b", "P14.T5.c"])
        self.assertEqual(pt.named_tasks("T3.b's follow-up, after P06.T33 and R08.T5.k", "P14"), ["P14.T3.b"])
        self.assertEqual(pt.named_tasks("P14.T24.h.a's floor", "P14"), ["P14.T24.h.a"])
        self.assertEqual(pt.named_tasks("T5–P06.T7", "P14"), ["P14.T5"])  # a range ends in this plan
        self.assertEqual(pt.decision_tasks("(decision-r08-t4ef-storage, decision-r08-t12-x)", "R08"),
                         ["R08.T4.e", "R08.T4.f", "R08.T12"])

    def test_record_keys(self) -> None:
        self.assertEqual(pt.record_key("P14.T54.b.i"), "P14.T54.b")
        self.assertEqual(pt.record_key("P14.T53"), "P14.T53")
        ids = {"P11.T4", "P14.T54.b"}
        self.assertEqual(pt.record_file_key("P11.T4.h", ids), "P11.T4")
        self.assertEqual(pt.record_file_key("P14.T54.b.i", ids), "P14.T54.b")
        self.assertIsNone(pt.record_file_key("P14.T9", ids))
        self.assertEqual(pt.normalise_id("p14.t54.B.i"), "P14.T54.b.i")


class Split(unittest.TestCase):
    def test_it_recombines_exactly_and_changes_nothing_outside_risks(self) -> None:
        result = split_fixture()
        self.assertEqual(ps.recombine(result, result.text, result.records), PLAN)
        self.assertEqual(ps.outside_risks_unchanged(result), [])
        self.assertEqual(ps.check_layout(PLAN_PATH, result.text, result.records), [])
        self.assertEqual(sorted(result.records), ["P07.T1", "P07.T2.a", "P07.T2.b", "P07.T3.a", "P07.T4"])
        moved = "".join(result.records.values())
        for it in result.moved:  # every moved bullet is in exactly one file, and not in the plan
            text = "".join(ps.split_lines(PLAN)[it.start : it.body_end])
            self.assertEqual(moved.count(text), 1)
            self.assertNotIn(text, result.text)

    def test_the_link_lines_and_the_index(self) -> None:
        text = split_fixture().text
        rel = "07-a-fixture/records/"
        self.assertIn(f"### P07.T1 A heading task\n\nRecords: [{rel}P07.T1.md]({rel}P07.T1.md).\n\nBody of T1.\n", text)
        self.assertIn(f"  goes on here.\n  Records: [{rel}P07.T2.a.md]({rel}P07.T2.a.md).\n  - A nested point.\n", text)
        self.assertIn(f"One line.\n  Records: [{rel}P07.T2.b.md]({rel}P07.T2.b.md).\n\n**P07.T3 ", text)
        self.assertIn(f"subtask.** Body.\nRecords: [{rel}P07.T3.a.md]({rel}P07.T3.a.md).\n", text)
        self.assertIn(f"### P07.T4 A parent\n\nRecords: [{rel}P07.T4.md]({rel}P07.T4.md).\n\n- **P07.T4.a", text)
        index = pt.RECORDS_INDEX.format(stem="07-a-fixture", dir="records")
        self.assertIn(f"## Risks and open points\n\n{index}\n\n- **An open point.**", text)
        # The tasks keep their markers and titles.
        def tasks(t: str) -> list[tuple[str, str]]:
            lines = t.splitlines()
            return [(m.task_id, m.title) for m in pt.task_markers(lines, pt.markers(lines))]

        self.assertEqual(tasks(text), tasks(PLAN))

    def test_the_record_files_and_their_blank_lines(self) -> None:
        records = split_fixture().records
        header = pt.RECORD_HEADER.format(key="P07.T2.a", plan="07-a-fixture.md")
        self.assertEqual(records["P07.T2.a"], header + "- **Deviations in T2.a and T3, as built.** Two tasks built"
                         " together.\n  - A nested detail.\n\n  A second paragraph, so a blank line follows the bullet.\n")
        self.assertTrue(records["P07.T4"].endswith("Both subtasks.\n- **P07.T4.c as built.** A subtask added as built,"
                                                   " with no entry of its own.\n"))

    def test_a_list_still_ends_with_a_blank_line_before_a_heading(self) -> None:
        # P06.T3's bullet had none after it, being followed by P07.T4.c's, which moves.
        result = split_fixture()
        self.assertIn("lazily continued at the margin.\n\n### Asked by plan R01\n", result.text)
        self.assertEqual(ps.recombine(result, result.text, result.records), PLAN)

    def test_crlf_is_kept(self) -> None:
        crlf = PLAN.replace("\n", "\r\n")
        result = split_fixture(crlf)
        self.assertEqual(ps.recombine(result, result.text, result.records), crlf)
        self.assertNotIn("\n", result.text.replace("\r\n", ""))
        for body in result.records.values():
            self.assertNotIn("\n", body.replace("\r\n", ""))
        self.assertEqual(ps.check_layout(PLAN_PATH, result.text, result.records), [])

    def test_a_plan_with_nothing_to_move_is_unchanged(self) -> None:
        plan = PLAN.split("- **Deviations in T1, as built.**")[0] + "- **Another open point.** None.\n"
        result = split_fixture(plan)
        self.assertEqual(result.text, plan)
        self.assertEqual(result.records, {})

    def test_overrides(self) -> None:
        result = split_fixture(assign=[("**An open point.**", "P07.T4.b")], keep=["**Deviations in T1, as built.**"])
        items = by_first(result)
        self.assertEqual(items["**An open point.** It names no"].key, "P07.T4.b")
        self.assertIsNone(items["**Deviations in T1, as built.*"].key)
        self.assertEqual(ps.recombine(result, result.text, result.records), PLAN)
        with self.assertRaises(ps.UsageError):
            split_fixture(keep=["**Deviations in T"])  # several bullets
        with self.assertRaises(ps.UsageError):
            split_fixture(assign=[("**An open point.**", "P07.T8")])  # no such task


class AgainSplit(unittest.TestCase):
    """A split plan split again: the bullets added to its Risks since are filed after the records."""

    def test_a_new_bullet_is_appended_and_a_new_file_made(self) -> None:
        first = split_fixture()
        later = first.text.replace(
            "### Asked by plan R01",
            "- **Deviations in T1, follow-up, as built.** More.\n- **Deviations in T4.b, as built.** New.\n\n"
            "### Asked by plan R01")
        second = split_fixture(later, dict(first.records))
        self.assertEqual(sorted(second.records), ["P07.T1", "P07.T4.b"])
        self.assertEqual(second.links, {"P07.T1": "present", "P07.T4.b": "added"})
        self.assertTrue(second.records["P07.T1"].startswith(first.records["P07.T1"]))
        self.assertTrue(second.records["P07.T1"].endswith("T1's record.\n- **Deviations in T1, follow-up, as built.**"
                                                          " More.\n"))
        self.assertEqual(ps.recombine(second, second.text, second.records), later)
        self.assertEqual(ps.check_layout(PLAN_PATH, second.text, {**first.records, **second.records}), [])
        self.assertEqual(second.text.count(pt.RECORDS_INDEX.format(stem="07-a-fixture", dir="records")), 1)
        third = split_fixture(second.text, {**first.records, **second.records})
        self.assertEqual((third.text, third.records), (second.text, {}))

    def test_a_bullet_already_filed_is_refused(self) -> None:
        first = split_fixture()
        again = first.text.replace("### Asked by plan R01", "- **Deviations in T1, as built.** T1's record.\n\n"
                                   "### Asked by plan R01")
        with self.assertRaises(ps.SplitError):
            split_fixture(again, dict(first.records))

    def test_the_layout_check_finds_a_missing_link_and_a_stray_file(self) -> None:
        result = split_fixture()
        rel = "07-a-fixture/records/P07.T1.md"
        text = result.text.replace(f"Records: [{rel}]({rel}).\n\n", "")
        problems = ps.check_layout(PLAN_PATH, text, {**result.records, "P07.T8": "x\n"})
        self.assertTrue(any("P07.T1's entry holds its link line 0 times" in p for p in problems), problems)
        self.assertTrue(any("P07.T8 is not a task" in p for p in problems), problems)


def run_main(args: list[str]) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
        code = ps.main(args)
    return code, out.getvalue()


class Command(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="plan-split-test-"))
        self.root = self.tmp / "repo"
        self.plans = self.root / "docs" / "agent" / "plans" / "galaxy-generation"
        self.plans.mkdir(parents=True)
        self.plan = self.plans / "07-a-fixture.md"
        self.plan.write_bytes(PLAN.encode("utf-8"))

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp)

    def test_check_writes_nothing(self) -> None:
        code, out = run_main([str(self.plan), "--check"])
        self.assertEqual(code, ps.EXIT_OK, out)
        self.assertIn("check pass", out)
        self.assertEqual(self.plan.read_bytes().decode("utf-8"), PLAN)
        self.assertFalse(pt.records_dir(self.plan).exists())

    def test_dry_run_writes_the_outputs_and_a_report(self) -> None:
        out_dir = self.tmp / "out"
        code, out = run_main([str(self.plan), "--dry-run", str(out_dir), "--json"])
        self.assertEqual(code, ps.EXIT_OK, out)
        rep = json.loads(out)
        self.assertEqual((rep["bullets_moved"], rep["check"]), (6, "pass"))
        self.assertEqual(len(rep["kept"]), 8)  # the open point, five records of no one task, the heading and its bullet
        self.assertTrue((out_dir / "galaxy-generation" / "07-a-fixture" / "records" / "P07.T4.md").is_file())
        self.assertTrue((out_dir / "galaxy-generation" / "07-a-fixture.split.json").is_file())
        self.assertEqual(self.plan.read_bytes().decode("utf-8"), PLAN)

    @unittest.skipUnless(shutil.which("git"), "no git")
    def test_write_then_verify_against_the_revision_before(self) -> None:
        git = ["git", "-C", str(self.root), "-c", "user.name=plan-split-test", "-c", "user.email=test@example.invalid",
               "-c", "commit.gpgsign=false"]
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(git + ["add", "-A"], check=True)
        subprocess.run(git + ["commit", "-q", "-m", "inline", "--no-gpg-sign"], check=True)
        with mock.patch.object(pt, "repo_root", return_value=self.root):
            code, out = run_main([str(self.plan), "--write"])
            self.assertEqual(code, ps.EXIT_OK, out)
            self.assertTrue((pt.records_dir(self.plan) / "P07.T1.md").is_file())
            subprocess.run(git + ["add", "-A"], check=True)
            subprocess.run(git + ["commit", "-q", "-m", "split", "--no-gpg-sign"], check=True)
            code, out = run_main([str(self.plan), "--verify", "HEAD~1"])
            self.assertEqual(code, ps.EXIT_OK, out)
            record = pt.records_dir(self.plan) / "P07.T1.md"
            record.write_bytes(record.read_bytes() + b"- **An edit.**\n")
            code, out = run_main([str(self.plan), "--verify", "HEAD~1"])
            self.assertEqual(code, ps.EXIT_FAIL, out)
            self.assertIn("P07.T1.md differs", out)
            code, out = run_main([str(self.plan), "--write"])  # its records have uncommitted changes
            self.assertEqual(code, ps.EXIT_FAIL, out)

    def test_the_readers_follow_the_split(self) -> None:
        def run(fn, *args) -> str:
            out = io.StringIO()
            with contextlib.redirect_stdout(out), mock.patch.object(pt, "commit_subjects", return_value=""):
                fn(self.root, *args)
            return out.getvalue()

        inline = run(pt.show_list, "P07", None)
        self.assertIn("  P07.T2.a  A bullet task  [records]", inline)  # by assign_records()'s rule
        self.assertNotIn("P07.T4.c", inline)  # a Risks record's ID is not a task
        self.assertIn("(the Risks bullets filed under it)", run(pt.show_records, "P07.T1", None))

        code, _ = run_main([str(self.plan), "--dry-run", str(self.tmp / "out")])
        self.assertEqual(code, ps.EXIT_OK)
        shutil.rmtree(self.plans)
        shutil.copytree(self.tmp / "out" / "galaxy-generation", self.plans,
                        ignore=shutil.ignore_patterns("*.split.json"))

        listing = run(pt.show_list, "P07", None)
        self.assertIn("  P07.T2.a  A bullet task  [records]", listing)
        self.assertIn("P07.T4  A parent  [records]", listing)
        self.assertNotIn("P07.T4.c", listing)  # a Risks record's ID is not a task
        records = run(pt.show_records, "P07.T4.b", None)
        self.assertIn("07-a-fixture/records/P07.T4.b.md (not yet made)", records)
        self.assertIn("## P07.T4's records", records)  # the parent's bind the subtask
        self.assertIn("**Deviations in T4.a–b, as built.**", records)
        acceptance = run(pt.show_acceptance, "P07.T2.b", None)
        self.assertIn("records/P07.T2.b.md:6: **Deferred findings (2026-10-13 rule)**, from P07.T2.b", acceptance)
        self.assertNotIn("records/P07.T2.b.md:3:", acceptance)  # the header names the task, and is no record
        context = run(pt.show_task, "P07.T3.a", None, True)
        self.assertIn("## Records of P07.T3.a", context)
        self.assertIn('**Ruled by "main"**', context)


class Hazards(unittest.TestCase):
    def test_an_unclosed_style_block_is_named(self) -> None:
        p = ["Canvases are named `VIEW,", "<style drawn>, <slot name>`.", "", "- More."]
        self.assertEqual(len(ps.html_hazards("x.md", p)), 1)
        self.assertEqual(ps.html_hazards("x.md", p + ["</style>"]), [])


REPO = Path(__file__).resolve().parents[4]
PLANS = REPO / "docs" / "agent" / "plans"


@unittest.skipUnless(PLANS.is_dir(), "no docs/agent/plans in this checkout")
class RepositoryPlans(unittest.TestCase):
    def test_every_plan_splits_and_recombines_exactly(self) -> None:
        for plan in sorted(PLANS.glob("*/[0-9][0-9]-*.md")):
            with self.subTest(plan=plan.name):
                result = ps.split_text(plan, ps.read_text(plan), ps.existing_records(plan))
                self.assertEqual(ps.verify_in_memory(result), [])
                self.assertEqual(ps.outside_risks_unchanged(result), [])
                self.assertEqual(ps.check_layout(plan, result.text, {**result.existing, **result.records}), [])


if __name__ == "__main__":
    unittest.main()
