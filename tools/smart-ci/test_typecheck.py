#!/usr/bin/env python3
"""Tests of the TypeScript type check that `just _typecheck-ts`, `just ci`, smart-ci and the commit
hook run (`pnpm typecheck`), run with smart-ci's own (`test_smart_ci.py` loads them).

TypeScript 7.0.2's incremental check trusts a warm `.tsbuildinfo` after a change to a module that
declares globals: it rechecks the changed file, but neither the files that read its globals nor,
under `isolatedModules`, the files that import it. A cache warmed before such a change then passes
a broken tree, and keeps passing it until the broken files themselves change. That is how a
`preload/api.ts` change once passed `pnpm typecheck` with 13 errors in its dependents (2026-11-13).
`tsc -b --force` checks every project from scratch whatever its cache holds. So:

- every `tsc` that a package's `typecheck` script runs either builds with `--force` or checks a
  project that keeps no incremental cache, and every project of the package is one it checks;
- on the repository's own compiler options, a forced build reports the errors that such a change
  leaves in its importers and in its globals' readers, over the cache that its first build left.

Standard library only: `python3 tools/smart-ci/test_typecheck.py`.
"""

from __future__ import annotations

import json
import posixpath
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Dict, List, Optional, Set

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import smart_ci  # noqa: E402

# The flags that make `tsc` build mode check each project from scratch, ignoring its `.tsbuildinfo`.
BUILD_FLAGS = {"-b", "--build"}
FORCE_FLAGS = {"-f", "--force"}
PROJECT_FLAGS = {"-p", "--project"}
# What follows a trailing comma in JSONC: blanks and comments, then a closing bracket.
TRAILING_COMMA = re.compile(r"(?:\s|//[^\n]*|/\*(?:[^*]|\*(?!/))*\*/)*[}\]]")
# TypeScript's launcher, which `pnpm install` puts at the root, as the packages' scripts find it.
TSC = ROOT / "node_modules" / "typescript" / "bin" / "tsc"


