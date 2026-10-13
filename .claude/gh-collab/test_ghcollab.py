#!/usr/bin/env python3
"""Tests for ghcollab.py, the gh-collab skills' shared module:
`python3 -I -B .claude/gh-collab/test_ghcollab.py`.

Standard library only. `gh` is never run: a fake answers from fakes.py's model, and the runner
refuses `gh` outright under GH_COLLAB_FORBID_GH. Git runs on throwaway repositories under
$GH_COLLAB_TEST_TMP, or else this clone's git directory, never the system's temporary directory.
"""

from __future__ import annotations

import ast
import io
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import fakes  # noqa: E402
import ghcollab as gc  # noqa: E402

LINK = "https://claude.ai/" + "code/session_01ABC"  # split so that no file holds a link


class ConfigTests(unittest.TestCase):
    def test_the_committed_configuration_loads_and_validates(self) -> None:
        cfg = gc.load_config(paths=[gc.REPO_CONFIG])
        self.assertTrue(cfg["allowed_authors"])
        self.assertEqual(cfg["labels"]["task"], "agent-task")
        self.assertTrue(cfg["commit"]["trailer"])
        self.assertTrue(gc.re.fullmatch(cfg["commit"]["trailer_pattern"], cfg["commit"]["trailer"]))
        self.assertEqual(gc.discussion_number(cfg), 7)
        self.assertEqual(cfg["worker"]["label_limits"], {"heavy": 1})
        self.assertNotIn("//", cfg)

    def test_layers_merge_key_by_key(self) -> None:
        with fakes.Sandbox() as box:
            one, two = box.dir / "one.json", box.dir / "two.json"
            one.write_text(json.dumps({"labels": {"task": "todo"}, "worker": {"max_parallel": 3}}), encoding="utf-8")
            two.write_text(json.dumps({"worker": {"max_parallel": 1}}), encoding="utf-8")
            cfg = gc.load_config(paths=[one, two, box.dir / "missing.json"])
            self.assertEqual(cfg["labels"]["task"], "todo")
            self.assertEqual(cfg["labels"]["done"], "agent-done")
            self.assertEqual(cfg["worker"]["max_parallel"], 1)
            self.assertEqual(cfg["worker"]["label_limits"], {"heavy": 1})

    def test_an_unreadable_or_invalid_configuration_fails_closed(self) -> None:
        with fakes.Sandbox() as box:
            bad = box.dir / "bad.json"
            bad.write_text("{not json", encoding="utf-8")
            with self.assertRaises(gc.CollabError):
                gc.load_config(paths=[bad])
            bad.write_text(json.dumps({"allowed_authors": "owner"}), encoding="utf-8")
            with self.assertRaises(gc.CollabError):
                gc.load_config(paths=[bad])
            bad.write_text(json.dumps({"labels": {"done": "agent-task"}}), encoding="utf-8")
            with self.assertRaises(gc.CollabError):
                gc.load_config(paths=[bad])
            bad.write_text(json.dumps({"branch_prefix": "../"}), encoding="utf-8")
            with self.assertRaises(gc.CollabError):
                gc.load_config(paths=[bad])

    def test_empty_values_resolve_from_the_repository(self) -> None:
        fake = fakes.FakeGitHub()
        cfg = gc.resolve_repo(dict(gc.DEFAULTS), gc.Gh(runner=fake))
        self.assertEqual((cfg["repo"], cfg["base_branch"], cfg["allowed_authors"]), (fakes.REPO, "trunk", ["owner"]))

    def test_gh_is_refused_while_forbidden(self) -> None:
        with mock.patch.dict(os.environ, {"GH_COLLAB_FORBID_GH": "1"}):
            with self.assertRaises(gc.CollabError):
                gc.run_process(["gh", "issue", "list"])


