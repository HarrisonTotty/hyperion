#!/usr/bin/env python3
"""Tests of the gate stamps (`stamp.py`), run with smart-ci's own (`test_smart_ci.py` loads them).

The stamp's key, on a scratch repository: the same tree gives the same id, and any change of content,
an untracked file's included, another. The stamps' storage, and what a run leaves: a stamp when it
passed on an unchanged tree, none and its tree's earlier one withdrawn when it failed. What covers
each hook's check, when a stamp is valid, and the decision for the tree being committed or pushed.
And the hooks' wiring: `.pre-commit-config.yaml` runs the five expensive checks through `stamp.py hook`
with today's commands, and a dry run of each entry on a scratch repository skips the check with a
covering stamp and runs it without one.

Standard library only: `python3 tools/smart-ci/test_stamp.py`.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path
from typing import Dict, List, Optional, Sequence

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import stamp  # noqa: E402

# Git's variables that point a command at another repository or index, which a test's scratch
# repository must not inherit (from a hook that runs these tests, say), and the hooks' own.
SCRUBBED = re.compile(r"^(GIT_(DIR|WORK_TREE|INDEX_FILE|OBJECT_DIRECTORY|ALTERNATE_OBJECT_DIRECTORIES|COMMON_DIR|PREFIX|"
                      r"NAMESPACE)|PRE_COMMIT_.*|SMART_CI_NO_STAMPS|SMART_CI_BASE)$")
# The scratch repository's own configuration, so that the user's (Git for Windows' `core.autocrlf`,
# say) changes nothing that the tests compare.
GIT_CONFIG = {"user.name": "stamp tests", "user.email": "stamp-tests@example.invalid", "commit.gpgsign": "false",
              "core.autocrlf": "false", "core.fsmonitor": "false"}
TOOLS = {"rustc": "rustc 1.98.1 (test)", "cargo": "cargo 1.98.1 (test)", "node": "v26.10.0", "pnpm": "12.5.1",
         "wasm-bindgen": "wasm-bindgen 0.2.129", "nextest": "cargo-nextest 0.9.146"}


def fake_probe(versions: Dict[str, str]):
    return lambda root, names: {name: versions.get(name, "") for name in names}


def steps(*recipes: Sequence[str]) -> List[dict]:
    """A plan's steps, each a recipe with its arguments (a bare name gets a `-p` selection)."""
    return [{"phase": "main", "title": r if isinstance(r, str) else r[0],
             "recipe": [r, "-p x"] if isinstance(r, str) else list(r), "causes": ["a path"]} for r in recipes]


WORKSPACE_CLIPPY = ("_clippy-for", "--workspace")
WORKSPACE_BINDINGS = ("_gen-protocol-check-for", "--workspace")


