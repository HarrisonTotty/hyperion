#!/usr/bin/env python3
"""Tests for land_issue.py: `python3 -I -B .claude/skills/land-issue/scripts/test_land_issue.py`.

Standard library only. `gh` is a fake (`.claude/gh-collab/fakes.py`); git runs on throwaway
repositories (a bare remote, the integration checkout and a worker's clone) under
$GH_COLLAB_TEST_TMP or this clone's git directory.
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

import fakes  # noqa: E402
import ghcollab as gc  # noqa: E402
import land_issue as li  # noqa: E402

LANE = "lane/6-r10-t13"


def run(ctx: gc.Ctx, *argv: str) -> Tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = li.main(list(argv), ctx)
    return code, out.getvalue(), err.getvalue()


class LandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.box = fakes.Sandbox().__enter__()
        self.remote, self.main = fakes.make_repos(self.box.dir)
        gate = f'"{sys.executable}" -c "print(\'integration suites pass\')"'
        self.cfg = fakes.make_config(self.box.dir / "scratch", land={"gate_command": gate, "push_args": ["--no-verify"]})
        self.fake = fakes.FakeGitHub()
        body = "Brief.\n\n" + gc.make_marker("brief", {"slug": "r10-t13", "task": "R10.T13", "scope": ["docs/*"]})
        self.fake.add_issue(6, body=body, labels=["agent-task", "agent-done"])
        self.ctx = fakes.make_ctx(self.fake, self.cfg, self.main)
        # The worker's clone: a lane with two task commits and a merge of the moved trunk.
        self.worker = self.box.dir / "worker"
        fakes.git(self.box.dir, "clone", "-q", str(self.remote), str(self.worker))
        fakes.git(self.worker, "switch", "-q", "--no-track", "-c", LANE, "origin/trunk")
        self.first = fakes.commit(self.worker, {"docs/a.md": "a\n"}, fakes.msg("docs(plans): R10.T13 Draft"))
        fakes.commit(self.main, {"README.md": "hello again\n"}, fakes.msg("docs: trunk moves"))
        fakes.git(self.main, "push", "-q", "origin", "trunk")
        fakes.git(self.worker, "fetch", "-q", "origin")
        fakes.git(self.worker, "merge", "-q", "--no-edit", "origin/trunk")
        self.second = fakes.commit(self.worker, {"docs/b.md": "b\n"}, fakes.msg("docs(plans): R10.T13 Review fixes"))
        fakes.git(self.worker, "push", "-q", "origin", f"HEAD:refs/heads/{LANE}")

    def tearDown(self) -> None:
        self.box.__exit__(None, None, None)

    def test_check_lists_the_non_merge_commits_and_changes_nothing(self) -> None:
        before = fakes.git(self.main, "rev-parse", "HEAD")
        code, out, err = run(self.ctx, "check", "6")
        self.assertEqual(code, 0, out + err)
        self.assertIn("to pick (2):", out)
        self.assertIn(f"{self.first[:8]} docs(plans): R10.T13 Draft", out)
        self.assertIn(f"{self.second[:8]} docs(plans): R10.T13 Review fixes", out)
        self.assertIn("1 merge commit(s) on the lane, not picked", out)
        self.assertIn("scope: docs/*", out)
        self.assertIn("LAND CHECK: pass", out)
        self.assertEqual(fakes.git(self.main, "rev-parse", "HEAD"), before)
        self.assertEqual(self.fake.writes(), [])

    def test_check_fails_on_a_bad_commit_and_an_untrusted_issue(self) -> None:
        fakes.commit(self.worker, {"src/x.rs": "x\n"}, "wip")
        fakes.git(self.worker, "push", "-q", "origin", f"HEAD:refs/heads/{LANE}")
        code, out, _ = run(self.ctx, "check", "6")
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("not a conventional commit", out)
        self.assertIn("outside the brief's scope: src/x.rs", out)
        self.assertIn("LAND CHECK: FAIL", out)
        self.fake.issues[6]["author"] = {"login": "mallory"}
        self.assertEqual(run(self.ctx, "check", "6")[0], gc.EXIT_UNTRUSTED)

    def test_a_rewritten_lane_is_refused(self) -> None:
        self.assertEqual(run(self.ctx, "check", "6")[0], 0)  # records the lane's head
        fakes.git(self.worker, "reset", "-q", "--hard", "HEAD~1")
        fakes.commit(self.worker, {"docs/c.md": "c\n"}, fakes.msg("docs(plans): R10.T13 Other"))
        fakes.git(self.worker, "push", "-q", "--force", "origin", f"HEAD:refs/heads/{LANE}")
        code, _, err = run(self.ctx, "check", "6")
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("was rewritten", err)

    def test_a_lane_rewritten_after_its_report_is_refused(self) -> None:
        self.fake.add_comment(6, f"Done at {self.second[:8]}.\n\n"
                              + gc.make_marker("report", {"worker": "w", "state": "done", "head": self.second}))
        self.assertEqual(run(self.ctx, "check", "6")[0], 0)
        fakes.git(self.worker, "reset", "-q", "--hard", "HEAD~1")
        fakes.commit(self.worker, {"docs/c.md": "c\n"}, fakes.msg("docs(plans): R10.T13 Other"))
        fakes.git(self.worker, "push", "-q", "--force", "origin", f"HEAD:refs/heads/{LANE}")
        fakes.git(self.main, "fetch", "-q", "origin")  # an ordinary fetch quietly takes the rewrite
        code, out, _ = run(self.ctx, "check", "6")
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn(f"no longer contains the reported head {self.second[:8]}", out)

    def test_a_lane_that_changes_the_collaborations_files_is_refused(self) -> None:
        fakes.commit(self.worker, {".claude/gh-collab/config.json": '{"allowed_authors": ["mallory"]}\n'},
                     fakes.msg("chore: R10.T13 Trust more"))
        fakes.git(self.worker, "push", "-q", "origin", f"HEAD:refs/heads/{LANE}")
        code, out, _ = run(self.ctx, "check", "6")
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("changes the collaboration's own files", out)

    def test_pick_gate_push_and_close(self) -> None:
        code, out, err = run(self.ctx, "close", "6")
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("not on origin/trunk yet", err)
        code, out, err = run(self.ctx, "pick", "6")
        self.assertEqual(code, 0, out + err)
        self.assertIn("picked 2 commit(s)", out)
        self.assertIn("tree: the same as the lane head's", out)
        self.assertIn("`git push --no-verify origin trunk`", out)
        self.assertEqual(fakes.git(self.main, "log", "--merges", "--format=%H", "origin/trunk..HEAD"), "")
        subjects = fakes.git(self.main, "log", "--format=%s", "origin/trunk..HEAD").splitlines()
        self.assertEqual(subjects, ["docs(plans): R10.T13 Review fixes", "docs(plans): R10.T13 Draft"])
        self.assertIn("there are no commits to land", run(self.ctx, "check", "6")[1])  # all picked already
        code, out, err = run(self.ctx, "gate", "run", "6")
        self.assertEqual(code, 0, out + err)
        self.assertEqual(run(self.ctx, "gate", "check", "6")[0], 0)
        code, _, err = run(self.ctx, "close", "6")
        self.assertIn("push the integration branch first", err)  # picked, not pushed
        fakes.git(self.main, "push", "-q", "origin", "trunk")  # the orchestrator's own step
        code, out, err = run(self.ctx, "close", "6", "--dry-run")
        self.assertEqual(code, 0, err)
        new = fakes.git(self.main, "rev-parse", "HEAD")
        self.assertIn(f"- `{new[:8]}` docs(plans): R10.T13 Review fixes (lane `{self.second[:8]}`)", out)
        self.assertIn("Integration gate: `gh-collab gate: exit 0`", out)
        self.assertEqual(self.fake.issues[6]["state"], "OPEN")
        self.assertEqual(self.fake.writes(), [])
        self.assertEqual(run(self.ctx, "close", "6")[0], 0)
        self.assertEqual(self.fake.issues[6]["state"], "CLOSED")
        self.assertEqual(self.fake.status_of(6), "Done")

    def test_pick_refuses_the_wrong_branch_a_dirty_tree_and_stops_on_a_conflict(self) -> None:
        fakes.git(self.main, "switch", "-q", "-c", "other")
        self.assertIn("check out trunk first", run(self.ctx, "pick", "6")[2])
        fakes.git(self.main, "switch", "-q", "trunk")
        (self.main / "dirty.txt").write_text("x\n", encoding="utf-8")
        self.assertIn("uncommitted changes", run(self.ctx, "pick", "6")[2])
        (self.main / "dirty.txt").unlink()
        fakes.commit(self.main, {"docs/a.md": "conflicting\n"}, fakes.msg("docs: R10.T13 Conflicting draft"))
        code, out, _ = run(self.ctx, "pick", "6")
        self.assertEqual(code, gc.EXIT_FAIL)
        self.assertIn("cherry-pick stopped", out)
        fakes.git(self.main, "cherry-pick", "--abort")


if __name__ == "__main__":
    unittest.main()