class TrustTests(unittest.TestCase):
    def setUp(self) -> None:
        self.cfg = fakes.make_config(Path("."))
        self.fake = fakes.FakeGitHub()
        self.gh = gc.Gh(fakes.REPO, runner=self.fake)

    def issue(self, number: int) -> gc.Issue:
        return gc.fetch_issue(self.gh, number)

    def test_an_open_task_issue_by_an_allowed_author_passes(self) -> None:
        self.fake.add_issue(1)
        self.assertEqual(gc.trust_check(self.issue(1), self.cfg), [])

    def test_each_failure_is_named(self) -> None:
        self.fake.add_issue(2, author="mallory")
        self.fake.add_issue(3, labels=["bug"])
        self.fake.add_issue(4, state="CLOSED")
        self.fake.add_issue(5)
        self.fake.editors[5] = "mallory"
        self.fake.add_issue(6)
        self.fake.editors[6] = ""  # edited by an account since deleted
        self.fake.add_issue(7)
        self.fake.editors[7] = "owner"
        reasons = {n: " ".join(gc.trust_check(self.issue(n), self.cfg)) for n in (2, 3, 4, 5, 6, 7)}
        self.assertIn("mallory is not an allowed author", reasons[2])
        self.assertIn("lacks the agent-task label", reasons[3])
        self.assertIn("closed", reasons[4])
        self.assertIn("last edited by mallory", reasons[5])
        self.assertIn("no longer exists", reasons[6])
        self.assertEqual(reasons[7], "")
        with self.assertRaises(gc.CollabError) as ctx:
            gc.require_trusted(self.issue(2), self.cfg)
        self.assertEqual(ctx.exception.code, gc.EXIT_UNTRUSTED)

    def test_no_allowed_authors_trusts_no_one(self) -> None:
        self.fake.add_issue(1)
        cfg = dict(self.cfg, allowed_authors=[])
        self.assertTrue(gc.trust_check(self.issue(1), cfg))

    def test_markers_count_only_from_allowed_authors(self) -> None:
        self.fake.add_issue(1)
        claim = gc.make_marker("claim", {"worker": "w-evil", "nonce": "1"})
        self.fake.add_comment(1, claim, author="mallory")
        self.fake.add_comment(1, gc.make_marker("claim", {"worker": "w-good", "nonce": "2"}))
        self.assertEqual(gc.winning_claim(self.issue(1), self.cfg).worker, "w-good")

    def test_the_earliest_claim_since_the_last_report_wins(self) -> None:
        self.fake.add_issue(1)
        self.fake.add_comment(1, gc.make_marker("claim", {"worker": "w-a", "nonce": "1"}))
        self.fake.add_comment(1, gc.make_marker("report", {"worker": "w-a", "state": "blocked"}))
        self.fake.add_comment(1, gc.make_marker("claim", {"worker": "w-b", "nonce": "2"}), at="2026-01-05T13:00:00Z")
        self.fake.add_comment(1, gc.make_marker("claim", {"worker": "w-c", "nonce": "3"}), at="2026-01-05T13:00:00Z")
        self.assertEqual(gc.winning_claim(self.issue(1), self.cfg).worker, "w-b")  # same second: the lower id
        self.fake.add_comment(1, gc.make_marker("answer", {"requeue": True}), at="2026-01-05T14:00:00Z")
        self.assertIsNone(gc.winning_claim(self.issue(1), self.cfg))


class MarkerAndMetaTests(unittest.TestCase):
    def test_a_marker_round_trips_and_cannot_close_its_comment(self) -> None:
        marker = gc.make_marker("brief", {"slug": "x", "note": "a --> b"})
        self.assertEqual(marker.count("-->"), 1)
        self.assertEqual(gc.find_markers("text " + marker), [("brief", {"slug": "x", "note": "a --> b"})])
        self.assertEqual(gc.strip_markers("keep " + marker), "keep")

    def test_a_hand_written_brief_yields_its_slug_base_and_task(self) -> None:
        cfg = fakes.make_config(Path("."))
        body = ("### Base\nBranch from `origin/trunk` at **be8f8a6f** or later. Name the branch "
                "`lane/<this issue's number>-r10-t13`.\n")
        issue = gc.issue_from({"number": 5, "title": "R10.T13 Guide drafts", "body": body, "labels": [],
                               "author": {"login": "owner"}, "state": "OPEN"})
        meta = gc.brief_meta(issue, cfg)
        self.assertEqual((meta["slug"], meta["base_commit"], meta["task"]), ("r10-t13", "be8f8a6f", "R10.T13"))
        self.assertEqual(gc.issue_lane(issue, cfg)[0], "lane/5-r10-t13")

    def test_a_slug_that_is_not_a_plain_branch_part_is_refused(self) -> None:
        cfg = fakes.make_config(Path("."))
        issue = gc.issue_from({"number": 5, "title": "x", "labels": [], "author": {}, "state": "OPEN",
                               "body": gc.make_marker("brief", {"slug": "../../trunk"})})
        with self.assertRaises(gc.CollabError):
            gc.brief_meta(issue, cfg)