class Scratch(unittest.TestCase):
    """A scratch repository with one commit on the integration branch, pushed (its remote-tracking
    copy), and git's environment scrubbed."""

    def setUp(self) -> None:
        patcher = unittest.mock.patch.dict(os.environ)
        patcher.start()
        self.addCleanup(patcher.stop)
        for name in [n for n in os.environ if SCRUBBED.match(n)]:
            del os.environ[name]
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.repo = Path(directory.name) / "repo"
        self.repo.mkdir()
        (Path(directory.name) / "no-hooks").mkdir()
        self.git("init", "-q")
        for key, value in {**GIT_CONFIG, "core.hooksPath": str(Path(directory.name) / "no-hooks")}.items():
            self.git("config", key, value)
        self.git("checkout", "-q", "-b", stamp.INTEGRATION_DEFAULT)
        self.write(".gitignore", "target/\n")
        self.write("src/lib.rs", "pub fn f() {}\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "base")
        self.base = self.git("rev-parse", "HEAD")
        self.git("update-ref", f"refs/remotes/origin/{stamp.INTEGRATION_DEFAULT}", self.base)
        self.git("checkout", "-q", "-b", "lane")

    def git(self, *args: str) -> str:
        return subprocess.run(["git", *args], cwd=str(self.repo), check=True, capture_output=True,
                              text=True).stdout.strip()

    def write(self, path: str, text: str) -> None:
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(text.encode("utf-8"))

    def commit(self, message: str = "change") -> str:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD")

    def run_record(self, tree: str, **fields: object) -> dict:
        """A passing run's record, as smart-ci writes it (`smart_ci.stamp_run`), with this environment."""
        run = {"tree": tree, "head": self.git("rev-parse", "HEAD"), "base": self.base,
               "base_ref": stamp.INTEGRATION_DEFAULT, "full": False, "full_because": [], "options": {},
               "steps": [], "changes": [["src/lib.rs", False]], "toolchain": dict(TOOLS),
               "environment": stamp.environment(), "configuration": stamp.configuration(self.repo),
               "time": "2026-10-15T18:00:00Z", "exit": 0}
        run.update(fields)
        return run

    def now(self, env: Optional[Dict[str, str]] = None, tools: Optional[Dict[str, str]] = None) -> stamp.Now:
        return stamp.Now(self.repo, {**os.environ, **(env or {})}, fake_probe(TOOLS if tools is None else tools))


class TheKey(Scratch):
    """The stamp's key: the working tree's id, uncommitted and untracked files included."""

    def test_the_same_tree_gives_the_same_key_and_a_clean_one_heads(self) -> None:
        first = stamp.worktree_tree(self.repo)
        self.assertEqual(first, stamp.worktree_tree(self.repo))
        self.assertEqual(first, self.git("rev-parse", "HEAD^{tree}"))
        self.assertRegex(first, stamp.TREE_ID)

    def test_any_change_of_content_gives_another_key(self) -> None:
        clean = stamp.worktree_tree(self.repo)
        self.write("src/lib.rs", "pub fn f() { }\n")
        edited = stamp.worktree_tree(self.repo)
        self.assertNotEqual(edited, clean)
        self.write("src/lib.rs", "pub fn f() {}\n")
        self.assertEqual(stamp.worktree_tree(self.repo), clean)
        self.git("add", "src/lib.rs")
        self.write("src/lib.rs", "pub fn g() {}\n")
        # Staged or not, the working tree's content is the key.
        self.assertNotIn(stamp.worktree_tree(self.repo), (clean, edited))

    def test_an_untracked_file_is_in_the_key_and_an_ignored_one_is_not(self) -> None:
        clean = stamp.worktree_tree(self.repo)
        self.write("target/debug/out.txt", "built\n")
        self.assertEqual(stamp.worktree_tree(self.repo), clean)
        self.write("src/new.rs", "pub fn n() {}\n")
        with_new = stamp.worktree_tree(self.repo)
        self.assertNotEqual(with_new, clean)
        self.write("src/new.rs", "pub fn n() { }\n")
        self.assertNotIn(stamp.worktree_tree(self.repo), (clean, with_new))
        # The key is the tree that `git add -A` and a commit record.
        key = stamp.worktree_tree(self.repo)
        self.git("add", "-A")
        self.assertEqual(self.git("write-tree"), key)

    @unittest.skipIf(os.name == "nt", "no executable bit")
    def test_a_files_mode_is_in_the_key(self) -> None:
        clean = stamp.worktree_tree(self.repo)
        os.chmod(self.repo / "src/lib.rs", 0o755)
        self.assertNotEqual(stamp.worktree_tree(self.repo), clean)

    def test_the_index_is_left_as_it_was(self) -> None:
        self.write("src/new.rs", "pub fn n() {}\n")
        self.write("src/lib.rs", "pub fn f() { }\n")
        index = Path(self.git("rev-parse", "--absolute-git-dir")) / "index"
        before = index.read_bytes()
        stamp.worktree_tree(self.repo)
        self.assertEqual(index.read_bytes(), before)
        self.assertEqual(self.git("diff", "--cached", "--name-only"), "")
        self.assertEqual(self.git("ls-files", "--others", "--exclude-standard"), "src/new.rs")
        self.assertFalse(list(Path(self.git("rev-parse", "--absolute-git-dir")).glob("smart-ci-index-*")))

    def test_edits_git_does_not_see_refuse_a_stamp(self) -> None:
        self.assertEqual(stamp.hidden_entries(self.repo), [])
        self.assertEqual(set(stamp.start(self.repo)), {"tree", "toolchain", "environment", "configuration"})
        self.git("update-index", "--assume-unchanged", "src/lib.rs")
        self.assertEqual(stamp.hidden_entries(self.repo), ["src/lib.rs"])
        with self.assertRaises(stamp.StampError):
            stamp.start(self.repo)
        self.git("update-index", "--no-assume-unchanged", "src/lib.rs")
        self.git("update-index", "--skip-worktree", "src/lib.rs")
        self.assertEqual(stamp.hidden_entries(self.repo), ["src/lib.rs"])

    def test_it_is_the_tree_gh_collabs_gate_log_records(self) -> None:
        ghcollab = load_ghcollab(self)
        self.write("src/lib.rs", "pub fn f() { }\n")
        self.write("docs/new.md", "new\n")
        scratch = self.repo.parent / "gh-collab-scratch"
        scratch.mkdir()
        self.assertEqual(ghcollab.worktree_tree(ghcollab.Git(str(self.repo)), scratch), stamp.worktree_tree(self.repo))

    def test_stamps_are_kept_in_the_common_directory_for_every_worktree(self) -> None:
        other = self.repo.parent / "other"
        self.git("worktree", "add", "-q", "-b", "other", str(other))
        common = (self.repo / ".git").resolve()
        self.assertEqual(stamp.stamps_dir(self.repo).resolve(), common / "smart-ci" / "stamps")
        self.assertEqual(stamp.stamps_dir(other).resolve(), common / "smart-ci" / "stamps")


class TheStamps(Scratch):
    """Writing, reading, withdrawing and pruning stamps, and what a run leaves."""

    def test_a_tree_keeps_its_last_runs_and_reads_back_only_passes_of_its_own(self) -> None:
        tree = stamp.worktree_tree(self.repo)
        self.assertEqual(stamp.runs_of(self.repo, tree), [])
        for i in range(stamp.KEEP_RUNS + 2):
            path = stamp.write(self.repo, self.run_record(tree, time=f"run {i}"))
        self.assertEqual(path, stamp.stamps_dir(self.repo) / f"{tree}.json")
        self.assertEqual([r["time"] for r in stamp.runs_of(self.repo, tree)],
                         [f"run {i}" for i in range(2, stamp.KEEP_RUNS + 2)])
        self.assertEqual(list(path.parent.glob(".*.tmp")), [])
        data = json.loads(path.read_text(encoding="utf-8"))
        data["runs"].append(self.run_record("0" * 40))
        data["runs"].append(self.run_record(tree, exit=1))
        path.write_text(json.dumps(data), encoding="utf-8")
        self.assertEqual(len(stamp.runs_of(self.repo, tree)), stamp.KEEP_RUNS)
        self.assertTrue(stamp.withdraw(self.repo, tree))
        self.assertFalse(stamp.withdraw(self.repo, tree))
        self.assertEqual(stamp.runs_of(self.repo, tree), [])

    def test_a_tree_id_is_checked_before_it_names_a_file(self) -> None:
        with self.assertRaises(stamp.StampError):
            stamp.stamp_path(self.repo, "../../outside")

    def test_old_stamps_are_pruned(self) -> None:
        tree = stamp.worktree_tree(self.repo)
        old = stamp.write(self.repo, self.run_record(tree))
        long_ago = old.stat().st_mtime - (stamp.KEEP_DAYS + 1) * 86400
        os.utime(old, (long_ago, long_ago))
        stamp.prune(old.parent)
        self.assertFalse(old.exists())

    def test_a_pass_stamps_its_tree_and_a_failure_withdraws_it(self) -> None:
        self.write("src/lib.rs", "pub fn f() { }\n")
        before = stamp.worktree_tree(self.repo)
        line = stamp.record(self.repo, before, 0, lambda tree: self.run_record(tree))
        self.assertIn(f"stamped tree {before[:10]}", line)
        self.assertEqual(len(stamp.runs_of(self.repo, before)), 1)
        line = stamp.record(self.repo, before, 1, lambda tree: self.run_record(tree))
        self.assertIn("no stamp (exit 1)", line)
        self.assertIn("withdrew", line)
        self.assertEqual(stamp.runs_of(self.repo, before), [])
        self.assertIn("no stamp", stamp.record(self.repo, None, 0, lambda tree: self.run_record(tree)))

    def test_a_tree_that_changed_during_the_run_is_not_stamped(self) -> None:
        before = stamp.worktree_tree(self.repo)
        self.write("src/later.rs", "pub fn l() {}\n")
        line = stamp.record(self.repo, before, 0, lambda tree: self.run_record(tree))
        self.assertIn("changed during the run", line)
        self.assertEqual(stamp.runs_of(self.repo, before), [])
        self.assertEqual(stamp.runs_of(self.repo, stamp.worktree_tree(self.repo)), [])


class Coverage(unittest.TestCase):
    """What a passing run's plan covers of each hook's check."""

    def run_of(self, **fields: object) -> dict:
        run = {"tree": "a" * 40, "full": False, "steps": [], "changes": [["crates/x/src/lib.rs", False]], "exit": 0}
        run.update(fields)
        return run

    def test_a_full_plan_covers_every_check(self) -> None:
        full = self.run_of(full=True, changes=[["tools/smart-ci/stamp.py", False]])
        for check in stamp.CHECKS:
            with self.subTest(check=check):
                self.assertTrue(stamp.covers(full, check)[0])

    def test_clippy_and_the_bindings_are_covered_only_by_the_workspaces_own(self) -> None:
        for check, workspace in (("clippy", WORKSPACE_CLIPPY), ("protocol-bindings", WORKSPACE_BINDINGS)):
            with self.subTest(check=check):
                ok, why = stamp.covers(self.run_of(steps=steps(workspace)), check)
                self.assertTrue(ok)
                self.assertTrue(why.endswith("as the hook does"), why)
                # A `-p` selection, however wide, is not the workspace's build.
                ok, why = stamp.covers(self.run_of(steps=steps((workspace[0], "-p a -p b --features a/x"))), check)
                self.assertFalse(ok)
                self.assertIn("--hook-checks", why)
                self.assertFalse(stamp.covers(self.run_of(steps=steps("_oxlint")), check)[0])
        self.assertFalse(stamp.covers(self.run_of(steps=steps(WORKSPACE_CLIPPY)), "protocol-bindings")[0])

    def test_the_typescript_checks_need_their_whole_tree_step(self) -> None:
        for check, recipe in (("oxlint", "_oxlint"), ("typecheck", "_typecheck-ts")):
            with self.subTest(check=check):
                self.assertTrue(stamp.covers(self.run_of(steps=steps(recipe)), check)[0])
                others = [r for r in ("_oxlint", "_typecheck-ts") if r != recipe]
                ok, why = stamp.covers(self.run_of(steps=steps(*others, WORKSPACE_CLIPPY, WORKSPACE_BINDINGS)), check)
                self.assertFalse(ok)
                self.assertIn("ran no", why)

    def test_any_passing_plan_covers_just_test(self) -> None:
        self.assertTrue(stamp.covers(self.run_of(steps=steps("_prettier-check")), "test")[0])
        self.assertTrue(stamp.covers(self.run_of(), "test")[0])

    def test_a_change_to_smart_cis_own_rules_covers_nothing_but_a_full_plan(self) -> None:
        own = self.run_of(steps=steps(WORKSPACE_CLIPPY, "_oxlint"), changes=[["tools/smart-ci/stamp.py", False]])
        for check in stamp.CHECKS:
            with self.subTest(check=check):
                ok, why = stamp.covers(own, check)
                self.assertFalse(ok)
                self.assertIn("smart-ci's own rules", why)
        self.assertFalse(stamp.covers(self.run_of(changes=None), "test")[0])

    def test_the_hooks_checks_are_smart_cis_steps(self) -> None:
        # Each whole-tree recipe is one `just ci` runs (smart-ci's table of its steps).
        import smart_ci
        known = set(smart_ci.CI_STEPS) | {r for _, calls in smart_ci.CI_STEPS.values() for r, _ in calls}
        for recipe in stamp.HOOK_STEPS.values():
            self.assertIn(recipe[0], known)
        # Clippy's and the bindings' are `ci`'s own workspace-wide steps, the hooks' commands.
        self.assertIn(("_clippy-for", ["--workspace"]), smart_ci.CI_STEPS["_clippy"][1])
        self.assertIn(("_gen-protocol-check-for", ["--workspace"]), smart_ci.CI_STEPS["gen-protocol-check"][1])
        self.assertEqual(stamp.HOOK_STEPS["clippy"], list(WORKSPACE_CLIPPY))
        self.assertEqual(stamp.HOOK_STEPS["protocol-bindings"], list(WORKSPACE_BINDINGS))
        self.assertEqual(stamp.INTEGRATION_DEFAULT, "rendering-and-planets")
        self.assertIn(f'"SMART_CI_BASE") or "{stamp.INTEGRATION_DEFAULT}"', Path(smart_ci.__file__).read_text(encoding="utf-8"))


class Validity(Scratch):
    """A stamp is valid with a pushed base and the toolchain, environment and configuration it recorded."""

    def test_the_base_must_be_an_ancestor_of_the_remotes_integration_branch(self) -> None:
        tree = stamp.worktree_tree(self.repo)
        tools = stamp.CHECKS["clippy"].tools
        self.assertIsNone(stamp.problem_with(self.run_record(tree), tools, self.now()))
        self.write("src/lib.rs", "pub fn f() { }\n")
        lane = self.commit()
        problem = stamp.problem_with(self.run_record(tree, base=lane), tools, self.now())
        self.assertIn("is not an ancestor of origin/rendering-and-planets", problem or "")
        self.assertIsNone(stamp.problem_with(self.run_record(tree, base=lane, full=True), tools, self.now()))
        self.assertIsNotNone(stamp.problem_with(self.run_record(tree, base=None), tools, self.now()))
        # The local branch, which carries picks not yet gated, is not enough; once pushed, it is.
        self.git("update-ref", f"refs/heads/{stamp.INTEGRATION_DEFAULT}", lane)
        self.assertIsNotNone(stamp.problem_with(self.run_record(tree, base=lane), tools, self.now()))
        self.git("update-ref", f"refs/remotes/origin/{stamp.INTEGRATION_DEFAULT}", lane)
        self.assertIsNone(stamp.problem_with(self.run_record(tree, base=lane), tools, self.now()))
        self.assertIsNotNone(stamp.problem_with(self.run_record(tree, base=lane), tools,
                                                self.now({"SMART_CI_BASE": "elsewhere"})))

    def test_the_toolchain_must_be_the_one_it_recorded(self) -> None:
        run = self.run_record(stamp.worktree_tree(self.repo))
        newer = {**TOOLS, "rustc": "rustc 1.99.0 (test)"}
        problem = stamp.problem_with(run, stamp.CHECKS["clippy"].tools, self.now(tools=newer))
        self.assertIn("rustc rustc 1.98.1 (test), now rustc 1.99.0 (test)", problem or "")
        node = {**TOOLS, "node": "v27.0.0"}
        # A check that runs no node does not mind it; the TypeScript checks build with cargo as well.
        self.assertIsNotNone(stamp.problem_with(run, stamp.CHECKS["typecheck"].tools,
                                                self.now(tools={**TOOLS, "wasm-bindgen": "wasm-bindgen 0.2.130"})))
        self.assertIsNone(stamp.problem_with(run, stamp.CHECKS["clippy"].tools, self.now(tools=node)))
        self.assertIsNotNone(stamp.problem_with(run, stamp.CHECKS["test"].tools, self.now(tools=node)))
        self.assertIsNotNone(stamp.problem_with(run, stamp.CHECKS["oxlint"].tools, self.now(tools=newer)))
        # A tool that cannot be run here is no match, even for a record without it.
        problem = stamp.problem_with(self.run_record("a" * 40, toolchain={**TOOLS, "pnpm": ""}),
                                     stamp.CHECKS["test"].tools, self.now(tools={**TOOLS, "pnpm": ""}))
        self.assertIn("pnpm cannot be run here", problem or "")

    def test_the_environment_and_configuration_must_be_those_it_recorded(self) -> None:
        run = self.run_record(stamp.worktree_tree(self.repo))
        problem = stamp.problem_with(run, stamp.CHECKS["clippy"].tools, self.now({"RUSTFLAGS": "-A warnings"}))
        self.assertIn("environment differs from its run's (RUSTFLAGS)", problem or "")
        for harmless in ({"CARGO_BUILD_JOBS": "4"}, {"CARGO_TERM_COLOR": "always"}, {"HYPERION_HEAVY_HELD": "1"},
                         {"HEAVY_SLICE": "agents.slice"}):
            with self.subTest(env=harmless):
                self.assertIsNone(stamp.problem_with(run, stamp.CHECKS["clippy"].tools, self.now(harmless)))
        self.assertIsNotNone(stamp.problem_with(run, stamp.CHECKS["test"].tools, self.now({"HYPERION_WORKERS": "2"})))
        # The values are kept as digests only.
        recorded = stamp.environment({"CARGO_HOME": "/somewhere/secret", "RUSTFLAGS": "-C x", "CARGO_REGISTRY_TOKEN": "t"})
        self.assertEqual(set(recorded), {"CARGO_HOME", "RUSTFLAGS"})
        self.assertNotIn("secret", json.dumps(recorded))
        self.assertIsNotNone(stamp.problem_with(run, stamp.CHECKS["test"].tools, self.now({"CI": "true"})))
        # The node_modules installed, by pnpm's record of them.
        self.write("node_modules/.pnpm/lock.yaml", "lockfileVersion: '9.0'\n")
        problem = stamp.problem_with(run, stamp.CHECKS["oxlint"].tools, self.now())
        self.assertIn("node_modules", problem or "")

    def test_cargo_configuration_above_the_tree_counts_where_it_differs_from_the_trees_own(self) -> None:
        # A worktree nested under another checkout reads that checkout's `.cargo/` too.
        nested = self.repo / "nested-worktree"
        self.git("worktree", "add", "-q", "-b", "nested", str(nested))
        key = "cargo configs above the tree"
        self.write(".cargo/config.toml", "[env]\nA = '1'\n")
        (nested / ".cargo").mkdir()
        (nested / ".cargo" / "config.toml").write_bytes(b"[env]\nA = '1'\n")
        # The outer checkout's own, with the nested tree's bytes, counts as none: the two see the same
        # configuration from above them (whatever lies above the scratch directory).
        self.assertEqual(stamp.configuration(nested).get(key), stamp.configuration(self.repo).get(key))
        self.write(".cargo/config.toml", "[build]\nrustflags = ['-A', 'warnings']\n")
        self.assertNotEqual(stamp.configuration(nested).get(key), stamp.configuration(self.repo).get(key))


class Decisions(Scratch):
    """The decision for the tree being committed (the index's) or pushed (PRE_COMMIT_TO_REF's)."""

    def covering(self, tree: str) -> dict:
        return self.run_record(tree, steps=steps(WORKSPACE_CLIPPY, "_oxlint"))

    def test_a_commit_is_covered_by_a_stamp_of_its_index_tree(self) -> None:
        self.write("src/lib.rs", "pub fn f() { }\n")
        gated = stamp.worktree_tree(self.repo)
        probe = fake_probe(TOOLS)
        decision = stamp.decide(self.repo, "clippy", os.environ, probe)
        self.assertFalse(decision.covered)
        self.assertIn("has not passed on the tree being committed", decision.reason)
        stamp.write(self.repo, self.covering(gated))
        # Not staged yet: the commit would record another tree.
        self.assertFalse(stamp.decide(self.repo, "clippy", os.environ, probe).covered)
        self.git("add", "-A")
        decision = stamp.decide(self.repo, "clippy", os.environ, probe)
        self.assertTrue(decision.covered, decision.reason)
        self.assertIn("its plan ran the workspace's Clippy as the hook does", decision.reason)
        self.assertFalse(stamp.decide(self.repo, "typecheck", os.environ, probe).covered)
        self.assertFalse(stamp.decide(self.repo, "clippy", {**os.environ, stamp.OFF_VAR: "1"}, probe).covered)
        self.assertFalse(stamp.decide(self.repo, "clippy", os.environ, fake_probe({**TOOLS, "cargo": "x"})).covered)
        # A hook's own index (`git commit -a`, `git commit <paths>`) is the one it reads.
        alternative = Path(self.git("rev-parse", "--absolute-git-dir")) / "other-index"
        self.git("read-tree", f"--index-output={alternative}", "HEAD")
        with unittest.mock.patch.dict(os.environ, {"GIT_INDEX_FILE": str(alternative)}):
            self.assertFalse(stamp.decide(self.repo, "clippy", os.environ, probe).covered)

    def test_the_working_tree_the_check_reads_must_be_the_stamped_tree(self) -> None:
        self.write("src/lib.rs", "pub fn f() { }\n")
        stamp.write(self.repo, self.covering(stamp.worktree_tree(self.repo)))
        self.git("add", "-A")
        probe = fake_probe(TOOLS)
        self.assertTrue(stamp.decide(self.repo, "clippy", os.environ, probe).covered)
        # An unstaged edit (`pre-commit run --all-files` stashes none), or an untracked file.
        for path in ("src/lib.rs", "src/scratch.rs"):
            with self.subTest(path=path):
                self.write(path, "pub fn edited() {}\n")
                decision = stamp.decide(self.repo, "clippy", os.environ, probe)
                self.assertFalse(decision.covered)
                self.assertIn("working tree the check would read", decision.reason)
                if path == "src/lib.rs":
                    self.write(path, "pub fn f() { }\n")
                else:
                    (self.repo / path).unlink()
        self.assertTrue(stamp.decide(self.repo, "clippy", os.environ, probe).covered)
        self.git("update-index", "--assume-unchanged", "src/lib.rs")
        decision = stamp.decide(self.repo, "clippy", os.environ, probe)
        self.assertFalse(decision.covered)
        self.assertIn("assume-unchanged", decision.reason)

    def test_a_push_is_covered_by_a_stamp_of_the_pushed_commits_tree(self) -> None:
        self.write("src/lib.rs", "pub fn f() { }\n")
        stamp.write(self.repo, self.run_record(stamp.worktree_tree(self.repo), steps=steps("_prettier-check")))
        pushed = self.commit()
        probe = fake_probe(TOOLS)
        self.assertTrue(stamp.decide(self.repo, "test", {**os.environ, "PRE_COMMIT_TO_REF": pushed}, probe).covered)
        self.assertFalse(stamp.decide(self.repo, "test", {**os.environ, "PRE_COMMIT_TO_REF": self.base}, probe).covered)
        decision = stamp.decide(self.repo, "test", os.environ, probe)
        self.assertFalse(decision.covered)
        self.assertIn("PRE_COMMIT_TO_REF", decision.reason)

    def test_a_newer_run_that_does_not_cover_leaves_an_older_one_that_does(self) -> None:
        tree = stamp.worktree_tree(self.repo)
        stamp.write(self.repo, self.covering(tree))
        stamp.write(self.repo, self.run_record(tree, steps=steps("_prettier-check")))
        self.assertTrue(stamp.decide(self.repo, "oxlint", os.environ, fake_probe(TOOLS)).covered)

    def test_check_names_the_base_and_the_hooks_covered(self) -> None:
        self.write("src/lib.rs", "pub fn f() { }\n")
        stamp.write(self.repo, self.covering(stamp.worktree_tree(self.repo)))
        head = self.commit()
        status, lines = stamp.check_tree(self.repo, head, os.environ, fake_probe(TOOLS))
        self.assertEqual(status, 0)
        self.assertEqual(len(lines), 1)
        self.assertTrue(lines[0].startswith("pass: smart-ci passed on tree "), lines[0])
        self.assertIn(f"against {self.base[:10]} (its merge-base with rendering-and-planets)", lines[0])
        self.assertIn("covers the hooks' clippy, oxlint, test", lines[0])
        status, lines = stamp.check_tree(self.repo, head, os.environ, fake_probe({}))
        self.assertEqual(status, 1)
        self.assertTrue(lines[0].startswith("stale: "))
        status, lines = stamp.check_tree(self.repo, self.base, os.environ, fake_probe(TOOLS))
        self.assertEqual((status, lines[0][:9]), (1, "no stamp:"))


def load_ghcollab(test: unittest.TestCase):
    """gh-collab's module (`.claude/gh-collab/ghcollab.py`), or skip the test where this checkout has none."""
    source = ROOT / ".claude" / "gh-collab" / "ghcollab.py"
    if not source.is_file():
        test.skipTest("no gh-collab in this checkout")
    spec = importlib.util.spec_from_file_location("ghcollab_for_stamp_tests", source)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module  # its dataclasses look their module up there
    test.addCleanup(sys.modules.pop, spec.name, None)
    spec.loader.exec_module(module)
    return module


# A gate as the worker's brief spells it, which is not its configured command: smart-ci's two ends
# around a passing plan (`smart_ci.stamp_before`, `stamp_after`), as a run of `just smart-ci` has them.
GATE = """
import sys
sys.path.insert(0, sys.argv[1])
from pathlib import Path
import smart_ci
root = Path.cwd()
began = smart_ci.stamp_before(root)
plan = smart_ci.Plan({}, [smart_ci.Step("side", "Prettier", ["_prettier-check", "src/lib.rs"], ["src/lib.rs"])],
                     set(), {}, [])
change = smart_ci.Change([("src/lib.rs", False)], "x", False, sys.argv[2])
smart_ci.stamp_after(root, began, 0, plan, change, "origin/rendering-and-planets", {})
print("smart-ci exit 0")
"""


class TheWorkersPush(Scratch):
    """The laptop worker's path (main, 2026-10-15): its gate runs through gh-collab's `gate run` with the
    brief's command, whose text and environment are not the configured gate's, and its push then runs
    the pre-push hook. The hook keys on the stamp of the pushed tree, which smart-ci wrote, not on the
    command that ran it."""

    def test_a_gate_run_through_gh_collab_with_any_command_stamps_the_pushed_tree(self) -> None:
        ghcollab = load_ghcollab(self)
        tools = stamp.toolchain(self.repo)
        if not all(tools.values()):
            self.skipTest(f"not every tool runs here: {tools}")
        self.write("src/lib.rs", "pub fn f() { }\n")
        head = self.commit("the lane's commit")
        git = ghcollab.Git(str(self.repo))
        log = self.repo.parent / "gh-collab" / "issue-7" / "gate.log"
        command = [sys.executable, "-B", "-c", GATE, str(HERE), self.base]
        # The brief's gate set what the worker's configured one does not; none of it changes a result.
        with unittest.mock.patch.dict(os.environ, {"HEAVY_MEMORY_MAX": "12G", "CARGO_BUILD_JOBS": "8",
                                                  "SMART_CI_SLOT": "build-slot"}):
            self.assertEqual(ghcollab.gate_run(git, log, command, self.repo.parent), 0)
        self.assertEqual(ghcollab.gate_check(git, log), [])
        self.assertFalse(ghcollab.gate_is_configured(ghcollab.gate_command_of(log), "just smart-ci origin/rendering-and-planets"))
        logged_tree = re.search(r" tree=(\S+)", log.read_text(encoding="utf-8")).group(1)  # type: ignore[union-attr]
        self.assertEqual(len(stamp.runs_of(self.repo, logged_tree)), 1)
        # With this machine's tools, as the stamp recorded them and as the hook reads them.
        decision = stamp.decide(self.repo, "test", {**os.environ, "PRE_COMMIT_TO_REF": head})
        self.assertTrue(decision.covered, decision.reason)
        status, lines = stamp.check_tree(self.repo, head)
        self.assertEqual(status, 0, lines)


# The hooks that run through `stamp.py hook`, with the check each is and the command it ran before.
WRAPPED = {
    "cargo-clippy": ("clippy", "cargo clippy --workspace --all-targets -- -D warnings"),
    "oxlint": ("oxlint", "just _with-surface pnpm lint"),
    "typecheck": ("typecheck", "just _with-surface pnpm typecheck"),
    "protocol-bindings": ("protocol-bindings", "just gen-protocol-check"),
    "just-test": ("test", "just test"),
}
ENTRY = "python3 -I tools/smart-ci/stamp.py hook {check} -- {command}"


def hooks_of(config: str) -> Dict[str, Dict[str, str]]:
    """Each hook's id with its keys, read from `.pre-commit-config.yaml` (its keys are one line each)."""
    hooks: Dict[str, Dict[str, str]] = {}
    current: Optional[Dict[str, str]] = None
    for line in config.splitlines():
        m = re.match(r"^\s+- id: (\S+)\s*$", line)
        if m:
            current = hooks.setdefault(m.group(1), {})
            continue
        m = re.match(r"^\s{8}([a-z_]+): (.*)$", line)
        if m and current is not None:
            value = m.group(2).strip()
            current[m.group(1)] = value[1:-1] if value[:1] == value[-1:] == "'" else value
    return hooks


class TheHooks(Scratch):
    """The hooks' wiring, and a dry run of each wrapped entry."""

    def config(self) -> Dict[str, Dict[str, str]]:
        return hooks_of((ROOT / ".pre-commit-config.yaml").read_text(encoding="utf-8"))

    def test_the_expensive_checks_run_through_the_stamp_and_the_cheap_ones_as_they_were(self) -> None:
        hooks = self.config()
        for hook, (check, command) in WRAPPED.items():
            with self.subTest(hook=hook):
                self.assertEqual(hooks[hook]["entry"], ENTRY.format(check=check, command=command))
                self.assertEqual(hooks[hook].get("verbose"), "true", "the skip's reason shows only on a verbose hook")
                self.assertEqual(hooks[hook].get("stages", "[pre-commit]"), f"[{stamp.CHECKS[check].stage}]")
        for hook in set(hooks) - set(WRAPPED):
            with self.subTest(hook=hook):
                self.assertNotIn("stamp.py", hooks[hook].get("entry", ""))
        for hook in ("cargo-fmt", "prettier", "no-session-links-msg", "no-session-links-files", "check-added-large-files",
                     "trailing-whitespace", "end-of-file-fixer"):
            self.assertIn(hook, hooks)

    def dry_run(self, entry: str, env: Dict[str, str]) -> subprocess.CompletedProcess:
        """The hook's entry as pre-commit runs it, from the repository's root, with the check replaced by a
        command that says it ran and fails."""
        words = shlex.split(entry)
        at = words.index("--")
        self.assertEqual(words[:3], ["python3", "-I", "tools/smart-ci/stamp.py"])
        sentinel = [sys.executable, "-c", "print('the check ran'); raise SystemExit(3)"]
        command = [sys.executable, "-I", str(HERE / "stamp.py"), *words[3:at + 1], *sentinel]
        return subprocess.run(command, cwd=str(self.repo), capture_output=True, text=True, env={**os.environ, **env})

    def test_each_entry_skips_with_a_covering_stamp_and_runs_its_check_without_one(self) -> None:
        self.write("src/lib.rs", "pub fn f() { }\n")
        tree = stamp.worktree_tree(self.repo)
        self.git("add", "-A")
        pushed = self.git("commit-tree", tree, "-p", "HEAD", "-m", "pushed")
        hooks = self.config()
        for hook, (check, _) in WRAPPED.items():
            env = {"PRE_COMMIT_TO_REF": pushed} if stamp.CHECKS[check].stage == "pre-push" else {}
            with self.subTest(hook=hook, stamp="none"):
                ran = self.dry_run(hooks[hook]["entry"], env)
                self.assertEqual(ran.returncode, 3, ran.stdout + ran.stderr)
                self.assertIn(f"{check}: not covered by a gate stamp", ran.stdout)
                self.assertIn("the check ran", ran.stdout)
        # A full plan's stamp, with this machine's toolchain as the run recorded it.
        tools = stamp.toolchain(self.repo)
        if not all(tools.values()):
            self.skipTest(f"not every tool runs here: {tools}")
        stamp.write(self.repo, self.run_record(tree, full=True, toolchain=tools))
        for hook, (check, _) in WRAPPED.items():
            env = {"PRE_COMMIT_TO_REF": pushed} if stamp.CHECKS[check].stage == "pre-push" else {}
            with self.subTest(hook=hook, stamp="full"):
                skipped = self.dry_run(hooks[hook]["entry"], env)
                self.assertEqual(skipped.returncode, 0, skipped.stdout + skipped.stderr)
                self.assertEqual(len(skipped.stdout.splitlines()), 1, skipped.stdout)
                self.assertTrue(skipped.stdout.startswith(f"{check}: skipped, smart-ci passed on "), skipped.stdout)
                self.assertNotIn("the check ran", skipped.stdout)
            with self.subTest(hook=hook, stamp="switched off"):
                ran = self.dry_run(hooks[hook]["entry"], {**env, stamp.OFF_VAR: "1"})
                self.assertEqual(ran.returncode, 3)

    def test_a_passing_check_prints_one_line_and_a_broken_stamp_runs_the_check(self) -> None:
        passing = [sys.executable, "-c", "print('quiet output')"]
        shown = io.StringIO()
        with contextlib.redirect_stdout(shown):
            self.assertEqual(stamp.run_hook(self.repo, "clippy", passing, os.environ), 0)
        self.assertEqual(len(shown.getvalue().splitlines()), 1, shown.getvalue())
        stamp.stamps_dir(self.repo).mkdir(parents=True)
        stamp.stamp_path(self.repo, stamp.index_tree(self.repo)).write_text("{not json", encoding="utf-8")
        failing = [sys.executable, "-c", "print('loud output'); raise SystemExit(5)"]
        ran = subprocess.run([sys.executable, "-I", str(HERE / "stamp.py"), "hook", "clippy", "--", *failing],
                             cwd=str(self.repo), capture_output=True, text=True)
        self.assertEqual(ran.returncode, 5)
        self.assertIn("loud output", ran.stdout)
        # Whatever goes wrong in the decision, the check runs.
        with unittest.mock.patch.object(stamp, "decide", side_effect=RuntimeError("broken")), \
                contextlib.redirect_stdout(io.StringIO()) as said:
            self.assertEqual(stamp.run_hook(self.repo, "clippy", failing, os.environ), 5)
        self.assertIn("the stamps could not be read: broken", said.getvalue())

    def test_the_command_line(self) -> None:
        ran = subprocess.run([sys.executable, "-I", str(HERE / "stamp.py"), "check", "HEAD"], cwd=str(self.repo),
                             capture_output=True, text=True)
        self.assertEqual(ran.returncode, 1)
        self.assertIn("no stamp: smart-ci has not passed on tree", ran.stdout)
        for bad in ([], ["hook", "nonsense", "--", "x"], ["hook", "clippy", "x"], ["check"]):
            with self.subTest(args=bad):
                self.assertEqual(subprocess.run([sys.executable, "-I", str(HERE / "stamp.py"), *bad], cwd=str(self.repo),
                                                capture_output=True).returncode, 2)


if __name__ == "__main__":
    unittest.main()
