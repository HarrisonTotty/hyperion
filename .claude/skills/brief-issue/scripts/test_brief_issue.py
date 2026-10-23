#!/usr/bin/env python3
"""Tests for brief_issue.py: `python3 -I -B .claude/skills/brief-issue/scripts/test_brief_issue.py`.

Standard library only. `gh` is a fake (`.claude/gh-collab/fakes.py`), so nothing is posted; git runs
on throwaway repositories under $GH_COLLAB_TEST_TMP or this clone's git directory.
"""

from __future__ import annotations

import contextlib
import io
import sys
import unittest
from pathlib import Path
from typing import Tuple

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[2] / "gh-collab"))

import brief_issue as bi  # noqa: E402
import fakes  # noqa: E402
import ghcollab as gc  # noqa: E402


def run(ctx: gc.Ctx, *argv: str) -> Tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = bi.main(list(argv), ctx)
    return code, out.getvalue(), err.getvalue()


def fill_in(draft: str) -> str:
    """Replace each placeholder with text, as the orchestrator would."""
    out = []
    for line in draft.splitlines():
        if "[[FILL" in line:
            line = line.split("[[FILL")[0] + "Read `docs/plan.md` and draft the readouts in it."
        out.append(line)
    return "\n".join(out) + "\n"


class BriefTests(unittest.TestCase):
    def setUp(self) -> None:
        self.box = fakes.Sandbox().__enter__()
        self.remote, self.main = fakes.make_repos(self.box.dir)
        self.cfg = fakes.make_config(self.box.dir / "scratch", worker={"gate_command": "just gate {remote}/{base}"},
                                     brief={"rules": ["Follow the repository's rules."]})
        self.fake = fakes.FakeGitHub()
        self.ctx = fakes.make_ctx(self.fake, self.cfg, self.main)
        self.draft = self.box.dir / "draft.md"

    def tearDown(self) -> None:
        self.box.__exit__(None, None, None)

    def scaffold(self, *extra: str) -> str:
        code, out, err = run(self.ctx, "scaffold", "--title", "R10.T13 Guide drafts", "--slug", "r10-t13",
                             "--task", "R10.T13", *extra)
        self.assertEqual(code, 0, err)
        return out

    def check(self, *extra: str) -> Tuple[int, str, str]:
        return run(self.ctx, "check", str(self.draft), "--title", "R10.T13 Guide drafts", "--label", "plan:R10",
                   "--label", "area:sky", "--milestone", "RM5 Generated worlds (R09, R10)", *extra)

    def test_the_scaffold_fills_the_standard_sections(self) -> None:
        out = self.scaffold("--scope", "docs/*")
        head = fakes.git(self.main, "rev-parse", "--short=8", "origin/trunk")
        self.assertIn(f"Branch from `origin/trunk` at **{head}** or later, as `lane/<this issue's number>-r10-t13`", out)
        self.assertIn("Run `just gate origin/trunk`", out)
        self.assertIn(fakes.TRAILER, out)
        self.assertIn("`<type>(<scope>): R10.T13 <summary>`", out)
        self.assertIn("- Follow the repository's rules.", out)
        self.assertIn("`report` step, which moves the issue from `agent-claimed` to `agent-done`", out)
        meta = [d for k, d in gc.find_markers(out) if k == "brief"][0]
        self.assertEqual(meta, {"slug": "r10-t13", "base": "trunk", "base_commit": head, "task": "R10.T13",
                                "scope": ["docs/*"]})

    def test_a_scope_naming_the_plan_gains_the_tasks_record_files(self) -> None:
        plan = "docs/agent/plans/rendering-and-planets/10-terrain-on-generated-worlds.md"
        records = "docs/agent/plans/rendering-and-planets/10-terrain-on-generated-worlds/records"
        out = self.scaffold("--scope", "crates/x/*", "--scope", plan)
        meta = [d for k, d in gc.find_markers(out) if k == "brief"][0]
        # R10.T13 has subtasks, whose files sit beside its own.
        self.assertEqual(meta["scope"], ["crates/x/*", plan, f"{records}/R10.T13.md", f"{records}/R10.T13.*.md"])
        self.assertTrue(gc.in_scope(f"{records}/R10.T13.b.md", meta["scope"]))
        self.assertEqual(bi.record_scope("R10.T13.b", [plan]), [f"{records}/R10.T13.b.md"])
        self.assertEqual(bi.record_scope("R10.T13.b.i", [plan]), [f"{records}/R10.T13.b.md"])
        self.assertEqual(bi.record_scope("P10.T13", [plan]), [])  # galaxy plan 10 is another file
        self.assertEqual(bi.record_scope("R10.T13", ["docs/agent/plans/**"]), [])  # covered already
        # A marker written by hand without it is warned about.
        text = gc.MARKER_RE.sub("", fill_in(self.scaffold())) + "\n" + gc.make_marker(
            "brief", {"slug": "r10-t13", "base": "trunk", "base_commit": meta["base_commit"], "task": "R10.T13",
                      "scope": [plan]})
        self.draft.write_text(text, encoding="utf-8")
        code, out, _ = self.check()
        self.assertIn("the scope names the task's plan but not its record file", out)

    def test_the_scaffold_refuses_a_bad_slug_and_an_unpushed_base(self) -> None:
        code, _, err = run(self.ctx, "scaffold", "--title", "x", "--slug", "../trunk")
        self.assertEqual(code, gc.EXIT_USAGE)
        local = fakes.commit(self.main, {"x.txt": "x\n"}, "docs: local")
        code, _, err = run(self.ctx, "scaffold", "--title", "x", "--slug", "x", "--base-commit", local[:8])
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("push it before the brief names it", err)

    def test_check_refuses_an_unfinished_brief_and_passes_a_finished_one(self) -> None:
        self.draft.write_text(self.scaffold(), encoding="utf-8")
        code, out, _ = self.check()
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("unfilled [[FILL]] placeholder", out)
        self.draft.write_text(fill_in(self.scaffold()), encoding="utf-8")
        code, out, _ = self.check()
        self.assertEqual(code, 0, out)
        self.assertEqual(self.fake.writes(), [])

    def test_a_hand_written_draft_takes_its_marker_from_the_flags(self) -> None:
        text = fill_in(self.scaffold())
        text = gc.MARKER_RE.sub("", text)
        self.draft.write_text(text, encoding="utf-8")
        self.assertIn("holds 0 brief markers", self.check()[1])
        code, out, _ = self.check("--slug", "r10-t13", "--task", "R10.T13")
        self.assertEqual(code, 0, out)
        code, out, _ = run(self.ctx, "post", str(self.draft), "--title", "R10.T13 Guide drafts", "--label", "plan:R10",
                           "--label", "area:sky", "--milestone", "RM5 Generated worlds (R09, R10)", "--slug", "r10-t13",
                           "--task", "R10.T13")
        self.assertEqual(code, 0, out)
        self.assertEqual(gc.brief_meta(gc.issue_from(self.fake.issues[1]), self.cfg)["task"], "R10.T13")
        self.draft.write_text(fill_in(self.scaffold()), encoding="utf-8")
        self.assertEqual(self.check("--slug", "x")[0], gc.EXIT_USAGE)  # it already has a marker

    def test_check_names_each_problem(self) -> None:
        text = fill_in(self.scaffold()).replace("### Gate", "### Checks").replace(fakes.TRAILER, "trailer")
        text += "\nLogs are in /home/alice/logs and `docs/missing.md`; mail alice@example.org.\n"
        self.draft.write_text(text, encoding="utf-8")
        self.fake.add_issue(3, title="R10.T13 An earlier brief")
        code, out, _ = run(self.ctx, "check", str(self.draft), "--title", "Guide drafts", "--label", "agent-done",
                           "--label", "nope", "--milestone", "RM9")
        self.assertEqual(code, gc.EXIT_FAIL)
        for expected in ("no section heading names 'Gate'", "does not quote the commit trailer",
                         "local home path", "email address", "`docs/missing.md` does not exist",
                         "state label 'agent-done'", "no label 'nope'", "no open milestone titled 'RM9'",
                         "#3 already briefs R10.T13", "the title does not name the task R10.T13"):
            self.assertIn(expected, out)

    def test_post_dry_run_posts_nothing_and_post_opens_the_issue(self) -> None:
        self.draft.write_text(fill_in(self.scaffold()), encoding="utf-8")
        args = ["post", str(self.draft), "--title", "R10.T13 Guide drafts", "--label", "plan:R10", "--label",
                "area:sky", "--milestone", "RM5 Generated worlds (R09, R10)"]
        code, out, err = run(self.ctx, *args, "--dry-run")
        self.assertEqual(code, 0, err)
        self.assertIn("DRY RUN: nothing posted.", out)
        self.assertIn("--label agent-task,plan:R10,area:sky", out)
        self.assertIn("then board: add as Todo", out)
        self.assertEqual(self.fake.writes(), [])
        self.assertEqual(self.fake.issues, {})
        code, out, err = run(self.ctx, *args)
        self.assertEqual(code, 0, err)
        issue = self.fake.issues[1]
        self.assertEqual(self.fake.label_names(1), ["agent-task", "plan:R10", "area:sky"])
        self.assertEqual(issue["milestone"], {"title": "RM5 Generated worlds (R09, R10)"})
        self.assertEqual(gc.brief_meta(gc.issue_from(issue), self.cfg)["slug"], "r10-t13")
        self.assertTrue(issue["body"].startswith("## Task: R10.T13 Guide drafts\n"))  # a body carries no tag
        self.assertEqual(self.fake.status_of(1), "Todo")
        self.assertIn("posted: https://github.com/owner/repo/issues/1", out)

    def test_labels_suggests_the_plan_label_and_milestone(self) -> None:
        code, out, _ = run(self.ctx, "labels", "--task", "R10.T13")
        self.assertEqual(code, 0)
        self.assertIn("suggested for R10.T13: label plan:R10; milestone 'RM5 Generated worlds (R09, R10)'", out)
        self.assertIn("area:*: area:sky", out)

    def test_answer_requeues_a_blocked_brief(self) -> None:
        self.fake.add_issue(4, labels=["agent-task", "agent-blocked"],
                            body=gc.make_marker("brief", {"slug": "x"}))
        self.fake.add_comment(4, gc.make_marker("claim", {"worker": "w-a", "nonce": "1"}))
        answer = self.box.dir / "answer.md"
        answer.write_text("Ruling: use the first table.\n", encoding="utf-8")
        code, out, _ = run(self.ctx, "answer", "4", "--body-file", str(answer), "--requeue", "--dry-run")
        self.assertEqual(code, 0)
        self.assertIn("\n[desk] Ruling: use the first table.\n", out)
        self.assertEqual(self.fake.writes(), [])
        code, _, err = run(self.ctx, "answer", "4", "--body-file", str(answer), "--requeue")
        self.assertEqual(code, 0, err)
        issue = gc.fetch_issue(self.ctx.gh, 4)
        self.assertEqual(issue.labels, ["agent-task"])
        self.assertIsNone(gc.winning_claim(issue, self.cfg))
        self.assertEqual(self.fake.status_of(4), "Todo")
        # The worker reads only comments that begin with the orchestrator's tag; the marker stays.
        posted = issue.comments[-1].body
        self.assertTrue(posted.startswith("[desk] Ruling: use the first table."), posted)
        self.assertEqual(issue.comments[-1].markers(), [("answer", {"requeue": True})])
        answer.write_text("## Ruling\n\nUse the first table.\n", encoding="utf-8")
        self.assertEqual(run(self.ctx, "answer", "4", "--body-file", str(answer))[0], 0)
        posted = gc.fetch_issue(self.ctx.gh, 4).comments[-1].body
        self.assertTrue(posted.startswith("[desk]\n\n## Ruling\n"), posted)  # the heading still renders
        self.assertEqual(gc.find_markers(posted), [("answer", {"requeue": False})])
        self.fake.add_issue(5, author="mallory")
        self.assertEqual(run(self.ctx, "answer", "5", "--body-file", str(answer))[0], gc.EXIT_UNTRUSTED)
        answer.write_text("See .git/rm45-orchestration/decision.md for the ruling.\n", encoding="utf-8")
        code, out, _ = run(self.ctx, "answer", "4", "--body-file", str(answer))
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("inside a .git directory", out)

    def test_setup_labels_creates_only_the_missing(self) -> None:
        self.fake.labels.remove("agent-blocked")
        self.assertIn("would create label 'agent-blocked'", run(self.ctx, "setup-labels", "--dry-run")[1])
        self.assertEqual(self.fake.writes(), [])
        run(self.ctx, "setup-labels")
        self.assertIn("agent-blocked", self.fake.labels)
        self.assertIn("all four present", run(self.ctx, "setup-labels")[1])

    def test_board_setup_and_sync(self) -> None:
        self.fake.scopes = "gist, read:org, repo"
        code, out, _ = run(self.ctx, "board-setup")
        self.assertEqual(code, 0)
        self.assertIn("gh auth refresh -s project", out)
        self.fake.scopes = "gist, project, read:org, repo"
        del self.fake.options["Blocked"]
        out = run(self.ctx, "board-setup", "--dry-run")[1]
        self.assertIn("project #2", out)
        self.assertIn("status options missing: Blocked", out)
        self.assertIn("Group by Milestone", out)
        self.fake.options["Blocked"] = "o4"
        self.fake.add_issue(1, labels=["agent-task", "agent-claimed"])
        self.fake.add_issue(2, labels=["agent-task", "agent-blocked"])
        self.fake.add_issue(3, author="mallory")
        out = run(self.ctx, "board-sync", "--dry-run")[1]
        self.assertIn("#1 (not on the board) -> Claimed", out)
        self.assertNotIn("#3", out)
        self.assertEqual(self.fake.writes(), [])
        run(self.ctx, "board-sync")
        self.assertEqual((self.fake.status_of(1), self.fake.status_of(2)), ("Claimed", "Blocked"))
        self.assertIn("0 card(s) moved", run(self.ctx, "board-sync")[1])


if __name__ == "__main__":
    unittest.main()