class HygieneTests(unittest.TestCase):
    cfg = fakes.make_config(Path("."))

    def rules(self, text: str, kind: str = "comment") -> list:
        with mock.patch.object(gc.socket, "gethostname", return_value="quasar-box.local"):
            return [f.rule for f in gc.errors(gc.hygiene(text, self.cfg, kind))]

    def test_clean_text_passes(self) -> None:
        text = f"Done.\n\n{fakes.TRAILER}\nPushed via git@github.com:owner/repo.git; ask ann@users.noreply.github.com"
        self.assertEqual(self.rules(text), [])

    def test_personal_details_links_and_secrets_are_refused(self) -> None:
        self.assertIn("email address", self.rules("mail me at someone@example.org"))
        self.assertIn("local home path", self.rules("log in /home/alice/run.log"))
        self.assertIn("local home path", self.rules("see /Users/bob/x"))
        self.assertEqual(self.rules("generic /home/user/x"), [])
        self.assertIn("this machine's host name", self.rules("built on QUASAR-BOX today"))
        self.assertIn("Claude session link", self.rules(f"see {LINK}"))
        self.assertEqual(self.rules("Rule: no `claude.ai/" + "code/session` link and no `Claude-Session:` line."), [])
        self.assertIn("Claude session link", self.rules("Claude-" + "Session: abc"))
        self.assertIn("possible secret (GitHub token)", self.rules("ghp_" + "a" * 36))
        self.assertIn("possible secret (private key)", self.rules("-----BEGIN RSA " + "PRIVATE KEY-----"))

    def test_a_plain_word_host_name_is_refused_only_as_a_host(self) -> None:
        with mock.patch.object(gc.socket, "gethostname", return_value="galaxy"):
            def rules(text: str) -> list:
                return [f.rule for f in gc.errors(gc.hygiene(text, self.cfg))]

            self.assertEqual(rules("The galaxy's disc; galaxy-generation plans."), [])
            for text in ("ssh galaxy.local", "quantum@galaxy", "galaxy:~/run$ ls"):
                with self.subTest(text=text):
                    self.assertIn("this machine's host name", rules(text))

    def test_brief_only_rules(self) -> None:
        self.assertIn("an unfilled [[FILL]] placeholder", self.rules("[[FILL: x]]", "brief"))
        self.assertTrue(any(".git directory" in r for r in self.rules("read .git/rm45-orchestration/x.md", "brief")))
        self.assertEqual(self.rules("[[FILL: x]]", "comment"), [])

    def test_a_diff_is_checked_for_links_and_secrets_only(self) -> None:
        self.assertEqual(self.rules("author = someone@example.org", "diff"), [])
        self.assertTrue(self.rules(f"url = '{LINK}'", "diff"))