def load_jsonc(path: Path) -> Dict[str, object]:
    """A tsconfig file: JSON with comments and trailing commas (TypeScript's JSONC)."""
    text = path.read_text(encoding="utf-8")
    out: List[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append(text[i : j + 1])
            i = j + 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            i = n if end < 0 else end
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = n if end < 0 else end + 2
        elif c == "," and TRAILING_COMMA.match(text, i + 1):
            i += 1
        else:
            out.append(c)
            i += 1
    data = json.loads("".join(out))
    assert isinstance(data, dict), path
    return data


def compiler_options(path: Path) -> Dict[str, object]:
    """A tsconfig's compiler options with those of the files it extends, as `tsc` merges them."""
    data = load_jsonc(path)
    merged: Dict[str, object] = {}
    extends = data.get("extends")
    for parent in [extends] if isinstance(extends, str) else (extends or []):
        merged.update(compiler_options((path.parent / str(parent)).resolve()))
    merged.update(data.get("compilerOptions") or {})
    return merged


def references(path: Path) -> List[Path]:
    """The projects a tsconfig references, each as the tsconfig file it names, recursively."""
    found: List[Path] = []
    for ref in load_jsonc(path).get("references") or []:
        target = (path.parent / ref["path"]).resolve()
        if target.is_dir():
            target = target / "tsconfig.json"
        found.append(target)
        found.extend(references(target))
    return found


def tsc_runs(script: str) -> List[List[str]]:
    """The `tsc` commands of a package script, each as its words: the script's `&&` chain."""
    return [words for words in (shlex.split(part) for part in script.split("&&")) if words and words[0] == "tsc"]


def ts_packages() -> List[Path]:
    """The pnpm workspace's packages (`pnpm-workspace.yaml`), each as its directory."""
    patterns = smart_ci.pnpm_workspace_patterns((ROOT / "pnpm-workspace.yaml").read_text(encoding="utf-8"))
    return sorted({manifest.parent for pattern in patterns for manifest in ROOT.glob(pattern + "/package.json")})


def build_operands(package: Path, words: List[str]) -> List[str]:
    """The projects a `tsc -b` command names: its tsconfig files and project directories."""
    return [w for w in words[1:] if not w.startswith("-") and (w.endswith(".json") or (package / w).is_dir())]


def typecheck_problems(package: Path, script: str) -> List[str]:
    """What is wrong with a package's `typecheck` script: a `tsc` that would trust an incremental
    cache (a build without `--force`, or `-p` on a composite or incremental project), or a project of
    the package (a `tsconfig*.json` that is not a solution's empty `files` list) that no `tsc` of it
    checks."""
    problems: List[str] = []
    runs = tsc_runs(script)
    if not runs:
        problems.append(f"runs no tsc: {script}")
    checked: Set[Path] = set()
    for words in runs:
        flags = set(words[1:])
        command = " ".join(words)
        if flags & BUILD_FLAGS:
            if not flags & FORCE_FLAGS:
                problems.append(f"`{command}` builds on the projects' warm .tsbuildinfo, which TypeScript "
                                "7.0.2 trusts after a change to a module that declares globals: add --force")
            for operand in build_operands(package, words) or ["tsconfig.json"]:
                solution = (package / (operand if operand.endswith(".json")
                                       else posixpath.join(operand, "tsconfig.json"))).resolve()
                checked.add(solution)
                checked.update(references(solution))
            continue
        project = next((words[i + 1] for i, w in enumerate(words[:-1]) if w in PROJECT_FLAGS), "tsconfig.json")
        path = (package / project).resolve()
        options = compiler_options(path)
        for option in ("composite", "incremental"):
            if options.get(option):
                problems.append(f"`{command}` checks {project}, whose `{option}` keeps a .tsbuildinfo that "
                                "TypeScript 7.0.2 trusts after a change to a module that declares globals: "
                                "check it with `tsc -b --force` instead")
        checked.add(path)
    for config in sorted(package.glob("tsconfig*.json")):
        if config.resolve() not in checked and load_jsonc(config).get("files") != []:
            problems.append(f"no tsc of it checks {config.name}")
    return problems


def typecheck_script(package: Path) -> Optional[str]:
    """A package's `typecheck` script, if it has one."""
    scripts = json.loads((package / "package.json").read_text(encoding="utf-8")).get("scripts", {})
    script = scripts.get("typecheck")
    return script if isinstance(script, str) else None


class TypecheckScripts(unittest.TestCase):
    """Each package's `typecheck` script reads no incremental cache and checks all its projects."""

    def test_load_jsonc(self) -> None:
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "tsconfig.json"
            path.write_text('{ // a\n "a": "x//y,}", /* b */ "b": [1, 2, /* c */ ], "c": { "d": true, // e\n },\n}\n',
                            encoding="utf-8")
            self.assertEqual(load_jsonc(path), {"a": "x//y,}", "b": [1, 2], "c": {"d": True}})

    def test_no_tsc_run_trusts_an_incremental_cache(self) -> None:
        packages = [p for p in ts_packages() if typecheck_script(p) is not None]
        self.assertGreaterEqual(len(packages), 3, packages)
        for package in packages:
            script = typecheck_script(package)
            assert script is not None
            with self.subTest(package=package.relative_to(ROOT).as_posix()):
                self.assertEqual(typecheck_problems(package, script), [])

    def test_the_old_scripts_fail_it(self) -> None:
        # The app's and perf's scripts before the fix checked their composite projects with
        # `tsc --noEmit -p`, on the projects' warm caches: how the 13 errors passed.
        app = ROOT / "apps" / "hyperion"
        perf = ROOT / "packages" / "perf"
        old_app = ("node ../../tools/node-floor/check.cjs && tsc --noEmit -p tsconfig.node.json && "
                   "tsc --noEmit -p tsconfig.web.json && tsc --noEmit -p tsconfig.worker.json")
        problems = typecheck_problems(app, old_app)
        self.assertEqual(len(problems), 3, problems)
        self.assertTrue(all("`composite`" in p for p in problems), problems)
        self.assertEqual(len(typecheck_problems(perf, "tsc --noEmit -p tsconfig.lib.json && "
                                                      "tsc --noEmit -p tsconfig.node.json")), 2)
        # A build that is not forced, and one that leaves a project out.
        self.assertIn("add --force", typecheck_problems(app, "tsc -b tsconfig.json")[0])
        self.assertEqual(typecheck_problems(app, "tsc -b --force tsconfig.json"), [])
        self.assertEqual(typecheck_problems(perf, "tsc -b --force tsconfig.lib.json"),
                         ["no tsc of it checks tsconfig.node.json"])


FIXTURE_API = """\
/** The fixture's report, as `preload/api.ts` exports its types and declares the window's API. */
export interface Report {
  readonly count: number;
}

declare global {
  interface Window {
    readonly fixture: Report;
  }
}
"""
FIXTURE_IMPORTER = """\
import type { Report } from "./api";

export const report: Report = { count: 1 };
"""
FIXTURE_READER = """\
export const count: number = window.fixture.count;
"""


@unittest.skipUnless(TSC.is_file() and shutil.which("node"),
                     "needs node and the TypeScript that `pnpm install` puts at the root")
class ForcedBuild(unittest.TestCase):
    """The app's own build command, over a warm cache that hides a broken tree."""

    def build(self, directory: Path) -> subprocess.CompletedProcess[str]:
        """The app's `typecheck` script's build, run on the fixture's solution (`tsconfig.json`)."""
        app = ROOT / "apps" / "hyperion"
        script = typecheck_script(app)
        assert script is not None
        builds = [words for words in tsc_runs(script) if set(words) & BUILD_FLAGS]
        self.assertEqual(len(builds), 1, script)
        words = builds[0]
        self.assertEqual(build_operands(app, words), ["tsconfig.json"], script)
        flags = [w for w in words[1:] if w != "tsconfig.json"]
        node = shutil.which("node")
        assert node is not None
        return subprocess.run(
            [node, str(TSC), *flags, "--pretty", "false", "tsconfig.json"],
            cwd=directory, capture_output=True, text=True, timeout=300, check=False,
        )

    def test_a_forced_build_reports_what_the_warm_cache_hid(self) -> None:
        with tempfile.TemporaryDirectory() as scratch:
            directory = Path(scratch)
            (directory / "tsconfig.json").write_text(
                json.dumps({"files": [], "references": [{"path": "./tsconfig.lib.json"}]}), encoding="utf-8"
            )
            # The repository's own options (`isolatedModules` and `verbatimModuleSyntax` among them),
            # composite, with its cache where the packages keep theirs.
            (directory / "tsconfig.lib.json").write_text(json.dumps({
                "extends": (ROOT / "tsconfig.base.json").as_posix(),
                "compilerOptions": {
                    "composite": True,
                    "tsBuildInfoFile": "node_modules/.cache/tsc/lib.tsbuildinfo",
                    "lib": ["es2023", "dom"],
                    "types": [],
                },
                "include": ["src"],
            }), encoding="utf-8")
            (directory / "src").mkdir()
            api = directory / "src" / "api.ts"
            api.write_text(FIXTURE_API, encoding="utf-8")
            (directory / "src" / "importer.ts").write_text(FIXTURE_IMPORTER, encoding="utf-8")
            (directory / "src" / "reader.ts").write_text(FIXTURE_READER, encoding="utf-8")

            first = self.build(directory)
            self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
            self.assertTrue((directory / "node_modules" / ".cache" / "tsc" / "lib.tsbuildinfo").is_file())

            # The exported type changes; its importer's literal and its global's reader break.
            api.write_text(FIXTURE_API.replace("readonly count:", "readonly crashCount:"), encoding="utf-8")
            second = self.build(directory)
            out = second.stdout + second.stderr
            self.assertNotEqual(second.returncode, 0, out)
            self.assertRegex(out, r"src/importer\.ts\(3,\d+\): error TS\d+")
            self.assertRegex(out, r"src/reader\.ts\(1,\d+\): error TS2339")


if __name__ == "__main__":
    unittest.main(verbosity=1)