class GitTests(unittest.TestCase):
    def test_commit_checks(self) -> None:
        cfg_scope = {"task": "R10.T13", "scope": ["docs/*"]}
        with fakes.Sandbox() as box:
            _, main = fakes.make_repos(box.dir)
            cfg = fakes.make_config(box.dir / "scratch")
            git = gc.Git(str(main))
            good = fakes.commit(main, {"docs/a.md": "a\n"}, fakes.msg("docs(plans): R10.T13 Draft"))
            fakes.git(main, "checkout", "-q", "-b", "side", "HEAD~1")
            fakes.commit(main, {"docs/b.md": "b\n"}, fakes.msg("docs: side"))
            fakes.git(main, "checkout", "-q", "trunk")
            fakes.git(main, "merge", "-q", "--no-edit", "side")
            bad_subject = fakes.commit(main, {"docs/c.md": "c\n"}, fakes.msg("Draft more"))
            two_trailers = fakes.commit(main, {"docs/d.md": "d\n"}, fakes.msg("docs: R10.T13 x", trailer=fakes.TRAILER
                                                                              + "\n" + fakes.TRAILER))
            outside = fakes.commit(main, {"src/e.rs": f"// {LINK}\n"}, fakes.msg("feat(x): R10.T13 code"))
            big = fakes.commit(main, {"docs/big.bin": "x" * 600_000}, fakes.msg("docs: R10.T13 big"))
            picks, merges = gc.lane_commits(git, "origin/trunk", "HEAD")
            self.assertEqual(len(merges), 1)
            self.assertNotIn(merges[0], picks)
            report = gc.check_commits(git, cfg, cfg_scope, picks, merges)
            text = "\n".join(report.problems)
            self.assertNotIn(good[:8], text)
            self.assertIn(f"{bad_subject[:8]} 'Draft more': its subject is not a conventional commit", text)
            self.assertIn("does not name the task R10.T13", text)
            self.assertIn(f"{two_trailers[:8]} 'docs: R10.T13 x': it has 2 Co-Authored-By trailers", text)
            self.assertIn(f"{outside[:8]}: adds Claude session link", text)
            self.assertIn(f"{outside[:8]}: changes paths outside the brief's scope: src/e.rs", text)
            self.assertIn(f"{big[:8]}: adds docs/big.bin, 600 kB", text)
            self.assertEqual(len(report.notes), 1)

    def test_a_lane_cannot_change_the_rules_or_leak_who_wrote_it(self) -> None:
        with fakes.Sandbox() as box:
            _, main = fakes.make_repos(box.dir)
            cfg = fakes.make_config(box.dir / "scratch")
            git = gc.Git(str(main))
            config = fakes.commit(main, {".claude/gh-collab/config.json": "{}\n"}, fakes.msg("chore: R10.T13 trust"))
            hooks = fakes.commit(main, {".claude/settings.local.json": "{}\n"}, fakes.msg("chore: R10.T13 hooks"))
            leak = fakes.commit(main, {"docs/x.md": "x\n"}, fakes.msg("docs: R10.T13 x", "Logs in /home/alice/run."))
            with mock.patch.dict(os.environ, {"GIT_AUTHOR_EMAIL": "alice@alices-macbook.local"}):
                stranger = fakes.commit(main, {"docs/y.md": "y\n"}, fakes.msg("docs: R10.T13 y"))
            picks, merges = gc.lane_commits(git, "origin/trunk", "HEAD")
            text = "\n".join(gc.check_commits(git, cfg, {"task": "R10.T13"}, picks, merges,
                                              identity_ref="origin/trunk").problems)
            self.assertIn(f"{config[:8]}: changes the collaboration's own files", text)
            self.assertIn(f"{hooks[:8]}: changes the collaboration's own files", text)
            self.assertIn(f"{leak[:8]} 'docs: R10.T13 x': its message holds local home path", text)
            self.assertIn(f"{stranger[:8]}: its author address 'alice@alices-macbook.local' is new", text)
            self.assertNotIn(f"{stranger[:8]}: its committer", text)  # the committer is known

    def test_commits_on_the_remote_base_are_never_the_lanes(self) -> None:
        with fakes.Sandbox() as box:
            _, main = fakes.make_repos(box.dir)
            other = fakes.commit(main, {"README.md": "moved\n"}, fakes.msg("feat: someone else's"))
            fakes.git(main, "push", "-q", "origin", "trunk")
            fakes.git(main, "reset", "-q", "--hard", "HEAD~1")  # the local branch lags behind
            fakes.git(main, "checkout", "-q", "-b", "lane/1-x", "origin/trunk")
            own = fakes.commit(main, {"a.txt": "a\n"}, fakes.msg("feat: mine"))
            git = gc.Git(str(main))
            self.assertEqual(gc.lane_commits(git, "trunk", "lane/1-x")[0], [other, own])
            self.assertEqual(gc.lane_commits(git, "trunk", "lane/1-x", exclude_ref="origin/trunk")[0], [own])

    def test_commits_already_picked_are_not_picked_again(self) -> None:
        with fakes.Sandbox() as box:
            _, main = fakes.make_repos(box.dir)
            fakes.git(main, "checkout", "-q", "-b", "lane/1-x")
            first = fakes.commit(main, {"a.txt": "1\n"}, fakes.msg("feat: one"), date="1700000000 +0000")
            fakes.git(main, "checkout", "-q", "trunk")
            fakes.commit(main, {"a.txt": "0\n"}, fakes.msg("feat: trunk moves"))
            # The pick lands with a changed context (a conflict resolved), so patch-ids differ.
            fakes.git(main, "cherry-pick", "-X", "theirs", first)
            fakes.git(main, "checkout", "-q", "lane/1-x")
            fakes.git(main, "merge", "-q", "--no-edit", "-X", "ours", "trunk")
            second = fakes.commit(main, {"b.txt": "2\n"}, fakes.msg("feat: two"))
            picks, _ = gc.lane_commits(gc.Git(str(main)), "trunk", "lane/1-x")
            self.assertEqual(picks, [second])

    def test_the_gate_log_proves_the_tree_it_ran_on(self) -> None:
        with fakes.Sandbox() as box:
            _, main = fakes.make_repos(box.dir)
            git, scratch = gc.Git(str(main)), box.dir / "scratch"
            scratch.mkdir()
            log = scratch / "gate.log"
            (main / "new.txt").write_text("work\n", encoding="utf-8")  # gate before the commit
            ok = [sys.executable, "-c", "print('suites pass')"]
            self.assertEqual(gc.gate_run(git, log, ok, scratch), 0)
            self.assertEqual(fakes.git(main, "status", "--porcelain"), "?? new.txt")  # index untouched
            self.assertIn("uncommitted", " ".join(gc.gate_check(git, log)))
            fakes.git(main, "add", "new.txt")
            fakes.git(main, "commit", "-q", "-m", "feat: new")
            self.assertEqual(gc.gate_check(git, log), [])
            self.assertTrue(log.read_text(encoding="utf-8").rstrip().endswith("gh-collab gate: exit 0"))
            fakes.commit(main, {"new.txt": "changed\n"}, "fix: after the gate")
            self.assertIn("different tree", " ".join(gc.gate_check(git, log)))
            fail = [sys.executable, "-c", "import sys; sys.exit(3)"]
            self.assertEqual(gc.gate_run(git, log, fail, scratch), 3)
            self.assertIn("did not pass", " ".join(gc.gate_check(git, log)))
            shell = [f'"{sys.executable}" -c "print(1)"']
            self.assertEqual(gc.gate_run(git, log, shell, scratch), 0)
            self.assertEqual(gc.gate_command_of(log), shell[0])

    def test_only_the_configured_gate_counts_for_skipping_the_hook(self) -> None:
        gate = "just smart-ci origin/trunk"
        self.assertTrue(gc.gate_is_configured(gate, gate))
        self.assertTrue(gc.gate_is_configured(f"HEAVY_MEMORY_MAX=10G CARGO_BUILD_JOBS=4 {gate}", gate))
        self.assertTrue(gc.gate_is_configured(f"systemd-run --user --scope -- timeout 8000 {gate}", gate))
        self.assertFalse(gc.gate_is_configured("true", gate))
        self.assertFalse(gc.gate_is_configured(f"{gate} || true", gate))
        self.assertFalse(gc.gate_is_configured(gate, ""))

    def test_a_body_can_come_from_standard_input(self) -> None:
        with mock.patch.object(sys, "stdin", io.StringIO("from stdin\n")):
            self.assertEqual(gc.read_body("-"), "from stdin\n")
        self.assertEqual(gc.read_body(""), "")
        with self.assertRaises(gc.CollabError):
            gc.read_body("/nonexistent/gh-collab/body.md")


class BoardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.cfg = fakes.make_config(Path("."))
        self.fake = fakes.FakeGitHub()
        self.gh = gc.Gh(fakes.REPO, runner=self.fake)
        self.fake.add_issue(1)

    def test_a_card_is_added_and_moved_by_option_name(self) -> None:
        url = self.fake.issues[1]["url"]
        self.assertEqual(gc.board_update(self.cfg, self.gh, url, "claimed"), "board: Claimed")
        self.assertEqual(gc.board_update(self.cfg, self.gh, url, "done"), "board: In progress")
        self.assertEqual(self.fake.status_of(1), "In progress")
        self.assertEqual(len(self.fake.items), 1)

    def test_without_the_scope_the_board_is_skipped_cleanly(self) -> None:
        self.fake.scopes = "gist, read:org, repo"
        note = gc.board_update(self.cfg, self.gh, self.fake.issues[1]["url"], "claimed")
        self.assertIn("skipped", note)
        self.assertIn("gh auth refresh -s project", note)
        self.assertFalse([c for c, _ in self.fake.calls if c[0] == "project"])

    def test_a_missing_option_is_reported_not_raised(self) -> None:
        del self.fake.options["Blocked"]
        note = gc.board_update(self.cfg, self.gh, self.fake.issues[1]["url"], "blocked")
        self.assertIn("not updated", note)
        self.assertEqual(gc.Board(self.cfg, self.gh).missing_options(), ["Blocked"])

    def test_the_status_follows_labels_and_markers(self) -> None:
        cfg = self.cfg
        self.fake.add_issue(2, labels=["agent-task", "agent-claimed"])
        self.assertEqual(gc.board_key(gc.fetch_issue(self.gh, 1), cfg), "queued")
        self.assertEqual(gc.board_key(gc.fetch_issue(self.gh, 2), cfg), "claimed")
        self.fake.add_comment(2, gc.make_marker("claim", {"worker": "w", "nonce": "1"}))
        self.fake.add_comment(2, gc.make_marker("start", {"worker": "w"}))
        self.assertEqual(gc.board_key(gc.fetch_issue(self.gh, 2), cfg), "started")
        self.fake.issues[2]["state"] = "CLOSED"
        self.assertEqual(gc.board_key(gc.fetch_issue(self.gh, 2), cfg), "landed")


class SchedulingTests(unittest.TestCase):
    def test_heavy_issues_run_one_at_a_time_beside_light_ones(self) -> None:
        cfg = fakes.make_config(Path("."))

        def issue(number: int, *labels: str) -> gc.Issue:
            return gc.issue_from({"number": number, "title": "t", "labels": [{"name": n} for n in labels],
                                  "author": {"login": "owner"}, "state": "OPEN"})

        heavy4, heavy6, light8, light9 = issue(4, "heavy"), issue(6, "heavy"), issue(8), issue(9)
        self.assertEqual(gc.waiting_reason(heavy6, [heavy4], cfg), "heavy slot busy (#4)")
        self.assertEqual(gc.waiting_reason(light8, [heavy4], cfg), "")
        self.assertEqual(gc.waiting_reason(light9, [heavy4, light8], cfg), "all 2 slots busy (#4, #8)")
        self.assertEqual(gc.waiting_reason(heavy6, [], cfg), "")
        never = gc.deep_merge(cfg, {"worker": {"label_limits": {"heavy": 0}}})
        self.assertEqual(gc.waiting_reason(heavy6, [], never), "this worker takes no heavy issues")


class Py39Tests(unittest.TestCase):
    def test_every_script_parses_as_python_3_9(self) -> None:
        """macOS's system Python is 3.9: the scripts must keep to its grammar."""
        root = HERE.parent
        files = sorted(HERE.glob("*.py")) + sorted(root.glob("skills/*/scripts/*.py"))
        names = {f.name for f in files}
        for name in ("ghcollab.py", "brief_issue.py", "work_issue.py", "guard_bash.py", "land_issue.py",
                     "survey_issues.py", "post_status.py"):
            self.assertIn(name, names)
        for path in files:
            if path.parent.name == "scripts" and path.parent.parent.name not in (
                    "brief-issue", "work-issue", "land-issue", "survey-issues", "post-status"):
                continue
            with self.subTest(path=path.name):
                ast.parse(path.read_text(encoding="utf-8"), filename=str(path), feature_version=(3, 9))

    def test_the_cli_runs_isolated(self) -> None:
        with fakes.Sandbox() as box:
            note = box.dir / "note.md"
            note.write_text("all clean\n", encoding="utf-8")
            proc = subprocess.run([sys.executable, "-I", str(HERE / "ghcollab.py"), "hygiene", str(note)],
                                  capture_output=True, text=True, cwd=str(HERE))
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertIn("hygiene: clean", proc.stdout)


if __name__ == "__main__":
    unittest.main()
