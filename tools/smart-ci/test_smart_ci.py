#!/usr/bin/env python3
"""Tests of `just smart-ci` (`smart_ci.py`), run by `just ci` (`_smart-ci-test`).

Two kinds. The rules, path by path, on a small workspace shaped like the real one (its crates,
their graph and features, a few TypeScript files that read Rust files), so that each rule of the
README's table is pinned without depending on today's tree. And smart-ci against the real
justfile and workspace: every step `just ci` runs is one smart-ci knows (`CI_STEPS`), each step
calls the recipes the table says with the arguments it says, and every recipe a plan runs exists
and takes the arguments the plan gives it. A step added to `ci`, or a command added to one of its
steps, fails `just ci` here until smart-ci knows it.

Standard library only: `python3 tools/smart-ci/test_smart_ci.py`.
"""

from __future__ import annotations

import contextlib
import functools
import io
import json
import os
import posixpath
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import unittest.mock
from pathlib import Path
from typing import Callable, Dict, Iterable, Iterator, List, Optional, Sequence, Set, Tuple

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import rust_graph  # noqa: E402
import smart_ci  # noqa: E402
from smart_ci import Crate, Plan, Workspace, make_plan  # noqa: E402
import stamp  # noqa: E402
import test_rust_graph  # noqa: E402
import test_stamp  # noqa: E402


def load_tests(loader: unittest.TestLoader, tests: unittest.TestSuite, pattern: Optional[str]) -> unittest.TestSuite:
    """These tests, the Rust graph's and the gate stamps', every class of them: `just ci` runs this file."""
    tests.addTests(loader.loadTestsFromModule(test_rust_graph))
    tests.addTests(loader.loadTestsFromModule(test_stamp))
    return tests


# just takes its settings from `JUST_*` variables as well as from its flags, and a nested `just`
# inherits them: `JUST_TIMESTAMP` puts the time before each recipe line it echoes, and `JUST_COLOR`,
# `JUST_QUIET`, `JUST_EXPLAIN`, `JUST_DRY_RUN`, `JUST_NO_DEPS` or `JUST_JUSTFILE` change what it
# prints, what it runs or which justfile it reads. Integration's runs set `JUST_TIMESTAMP=true` for
# every `just` that smart-ci runs, this suite's `_smart-ci-test` among them (perf-build Q4). The tests
# read just's own output (its dry runs, dump and evaluations) and run this checkout's recipes, so they
# run just with none of these, whoever calls them; the justfile reads none itself. Kept: where just may
# write its scripts and find `cygpath`, which are the machine's and change nothing it prints.
JUST_MACHINE_SETTINGS = ("JUST_TEMPDIR", "JUST_CYGPATH")


@contextlib.contextmanager
def without_just_settings() -> Iterator[None]:
    """The environment without just's settings (`JUST_*`, but the machine's), until the block ends."""
    with unittest.mock.patch.dict(os.environ):
        for name in [n for n in os.environ if n.upper().startswith("JUST_")
                     and n.upper() not in JUST_MACHINE_SETTINGS]:
            del os.environ[name]
        yield


_MODULE_ENVIRONMENT = contextlib.ExitStack()


def setUpModule() -> None:
    _MODULE_ENVIRONMENT.enter_context(without_just_settings())


def tearDownModule() -> None:
    _MODULE_ENVIRONMENT.close()


BASE = "hyperion-base"
TESTKIT = "hyperion-testkit"
SURFACE = "hyperion-surface"
SIM = "hyperion-sim"
PROTOCOL = "hyperion-protocol"
SERVER = "hyperion-server"
FIT = "hyperion-fit"


def dep(name: str, kind: Optional[str] = None, features: Sequence[str] = ()) -> Tuple:
    return (name, kind, list(features), True, False, name)


# The files of the small workspace, with the contents the scans read.
FILES: Dict[str, str] = {
    "Cargo.toml": "",
    "Cargo.lock": "",
    "justfile": "",
    "package.json": "",
    "pnpm-lock.yaml": "",
    "pnpm-workspace.yaml": "",
    "rust-toolchain.toml": "",
    "rustfmt.toml": "",
    "clippy.toml": "",
    "tsconfig.base.json": "",
    ".oxlintrc.json": "",
    ".prettierrc.json": "",
    ".prettierignore": "",
    ".gitignore": "",
    ".gitattributes": "",
    ".editorconfig": "",
    ".pre-commit-config.yaml": "",
    ".config/nextest.toml": "",
    ".cargo/config.toml": "",
    "README.md": "",
    "NOTICE": "",
    ".claude/skills/validate/SKILL.md": "",
    ".claude/skills/validate/scripts/select_checks.py": "",
    "docs/agent/plans/rendering-and-planets/09-surface-generator.md": "",
    "docs/frontend/ux-guidelines.md": "",
    "docs/measurements/descent-spike/2026-10-07-effect-low.json": "",
    "tools/gpu-replay/src/main.rs": "",
    "tools/gpu-replay/Cargo.toml": "",
    "tools/smart-ci/smart_ci.py": "",
    "tools/electron-node/node": "",
    "tools/node-floor/check.cjs": "",
    "tools/portable/flock": "",
    "crates/hyperion-base/Cargo.toml": "",
    "crates/hyperion-base/clippy.toml": "",
    "crates/hyperion-base/src/lib.rs": "",
    "crates/hyperion-base/src/math.rs": "",
    "crates/hyperion-base/src/version.rs": "pub const GENERATOR_VERSION: u32 = 21;",
    "crates/hyperion-testkit/Cargo.toml": "",
    "crates/hyperion-testkit/src/lib.rs": "",
    "crates/hyperion-surface/Cargo.toml": "",
    "crates/hyperion-surface/clippy.toml": "",
    "crates/hyperion-surface/src/lib.rs": "",
    "crates/hyperion-surface/src/field.rs": "#[cfg(test)]\nuse wasm_bindgen_test::wasm_bindgen_test as test;",
    "crates/hyperion-surface/src/wasm.rs": "use wasm_bindgen::prelude::wasm_bindgen;",
    "crates/hyperion-surface/src/tables/bowl_permanent_shadow.rs": "",
    "crates/hyperion-surface/tests/golden/bake.golden": "",
    "crates/hyperion-surface/tests/golden/wire/codes.golden": "",
    "crates/hyperion-surface/benches/test_planet.rs": "",
    "crates/hyperion-sim/Cargo.toml": "",
    "crates/hyperion-sim/clippy.toml": "",
    "crates/hyperion-sim/src/lib.rs": "",
    "crates/hyperion-sim/src/galaxy/disc.rs": "",
    "crates/hyperion-sim/src/sky/colour.rs": 'concat!(env!("CARGO_MANIFEST_DIR"), '
    '"/../../packages/protocol/fixtures/bake_wavelengths_nm.json")',
    "crates/hyperion-sim/src/tables/mge.rs": "",
    "crates/hyperion-sim/tests/golden/orbit/states.golden": "",
    "crates/hyperion-sim/tests/data/sse/README.md": "",
    "crates/hyperion-protocol/Cargo.toml": "",
    "crates/hyperion-protocol/src/lib.rs": "",
    "crates/hyperion-protocol/src/planetary/requests.rs": "",
    "crates/hyperion-server/Cargo.toml": "",
    "crates/hyperion-server/src/main.rs": "#[cfg(unix)]\nfn on_hangup() {}\n",
    "crates/hyperion-server/src/compute/density_map.rs": "//! `packages/protocol/fixtures/density_map_4x2.json`",
    "crates/hyperion-fit/Cargo.toml": "",
    "crates/hyperion-fit/clippy.toml": "",
    "crates/hyperion-fit/src/main.rs": "",
    "crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv": "",
    "crates/hyperion-fit/data/iprt_phase_a/README.md": "",
    "crates/hyperion-fit/manifests/mge.toml": "",
    "packages/protocol/package.json": "",
    "packages/protocol/README.md": "",
    "packages/protocol/src/index.ts": 'export * from "./hex";\nexport type { BodyId } from "./generated/BodyId";',
    "packages/protocol/src/hex.ts": "",
    "packages/protocol/src/hex.test.ts": 'import { hex } from "./hex";',
    "packages/protocol/src/generated/BodyId.ts": "",
    "packages/protocol/fixtures/density_map_4x2.json": "",
    "packages/protocol/fixtures/bake_wavelengths_nm.json": "",
    "apps/hyperion/package.json": "",
    "apps/hyperion/electron.vite.config.mts": "",
    "apps/hyperion/scripts/testRender.sh": "",
    "apps/hyperion/src/main/results.ts": "",
    "apps/hyperion/src/main/results.test.ts": 'const dir = join(__dirname, "../../../../docs/measurements/descent-spike");',
    "apps/hyperion/src/smoke/main.ts": "",
    "apps/hyperion/src/renderer/src/view/atmosphere/materials/water.json": "",
    "apps/hyperion/src/renderer/src/view/atmosphere/materials/phase/mars-dust.json": "",
    "apps/hyperion/src/tools/atmosphereData.ts": "",
    "apps/hyperion/src/tools/atmosphereData.test.ts": 'import { read } from "./atmosphereData";\n'
    'const csv = resolve(here, "../../../../crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv");',
    "apps/hyperion/src/renderer/src/lib/format.ts": 'import type { BodyId } from "@hyperion/protocol";\n'
    '/** Units as `docs/frontend/ux-guidelines.md` gives them. */\n'
    'const unit = "km"; // see "docs/frontend/ux-guidelines.md"',
    "apps/hyperion/src/renderer/src/lib/format.test.ts": 'import { format } from "./format";',
    "apps/hyperion/src/renderer/src/lib/orbit.test.ts": "import fixture from "
    '"../../../../../../crates/hyperion-sim/tests/golden/orbit/states.golden?raw";',
    "apps/hyperion/src/renderer/src/wasm/handleRequest.test.ts": "import versionSource from "
    '"../../../../../../crates/hyperion-base/src/version.rs?raw";',
    "apps/hyperion/src/renderer/src/view/terrain/heightBake.ts": "import init from "
    '"../../generated/surface/hyperion_surface";',
    "apps/hyperion/src/renderer/src/view/terrain/heightBake.test.ts": 'import { bake } from "./heightBake";',
    "apps/hyperion/src/renderer/src/view/terrain/heightWasm.test.ts": "import { initSync } from "
    '"../../generated/surface/hyperion_surface";\nimport golden from '
    '"../../../../../../../crates/hyperion-surface/tests/golden/bake.golden?raw";',
    "apps/hyperion/src/renderer/src/view/terrain/cube.ts": "",
    "apps/hyperion/src/renderer/src/view/engine/catalogue.ts": 'import { WIRE } from "../wireframe/submit";',
    "apps/hyperion/src/renderer/src/view/engine/device.ts": "",
    "apps/hyperion/src/renderer/src/view/wireframe/submit.ts": "",
    "apps/hyperion/src/renderer/src/view/wireframe/lines.ts": "",
    "apps/hyperion/src/renderer/src/view/shaders/sky.wgsl": "",
}


def workspace() -> Workspace:
    crates = {
        BASE: Crate(BASE, "crates/" + BASE, [dep(TESTKIT, "dev")]),
        TESTKIT: Crate(TESTKIT, "crates/" + TESTKIT, []),
        SURFACE: Crate(SURFACE, "crates/" + SURFACE, [dep(BASE), dep(TESTKIT, "dev")], {"testing": []}),
        SIM: Crate(SIM, "crates/" + SIM, [dep(BASE), dep(SURFACE), dep(TESTKIT, "dev")],
                   {"testing": ["hyperion-surface/testing"]}),
        PROTOCOL: Crate(PROTOCOL, "crates/" + PROTOCOL, []),
        SERVER: Crate(SERVER, "crates/" + SERVER, [dep(PROTOCOL), dep(SIM), dep(SURFACE), dep(TESTKIT, "dev")]),
        FIT: Crate(FIT, "crates/" + FIT, [dep(SIM, None, ["testing"]), dep(TESTKIT, "dev")]),
    }
    ws = Workspace(
        crates=crates,
        wasip1=[BASE, SURFACE, SIM, TESTKIT],
        browser=[BASE, SURFACE, TESTKIT],
        ts_packages={"apps/hyperion": ("hyperion", None), "packages/protocol": ("@hyperion/protocol", "./src/index.ts")},
        files=sorted(FILES),
        read=lambda path: FILES.get(path, ""),
        prettier_excludes=[".claude/**", "target/**"],
    )
    ws.catalogue_sources = smart_ci.catalogue_sources(ws)
    return ws


def plan(*paths: str, deleted: Iterable[str] = (), dependents: bool = False, all_tests: bool = False) -> Plan:
    gone = set(deleted)
    return make_plan(sorted((p, p in gone) for p in set(paths) | gone), workspace(), dependents, all_tests)


def calls(p: Plan) -> List[str]:
    return [step.recipe[0] for step in p.steps]


def args(p: Plan, recipe: str, which: int = 0) -> List[str]:
    """The arguments of the plan's `which`-th call of `recipe`."""
    found = [step.recipe[1:] for step in p.steps if step.recipe[0] == recipe]
    assert len(found) > which, f"{recipe} is not in the plan: {calls(p)}"
    return found[which]


def packages(p: Plan, recipe: str, which: int = 0) -> List[str]:
    """The crates of a cargo selection argument, `-p a -p b --features ...`."""
    words = args(p, recipe, which)[0].split()
    return [words[i + 1] for i, w in enumerate(words) if w == "-p"]


TS_RECIPES = {"_typecheck-ts", "_oxlint", "_vitest-run"}
# `just test-render`'s steps, which a plan runs beside `ci`'s.
RENDER_RECIPES = {recipe[0] for recipes in smart_ci.RENDER_PHASES.values() for recipe in recipes}
CARGO_RECIPES = {
    "_rustfmt-check", "_clippy-for", "_doc-check-for", "_cross-clippy-for", "_browser-clippy-for",
    "_relaxed-simd-refused", "_gen-protocol-check-for", "fit-check", "_fit-check-built", "_test-build-for",
    "_nextest-run", "_doctest-run", "_wasip1-build", "_wasip1-nextest",
    "_browser", "_wasm-run", "_wasm-preflight", "gen-surface",
}


class TheRules(unittest.TestCase):
    """Each rule of the table, on the small workspace."""

    def test_docs_only_runs_prettier_on_the_changed_files_alone(self) -> None:
        p = plan("docs/agent/plans/rendering-and-planets/09-surface-generator.md", "README.md",
                 "packages/protocol/README.md", "crates/hyperion-sim/README.md")
        self.assertEqual(calls(p), ["_prettier-check"])
        self.assertEqual(args(p, "_prettier-check")[0].split(), sorted([
            "README.md", "crates/hyperion-sim/README.md", "docs/agent/plans/rendering-and-planets/09-surface-generator.md",
            "packages/protocol/README.md"]))
        self.assertFalse(p.is_full)
        self.assertFalse(p.render)

    def test_a_file_named_only_in_comments_runs_no_test(self) -> None:
        self.assertEqual(calls(plan("docs/frontend/ux-guidelines.md")), ["_prettier-check"])
        self.assertEqual(smart_ci.code_of('a\n * `x/y.md`\n// "x/y.md"\nb = "u://v"; // "x/y.md"\n'),
                         'a\nb = "u://v";')

    def test_agent_configuration_runs_nothing_but_oxlint_on_its_scripts(self) -> None:
        self.assertEqual(calls(plan(".claude/skills/validate/SKILL.md")), [])
        self.assertEqual(calls(plan(".claude/skills/validate/scripts/select_checks.py")), [])
        self.assertEqual(calls(plan(".claude/hooks/check.ts")), ["gen-surface", "_oxlint"])

    def test_typescript_only_runs_the_typescript_side_and_no_cargo(self) -> None:
        p = plan("apps/hyperion/src/renderer/src/lib/format.ts")
        self.assertEqual(calls(p), ["gen-surface", "_prettier-check", "_typecheck-ts", "_oxlint", "_vitest-run"])
        # The vitest files that reach it through the import graph; with --all-tests, every one.
        self.assertEqual(args(p, "_vitest-run"), ["hyperion", "src/renderer/src/lib/format.test.ts"])
        self.assertEqual(args(plan("apps/hyperion/src/renderer/src/lib/format.ts", all_tests=True), "_vitest-run"),
                         ["", ""])
        self.assertEqual(args(p, "_prettier-check"), ["apps/hyperion/src/renderer/src/lib/format.ts"])
        self.assertFalse(set(calls(p)) & (CARGO_RECIPES - {"gen-surface"}))

    def test_no_typescript_change_runs_no_typescript_check(self) -> None:
        for path in ("crates/hyperion-sim/src/galaxy/disc.rs", "crates/hyperion-server/src/main.rs",
                     "crates/hyperion-fit/manifests/mge.toml", "docs/frontend/ux-guidelines.md",
                     "tools/gpu-replay/src/main.rs", ".config/nextest.toml"):
            with self.subTest(path=path):
                self.assertFalse(set(calls(plan(path))) & (TS_RECIPES | {"gen-surface"}), calls(plan(path)))

    def test_a_single_crate_runs_its_own_checks_and_tests(self) -> None:
        p = plan("crates/hyperion-server/src/main.rs")
        self.assertEqual(calls(p), ["_rustfmt-check", "_cross-clippy-for", "_clippy-for", "_doc-check-for",
                                    "_test-build-for", "_nextest-run", "_doctest-run"])
        self.assertEqual(args(p, "_rustfmt-check"), ["-p hyperion-server"])
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-server"])
        self.assertEqual(args(p, "_doc-check-for"), ["-p hyperion-server"])
        self.assertIn("rustdoc: hyperion-server", [step.title for step in p.steps])
        self.assertEqual(args(p, "_cross-clippy-for"), ["-p hyperion-server", ""])
        self.assertEqual(args(p, "_test-build-for"), ["-p hyperion-server"])
        for recipe in ("_nextest-run", "_doctest-run"):
            self.assertEqual(args(p, recipe), ["-p hyperion-server", ""])

    def test_a_sim_change_runs_the_sims_checks_wasip1_and_fit_check_but_no_typescript(self) -> None:
        p = plan("crates/hyperion-sim/src/galaxy/disc.rs")
        # No cross-Clippy: the sim is the same code on Linux, macOS and Windows (CROSS_CLIPPY_CRATES).
        # smart-ci's own tests: their guard of the justfile's `wasm_sky` reads the sim's sources.
        # The fitted tables' check after the test build, building hyperion-fit itself, which the plan
        # does not test, so that the test build makes no binary for it (perf-build Q6).
        self.assertEqual(calls(p), [
            "_wasm-preflight", "_rustfmt-check", "_smart-ci-test", "_clippy-for", "_doc-check-for",
            "_test-build-for", "fit-check", "_wasip1-build", "_nextest-run", "_doctest-run", "_wasm-run"])
        self.assertEqual(args(p, "_wasm-preflight"), ["wasip1"])
        # The features `--workspace` gives the sim: the fitting crate asks for `testing`, in its
        # normal dependencies, so `cargo doc --workspace` builds it too.
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-sim --features hyperion-sim/testing"])
        self.assertEqual(args(p, "_doc-check-for"), ["-p hyperion-sim --features hyperion-sim/testing"])
        # The testkit's source scans read the sim's sources: its native suite runs, not its doctests.
        self.assertEqual(packages(p, "_nextest-run"), [SIM, TESTKIT])
        self.assertEqual(packages(p, "_doctest-run"), [SIM])
        # On wasip1, the features of `ci`'s selection there, where nothing asks for `testing`.
        self.assertEqual(args(p, "_wasip1-build"), ["-p hyperion-sim", ""])
        self.assertEqual(args(p, "_wasm-run"), ["-p hyperion-sim", "", ""])
        self.assertIn("hyperion-fit, hyperion-server", " ".join(p.notes))
        self.assertIn("Clippy for the other platforms skips hyperion-sim", " ".join(p.notes))

    def test_rustdoc_runs_for_a_crates_code_after_its_clippy(self) -> None:
        # A change to code can break a link as surely as a change to a doc (an item moves, or loses
        # `pub`), so the trigger is the code: every `.rs` file of a crate, its manifest too.
        for path in ("crates/hyperion-sim/src/galaxy/disc.rs", "crates/hyperion-surface/src/field.rs",
                     "crates/hyperion-surface/benches/test_planet.rs", "crates/hyperion-protocol/Cargo.toml"):
            with self.subTest(path=path):
                p = plan(path)
                steps = [s for s in p.steps if s.recipe[0] == "_doc-check-for"]
                self.assertEqual(len(steps), 1, calls(p))
                self.assertEqual(steps[0].phase, "main")
                self.assertEqual(calls(p)[calls(p).index("_clippy-for") + 1], "_doc-check-for")
                self.assertEqual(steps[0].title, "rustdoc: " + ", ".join(packages(p, "_doc-check-for")))
                self.assertIn(path, steps[0].causes)
        # No hook runs rustdoc, so `--hook-checks`, which widens Clippy to the workspace's, leaves it
        # to the changed crates.
        hooked = make_plan([("crates/hyperion-sim/src/galaxy/disc.rs", False)], workspace(), False, hook_checks=True)
        self.assertEqual(args(hooked, "_clippy-for"), ["--workspace"])
        self.assertEqual(args(hooked, "_doc-check-for"), ["-p hyperion-sim --features hyperion-sim/testing"])
        # Data, goldens, other configuration and docs the crate does not include run no rustdoc.
        for path in ("crates/hyperion-sim/tests/golden/orbit/states.golden",
                     "crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv", "crates/hyperion-fit/manifests/mge.toml",
                     "crates/hyperion-sim/README.md", ".config/nextest.toml", "rustfmt.toml", "clippy.toml",
                     "crates/hyperion-base/clippy.toml", "packages/protocol/src/generated/BodyId.ts"):
            with self.subTest(path=path):
                self.assertNotIn("_doc-check-for", calls(plan(path)))

    def test_dependents_adds_the_reverse_dependents_clippy_and_tests(self) -> None:
        p = plan("crates/hyperion-sim/src/galaxy/disc.rs", dependents=True)
        self.assertEqual(packages(p, "_clippy-for"), [FIT, SERVER, SIM])
        # Their rustdoc too: a moved or renamed public item breaks their links.
        self.assertEqual(packages(p, "_doc-check-for"), [FIT, SERVER, SIM])
        self.assertEqual(packages(p, "_nextest-run"), [FIT, SERVER, SIM, TESTKIT])
        self.assertEqual(packages(p, "_rustfmt-check"), [SIM])
        self.assertFalse(any("Not checked" in note for note in p.notes))
        base = plan("crates/hyperion-base/src/math.rs", dependents=True)
        self.assertEqual(packages(base, "_clippy-for"), [BASE, FIT, SERVER, SIM, SURFACE])
        self.assertEqual(packages(base, "_doc-check-for"), [BASE, FIT, SERVER, SIM, SURFACE])
        # Without --dependents, the changed crate's alone, and the plan says what it leaves out.
        alone = plan("crates/hyperion-sim/src/galaxy/disc.rs")
        self.assertEqual(packages(alone, "_doc-check-for"), [SIM])
        self.assertIn("hyperion-fit, hyperion-server's Clippy and rustdoc", " ".join(alone.notes))

    def test_a_surface_change_rebuilds_the_module_and_runs_the_vitest_files_that_import_it(self) -> None:
        p = plan("crates/hyperion-surface/src/field.rs")
        self.assertEqual(calls(p), [
            "gen-surface", "_wasm-preflight", "_rustfmt-check", "_relaxed-simd-refused", "_vitest-run",
            "_clippy-for", "_doc-check-for", "_browser-clippy-for", "_test-build-for", "_wasip1-build", "_browser",
            "_nextest-run", "_doctest-run", "_wasm-run"])
        self.assertEqual(args(p, "_wasm-preflight"), ["wasip1", "browser"])
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-surface --features hyperion-surface/testing"])
        self.assertEqual(args(p, "_browser-clippy-for"), ["hyperion-surface"])
        # Its native lists from the test build's own selection, so that they build nothing again.
        surface = "-p hyperion-surface -p hyperion-testkit --features hyperion-surface/testing"
        self.assertEqual(args(p, "_browser", 0), ["prepare", "hyperion-surface", surface])
        # Under the lock, wasip1 and the browser target side by side, in one step.
        self.assertEqual(args(p, "_wasm-run"), ["-p hyperion-surface", "", "hyperion-surface"])
        self.assertEqual(args(p, "_vitest-run"), [
            "hyperion",
            "src/renderer/src/view/terrain/heightBake.test.ts src/renderer/src/view/terrain/heightWasm.test.ts"])
        # field.rs writes no export, so the module's .d.ts is unchanged: no tsc, no oxlint.
        self.assertNotIn("_typecheck-ts", calls(p))
        self.assertNotIn("fit-check", calls(p))

    def test_a_change_to_the_modules_exports_runs_tsc_and_oxlint(self) -> None:
        for p in (plan("crates/hyperion-surface/src/wasm.rs"), plan(deleted=["crates/hyperion-surface/src/field.rs"])):
            self.assertIn("_typecheck-ts", calls(p))
            self.assertIn("_oxlint", calls(p))

    def test_base_feeds_the_surface_module(self) -> None:
        p = plan("crates/hyperion-base/src/math.rs")
        self.assertIn("gen-surface", calls(p))
        self.assertIn("_relaxed-simd-refused", calls(p))
        self.assertEqual(args(p, "_wasm-preflight"), ["wasip1", "browser"])

    def test_the_protocol_runs_the_bindings_check_and_the_typescript_side(self) -> None:
        p = plan("crates/hyperion-protocol/src/planetary/requests.rs")
        self.assertEqual(calls(p), [
            "gen-surface", "_rustfmt-check", "_typecheck-ts", "_oxlint", "_vitest-run", "_clippy-for",
            "_doc-check-for", "_gen-protocol-check-for", "_test-build-for", "_nextest-run", "_doctest-run"])
        self.assertEqual(args(p, "_gen-protocol-check-for"), ["-p hyperion-protocol"])
        self.assertEqual(args(p, "_vitest-run"), ["", ""])

    def test_the_bindings_run_their_check_and_the_typescript_side(self) -> None:
        p = plan("packages/protocol/src/generated/BodyId.ts")
        self.assertEqual(calls(p), ["gen-surface", "_prettier-check", "_typecheck-ts", "_oxlint", "_vitest-run",
                                    "_gen-protocol-check-for"])
        self.assertEqual(args(p, "_gen-protocol-check-for"), ["-p hyperion-protocol"])

    def test_the_bindings_check_shares_the_test_builds_selection(self) -> None:
        p = plan("crates/hyperion-protocol/src/lib.rs", "crates/hyperion-sim/src/galaxy/disc.rs")
        self.assertEqual(args(p, "_gen-protocol-check-for"), args(p, "_test-build-for"))

    def test_a_shared_fixture_runs_both_sides(self) -> None:
        p = plan("packages/protocol/fixtures/density_map_4x2.json")
        self.assertIn("_typecheck-ts", calls(p))
        self.assertEqual(args(p, "_vitest-run"), ["", ""])
        self.assertEqual(packages(p, "_nextest-run"), [SERVER])
        self.assertEqual(packages(plan("packages/protocol/fixtures/bake_wavelengths_nm.json"), "_nextest-run"), [SIM])

    def test_a_shader_runs_test_render(self) -> None:
        p = plan("apps/hyperion/src/renderer/src/view/shaders/sky.wgsl")
        self.assertEqual(p.render, {"apps/hyperion/src/renderer/src/view/shaders/sky.wgsl"})
        # test-render's steps: the client's build beside the others, the harness last in the lock.
        self.assertEqual(calls(p), ["gen-surface", "_typecheck-ts", "_oxlint", "_test-render-build", "_test-render-run"])
        self.assertEqual([s.phase for s in p.steps if s.recipe[0].startswith("_test-render")], ["side", "locked"])
        # No vitest file imports the shader (as `?raw`), so none runs; tsc and oxlint do.
        self.assertNotIn("_vitest-run", calls(p))
        self.assertIn("_vitest-run", calls(plan("apps/hyperion/src/renderer/src/view/shaders/sky.wgsl", all_tests=True)))
        for path in ("apps/hyperion/src/renderer/src/view/engine/device.ts", "apps/hyperion/src/smoke/main.ts",
                     "apps/hyperion/scripts/testRender.sh", "apps/hyperion/electron.vite.config.mts",
                     # The shader catalogue imports it: a material's spec.
                     "apps/hyperion/src/renderer/src/view/wireframe/submit.ts"):
            with self.subTest(path=path):
                self.assertTrue(plan(path).render)
        self.assertFalse(plan("apps/hyperion/src/renderer/src/view/wireframe/lines.ts").render)
        self.assertFalse(plan("apps/hyperion/src/renderer/src/view/terrain/cube.ts").render)

    def test_the_whole_workspaces_inputs_run_the_full_ci(self) -> None:
        for path in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "justfile", "pnpm-lock.yaml",
                     ".cargo/config.toml", "tools/portable/flock"):
            with self.subTest(path=path):
                p = plan(path, "crates/hyperion-sim/src/lib.rs")
                self.assertTrue(p.is_full)
                self.assertEqual(p.steps, [])
                self.assertEqual(p.recipes()[0], "ci")
        self.assertEqual(plan("pnpm-lock.yaml").recipes(), ["ci", "_test-render-build", "_test-render-run"])

    def test_a_root_manifest_change_runs_the_full_ci(self) -> None:
        p = plan("Cargo.toml")
        self.assertTrue(p.is_full)
        self.assertEqual(p.recipes(), ["ci"])
        self.assertIn("Cargo.toml", " ".join(p.full))

    def test_a_path_no_rule_knows_runs_the_full_ci(self) -> None:
        for path in ("scripts/release.sh", ".github/workflows/ci.yml", "apps/other/index.ts", "crates/README.md",
                     "docs/agent/diagram.png", "tools/new-tool/run.sh", ".config/other.toml"):
            with self.subTest(path=path):
                self.assertTrue(plan(path).is_full, calls(plan(path)))

    def test_a_crates_data_runs_its_tests_and_the_vitest_files_that_read_it(self) -> None:
        p = plan("crates/hyperion-sim/tests/golden/orbit/states.golden")
        self.assertEqual(calls(p), ["gen-surface", "_wasm-preflight", "_vitest-run", "_test-build-for", "fit-check",
                                    "_wasip1-build", "_nextest-run", "_doctest-run", "_wasm-run"])
        self.assertEqual(args(p, "_vitest-run"), ["hyperion", "src/renderer/src/lib/orbit.test.ts"])
        fit_data = plan("crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv")
        self.assertEqual(packages(fit_data, "_nextest-run"), [FIT])
        self.assertEqual(args(fit_data, "_vitest-run"), ["hyperion", "src/tools/atmosphereData.test.ts"])
        # The plan tests hyperion-fit, so its test build makes the binary that the check runs, as `ci`'s does.
        self.assertIn("_fit-check-built", calls(fit_data))
        self.assertNotIn("fit-check", calls(fit_data))
        # A README in a data directory is data: a dataset's files are hashed through its PROVENANCE.toml.
        self.assertEqual(packages(plan("crates/hyperion-fit/data/iprt_phase_a/README.md"), "_nextest-run"), [FIT])
        self.assertEqual(packages(plan("crates/hyperion-sim/tests/data/sse/README.md"), "_nextest-run"), [SIM])
        manifest = plan("crates/hyperion-fit/manifests/mge.toml")
        self.assertEqual(calls(manifest), ["_test-build-for", "_fit-check-built", "_nextest-run", "_doctest-run"])

    def test_the_sims_tables_run_the_fitting_crates_tests(self) -> None:
        p = plan("crates/hyperion-sim/src/tables/mge.rs")
        self.assertEqual(packages(p, "_nextest-run"), [FIT, SIM, TESTKIT])
        self.assertEqual(packages(p, "_doctest-run"), [FIT, SIM])
        self.assertEqual(packages(p, "_clippy-for"), [SIM])

    def test_r08s_material_files_run_the_fitting_crates_tests_and_check(self) -> None:
        # `liquid_optics` reads them at run time and hashes their data (P14.T49.h): the fitting
        # crate's native suite, with its rerender test, and the staleness check its build makes.
        p = plan("apps/hyperion/src/renderer/src/view/atmosphere/materials/water.json")
        self.assertIn(FIT, packages(p, "_nextest-run"))
        self.assertIn("_fit-check-built", calls(p))
        self.assertNotIn("_wasip1-build", calls(p))
        # A file beside them that is not a material file reads nothing of the fitting crate.
        for regex, reader, _, _ in smart_ci.RUST_READERS:
            if reader == FIT:
                self.assertFalse(re.search(
                    regex, "apps/hyperion/src/renderer/src/view/atmosphere/materials/materials.ts"))

    def test_r08s_phase_tables_run_the_fitting_crates_tests_and_check(self) -> None:
        # `condensate_extinction` reads them at run time and hashes the fields it reads (P14.T49.e,
        # science-r08-nonspherical §6.4): the fitting crate's native suite, with its rerender test,
        # and the staleness check its build makes.
        p = plan("apps/hyperion/src/renderer/src/view/atmosphere/materials/phase/mars-dust.json")
        self.assertIn(FIT, packages(p, "_nextest-run"))
        self.assertIn("_fit-check-built", calls(p))
        self.assertNotIn("_wasip1-build", calls(p))
        # A file deeper down, or not a table, reads nothing of the fitting crate.
        for path in ("apps/hyperion/src/renderer/src/view/atmosphere/materials/phase/old/x.json",
                     "apps/hyperion/src/renderer/src/view/atmosphere/materials/phase/README.md"):
            self.assertFalse(any(re.search(regex, path) for regex, reader, _, _ in smart_ci.RUST_READERS
                                 if reader == FIT), path)

    def test_the_surface_crates_tables_run_the_fitting_crates_tests_and_check(self) -> None:
        p = plan("crates/hyperion-surface/src/tables/bowl_permanent_shadow.rs")
        self.assertIn(FIT, packages(p, "_nextest-run"))
        # The fitting crate's test build makes the binary the staleness check runs.
        self.assertIn("_fit-check-built", calls(p))

    def test_the_generator_version_runs_every_crates_tests(self) -> None:
        p = plan("crates/hyperion-base/src/version.rs")
        self.assertEqual(packages(p, "_nextest-run"), sorted(workspace().crates))
        self.assertIn("src/renderer/src/wasm/handleRequest.test.ts", args(p, "_vitest-run")[1])

    def test_a_crates_manifest_adds_its_dependents(self) -> None:
        p = plan("crates/hyperion-surface/Cargo.toml")
        self.assertEqual(packages(p, "_clippy-for"), [FIT, SERVER, SIM, SURFACE])
        self.assertEqual(packages(p, "_doc-check-for"), [FIT, SERVER, SIM, SURFACE])
        self.assertEqual(packages(p, "_rustfmt-check"), [SURFACE])
        self.assertEqual(packages(p, "_nextest-run"), [FIT, SERVER, SIM, SURFACE, TESTKIT])
        self.assertIn("gen-surface", calls(p))

    def test_clippy_configurations(self) -> None:
        root = plan("clippy.toml")
        self.assertEqual(packages(root, "_clippy-for"), [PROTOCOL, SERVER, TESTKIT])
        self.assertEqual(args(root, "_cross-clippy-for")[1], "gpu-replay")
        self.assertIn("gpu-replay-check", calls(root))
        self.assertEqual(packages(root, "_nextest-run"), [TESTKIT])
        own = plan("crates/hyperion-sim/clippy.toml")
        self.assertEqual(packages(own, "_clippy-for"), [SIM])
        self.assertNotIn("_rustfmt-check", calls(own))
        # Clippy's configuration is no input of rustdoc's.
        for p in (root, own):
            self.assertNotIn("_doc-check-for", calls(p))
        self.assertEqual(packages(plan("rustfmt.toml"), "_rustfmt-check"), sorted(workspace().crates))

    def test_tools(self) -> None:
        self.assertEqual(calls(plan("tools/gpu-replay/src/main.rs")), ["gpu-replay-check", "_cross-clippy-for"])
        self.assertEqual(args(plan("tools/gpu-replay/src/main.rs"), "_cross-clippy-for"), ["", "gpu-replay"])
        self.assertEqual(calls(plan("tools/smart-ci/smart_ci.py")), ["_smart-ci-test"])
        node = plan("tools/electron-node/node")
        self.assertEqual(args(node, "_browser", 0), [
            "prepare", "hyperion-base hyperion-surface hyperion-testkit",
            "-p hyperion-base -p hyperion-surface -p hyperion-testkit --features hyperion-surface/testing"])
        self.assertNotIn("_vitest-run", calls(node))
        # The Node floor's preflight runs before every vitest and tsc, and smart-ci runs it too.
        floor = plan("tools/node-floor/check.cjs")
        self.assertFalse(floor.is_full)
        self.assertEqual(calls(floor), ["gen-surface", "_prettier-check", "_typecheck-ts", "_oxlint", "_smart-ci-test",
                                        "_vitest-run"])
        self.assertEqual(args(floor, "_vitest-run"), ["", ""])

    def test_configuration_of_the_typescript_side_prettier_oxlint_and_nextest(self) -> None:
        root = plan("package.json")
        self.assertIn("_prettier-check-all", calls(root))
        self.assertEqual(args(root, "_vitest-run"), ["", ""])
        self.assertEqual(calls(plan(".oxlintrc.json")), ["gen-surface", "_prettier-check", "_oxlint"])
        self.assertEqual(calls(plan(".prettierrc.json")), ["_prettier-check-all"])
        self.assertEqual(calls(plan(".gitignore")), ["gen-surface", "_prettier-check-all", "_oxlint"])
        self.assertEqual(args(plan("tsconfig.base.json"), "_vitest-run"), ["", ""])
        nextest = plan(".config/nextest.toml")
        self.assertEqual(packages(nextest, "_nextest-run"), sorted(workspace().crates))
        self.assertNotIn("_clippy-for", calls(nextest))
        # Its default filter skips the export tests, which the bindings' check runs.
        self.assertEqual(args(nextest, "_gen-protocol-check-for"), args(nextest, "_test-build-for"))
        hooks = plan(".pre-commit-config.yaml")
        self.assertEqual(calls(hooks), ["_prettier-check"])
        self.assertTrue(hooks.notes)

    def test_gits_attributes_run_nothing(self) -> None:
        # How git diffs, merges and normalises files: no build, lint or test reads it, so not the full ci.
        self.assertNotIn(".gitattributes", smart_ci.FULL_CI_ROOT)
        attributes = plan(".gitattributes")
        self.assertFalse(attributes.is_full)
        self.assertEqual(calls(attributes), [])
        self.assertIn(".gitattributes", attributes.why)
        # Beside another change it adds nothing to that change's plan.
        self.assertEqual(calls(plan(".gitattributes", "README.md")), ["_prettier-check"])
        self.assertEqual(args(plan(".gitattributes", "README.md"), "_prettier-check"), ["README.md"])

    def test_measurements_run_the_clients_suite(self) -> None:
        p = plan("docs/measurements/descent-spike/2026-10-07-effect-low.json")
        self.assertEqual(args(p, "_vitest-run"), ["hyperion", ""])
        self.assertNotIn("_typecheck-ts", calls(p))

    def test_a_package_source_runs_the_vitest_files_of_every_package_that_reach_it(self) -> None:
        # hex.ts: its own test, and through the package's entry (`export * from "./hex"`), the client's
        # test of a file that imports `@hyperion/protocol`.
        p = plan("packages/protocol/src/hex.ts")
        self.assertEqual(args(p, "_vitest-run", 0), ["hyperion", "src/renderer/src/lib/format.test.ts"])
        self.assertEqual(args(p, "_vitest-run", 1), ["@hyperion/protocol", "src/hex.test.ts"])
        self.assertEqual(args(plan("packages/protocol/src/hex.ts", all_tests=True), "_vitest-run"), ["", ""])
        # Outside `src/`: the package's configuration, every package's suite.
        self.assertEqual(args(plan("packages/protocol/vitest.config.mts"), "_vitest-run"), ["", ""])

    def test_paths_that_are_not_one_word_widen_the_step_to_its_whole_scope(self) -> None:
        self.assertEqual(calls(plan("docs/agent/plans/a plan.md")), ["_prettier-check-all"])

    def test_deleted_files_go_to_no_file_list(self) -> None:
        p = plan("docs/frontend/ux-guidelines.md", deleted=["docs/agent/plans/old.md"])
        self.assertEqual(args(p, "_prettier-check"), ["docs/frontend/ux-guidelines.md"])
        self.assertEqual(calls(plan(deleted=["docs/agent/plans/old.md"])), [])

    def test_steps_keep_cis_order_and_phases(self) -> None:
        p = plan("crates/hyperion-surface/src/wasm.rs", "crates/hyperion-protocol/src/lib.rs",
                 "apps/hyperion/src/renderer/src/lib/format.ts", "tools/gpu-replay/src/main.rs",
                 "crates/hyperion-sim/src/galaxy/disc.rs", "tools/smart-ci/smart_ci.py", "README.md")
        order = list(smart_ci.PHASES)
        phases = [order.index(step.phase) for step in p.steps]
        self.assertEqual(phases, sorted(phases))
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "side"], [
            "_rustfmt-check", "_prettier-check", "_typecheck-ts", "_oxlint", "_relaxed-simd-refused",
            "gpu-replay-check", "_cross-clippy-for", "_smart-ci-test"])
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "unlocked"], ["_vitest-run"])
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "main"], [
            "_clippy-for", "_doc-check-for", "_browser-clippy-for", "_gen-protocol-check-for", "_test-build-for",
            "fit-check", "_wasip1-build", "_browser"])
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "locked"], [
            "_nextest-run", "_doctest-run", "_wasm-run"])

    def test_the_browser_lists_are_built_from_the_test_builds_selection(self) -> None:
        # Built from another selection, they would build their crates a second time, with other
        # features, before the lock.
        cases = [(["crates/hyperion-base/src/math.rs"], False), (["crates/hyperion-surface/src/field.rs"], True),
                 (["crates/hyperion-testkit/src/lib.rs", "crates/hyperion-sim/src/galaxy/disc.rs"], False)]
        for paths, dependents in cases:
            with self.subTest(paths=paths, dependents=dependents):
                p = plan(*paths, dependents=dependents)
                self.assertEqual(args(p, "_browser")[:1], ["prepare"])
                self.assertEqual(args(p, "_browser")[2], args(p, "_test-build-for")[0])

    def test_every_step_names_its_causes(self) -> None:
        p = plan("crates/hyperion-surface/src/field.rs", "README.md")
        for step in p.steps:
            self.assertTrue(step.causes, step.title)
        text = smart_ci.describe(p, [("crates/hyperion-surface/src/field.rs", False), ("README.md", False)], "test", False)
        self.assertIn("just _clippy-for", text)
        self.assertIn("because of crates/hyperion-surface/src/field.rs", text)

    def test_cross_clippy_runs_for_the_crates_whose_code_differs_between_the_platforms(self) -> None:
        # decision-test-coverage.md L4: the server (CROSS_CLIPPY_CRATES) and tools/gpu-replay alone.
        self.assertEqual(args(plan("crates/hyperion-server/src/main.rs"), "_cross-clippy-for"), ["-p hyperion-server", ""])
        for path in ("crates/hyperion-sim/src/galaxy/disc.rs", "crates/hyperion-base/src/math.rs",
                     "crates/hyperion-surface/src/field.rs", "crates/hyperion-protocol/src/lib.rs"):
            with self.subTest(path=path):
                self.assertNotIn("_cross-clippy-for", calls(plan(path)))
        both = plan("crates/hyperion-base/src/math.rs", "crates/hyperion-server/src/main.rs", dependents=True)
        self.assertEqual(packages(both, "_cross-clippy-for"), [SERVER])

    def test_a_crate_the_guard_flags_gets_cross_clippy_and_smart_cis_tests(self) -> None:
        # A crate outside CROSS_CLIPPY_CRATES with code that differs between the platforms: its
        # cross-Clippy runs, and smart-ci's own tests, which fail until it is listed, in the same commit.
        ws = workspace()
        files = dict(FILES)
        files["crates/hyperion-base/src/math.rs"] = "#[cfg(unix)]\nfn only_unix() {}\n"
        ws.read = lambda path: files.get(path, "")
        p = make_plan([("crates/hyperion-base/src/math.rs", False)], ws, False)
        self.assertEqual(packages(p, "_cross-clippy-for"), [BASE])
        self.assertIn("_smart-ci-test", calls(p))
        self.assertIn("CROSS_CLIPPY_CRATES", " ".join(p.notes))
        self.assertIn("crates/hyperion-base/src/math.rs: cfg(unix)", " ".join(p.notes))
        # A listed crate the guard finds nothing in runs smart-ci's tests too, so that the list stays honest.
        files["crates/hyperion-server/src/main.rs"] = "fn main() {}\n"
        p = make_plan([("crates/hyperion-server/src/main.rs", False)], ws, False)
        self.assertIn("_smart-ci-test", calls(p))
        self.assertIn("drop it", " ".join(p.notes))
        # A manifest's OS dependency counts the same.
        ws = workspace()
        ws.os_manifest = {SIM: ["a dependency on libc"]}
        p = make_plan([("crates/hyperion-sim/src/galaxy/disc.rs", False)], ws, False)
        self.assertEqual(packages(p, "_cross-clippy-for"), [SIM])

    def test_features_are_those_the_workspace_gives(self) -> None:
        ws = workspace()
        features = ws.unified_features(ws.crates)
        self.assertEqual(features[SIM], {"testing"})
        self.assertEqual(features[SURFACE], {"testing"})
        self.assertEqual(ws.unified_features([SIM])[SURFACE], set())
        self.assertEqual(ws.cargo_packages([SURFACE, SERVER]), "-p hyperion-server -p hyperion-surface "
                                                             "--features hyperion-surface/testing")
        self.assertEqual(ws.cargo_packages([SURFACE], universe=ws.wasip1), "-p hyperion-surface")
        # rustdoc's: `cargo doc --workspace` builds no test, so a dev-dependency enables nothing. The
        # fitting crate's normal dependency still gives the sim `testing`, and the sim the surface's.
        self.assertEqual(ws.unified_features(ws.crates, dev=False)[SIM], {"testing"})
        self.assertEqual(ws.cargo_packages([SURFACE, SERVER], dev=False), "-p hyperion-server -p hyperion-surface "
                                                                        "--features hyperion-surface/testing")
        # A feature asked for only by a dev-dependency: Clippy's build, with the tests, has it;
        # rustdoc's does not.
        ws.crates[FIT] = Crate(FIT, "crates/" + FIT, [dep(SIM), dep(TESTKIT, "dev")])
        ws.crates[SERVER].deps.append(dep(SIM, "dev", ["testing"]))
        self.assertEqual(ws.unified_features(ws.crates)[SIM], {"testing"})
        self.assertEqual(ws.unified_features(ws.crates, dev=False)[SIM], set())
        self.assertEqual(ws.unified_features(ws.crates, dev=False)[SURFACE], set())
        self.assertEqual(ws.cargo_packages([SIM]), "-p hyperion-sim --features hyperion-sim/testing")
        self.assertEqual(ws.cargo_packages([SIM], dev=False), "-p hyperion-sim")
        p = make_plan([("crates/hyperion-sim/src/galaxy/disc.rs", False)], ws, False)
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-sim --features hyperion-sim/testing"])
        self.assertEqual(args(p, "_doc-check-for"), ["-p hyperion-sim"])
        ws = workspace()
        self.assertEqual(ws.surface_module_crates(), {BASE, SURFACE})
        self.assertEqual(ws.reverse_dependents([BASE]), {SURFACE, SIM, SERVER, FIT})
        self.assertEqual(ws.reverse_dependents([TESTKIT]), {BASE, SURFACE, SIM, SERVER, FIT})


class GraphMode(unittest.TestCase):
    """The plan with the code's graph: the tests that reach a change, by filter, in any crate."""

    SOURCES = {
        "crates/hyperion-base/src/lib.rs": (
            "pub fn unit() -> u32 { 1 }\npub fn other() -> u32 { 2 }\n"
            "/// ```\n/// assert_eq!(hyperion_base::unit(), 1);\n/// ```\npub fn documented() {}\n"
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn unit_is_one() { assert_eq!(super::unit(), 1); }\n"
            "    #[test]\n    fn other_is_two() { assert_eq!(super::other(), 2); }\n}\n"),
        "crates/hyperion-sim/src/lib.rs": (
            "pub mod sky;\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn plain() {}\n}\n"),
        "crates/hyperion-sim/src/sky/mod.rs": (
            "pub fn glow() -> u32 { hyperion_base::unit() * 3 }\n"
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn glows() { assert_eq!(super::glow(), 3); }\n}\n"),
    }

    def workspace(self, now: Dict[str, str]) -> Workspace:
        ws = workspace()
        files = dict(FILES)
        files.update(now)
        ws.files = sorted(files)
        ws.read = lambda path: files.get(path, "")
        ws.rust = {
            BASE: rust_graph.CrateInfo(BASE, "crates/" + BASE, [], [TESTKIT], [
                rust_graph.Target("lib", "hyperion_base", "crates/hyperion-base/src/lib.rs", True, True)]),
            TESTKIT: rust_graph.CrateInfo(TESTKIT, "crates/" + TESTKIT, [], [], [
                rust_graph.Target("lib", "hyperion_testkit", "crates/hyperion-testkit/src/lib.rs", True, True)]),
            SURFACE: rust_graph.CrateInfo(SURFACE, "crates/" + SURFACE, [BASE], [TESTKIT], []),
            SIM: rust_graph.CrateInfo(SIM, "crates/" + SIM, [BASE, SURFACE], [TESTKIT], [
                rust_graph.Target("lib", "hyperion_sim", "crates/hyperion-sim/src/lib.rs", True, True)]),
            PROTOCOL: rust_graph.CrateInfo(PROTOCOL, "crates/" + PROTOCOL, [], [], []),
            SERVER: rust_graph.CrateInfo(SERVER, "crates/" + SERVER, [PROTOCOL, SIM, SURFACE], [TESTKIT], []),
            FIT: rust_graph.CrateInfo(FIT, "crates/" + FIT, [SIM], [TESTKIT], []),
        }
        ws.read_bytes = lambda path: files[path].encode() if path in files else None
        ws.read_base = lambda path: self.SOURCES[path].encode() if path in self.SOURCES else None
        return ws

    def plan_change(self, path: str, old: str, new: str, all_tests: bool = False) -> Plan:
        now = dict(self.SOURCES)
        now[path] = now[path].replace(old, new)
        return make_plan([(path, False)], self.workspace(now), False, all_tests)

    def test_a_change_runs_the_tests_that_reach_it_in_every_crate(self) -> None:
        p = self.plan_change("crates/hyperion-base/src/lib.rs", "{ 1 }", "{ 4 }")
        self.assertEqual(packages(p, "_nextest-run"), [BASE, SIM, TESTKIT])
        self.assertEqual(args(p, "_nextest-run")[1], "(binary_id(=hyperion-base) & (test(=tests::unit_is_one))) | "
                                                     "(binary_id(=hyperion-sim) & (test(/^sky::tests::/))) | package(hyperion-testkit)")
        self.assertEqual(args(p, "_doctest-run"), ["-p hyperion-base", "crates/hyperion-base/src/lib.rs"])
        # wasip1: built and its modules compiled outside the lock, run under it, with the same filter.
        self.assertEqual(args(p, "_wasip1-build")[1], args(p, "_wasm-run")[1])
        self.assertIn("binary_id(=hyperion-sim) & (test(/^sky::tests::/))", args(p, "_wasip1-build")[1])
        details = "\n".join(p.details)
        self.assertIn("hyperion-sim: 1 of 2 tests", details)
        self.assertIn("hyperion-base: 1 of 2 tests", details)
        self.assertIn("built in", details)
        # rustfmt and Clippy stay the changed crate's.
        self.assertEqual(packages(p, "_clippy-for"), [BASE])

    def test_a_crates_data_runs_its_tests_and_other_crates_tests_that_name_its_directory(self) -> None:
        now = dict(self.SOURCES)
        reader = "crates/hyperion-sim/src/lib.rs"
        now[reader] = now[reader].replace("fn plain() {}", "fn plain() {}\n    #[test]\n    fn reads() { let _ = "
                                          "std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"../hyperion-base/data\"); }")
        now["crates/hyperion-base/data/table.bin"] = ""
        p = make_plan([("crates/hyperion-base/data/table.bin", False)], self.workspace(now), False)
        self.assertEqual(packages(p, "_nextest-run"), [BASE, SIM])
        self.assertIn("binary_id(=hyperion-sim) & (test(=tests::reads))", args(p, "_nextest-run")[1])
        # With --all-tests, its crate's alone, as before the graph.
        q = make_plan([("crates/hyperion-base/data/table.bin", False)], self.workspace(now), False, all_tests=True)
        self.assertEqual(packages(q, "_nextest-run"), [BASE])

    def test_markdown_that_a_crate_root_includes_as_its_documentation_is_its_data(self) -> None:
        now = dict(self.SOURCES)
        root = "crates/hyperion-base/src/lib.rs"
        now[root] = "#![doc = include_str!(\"../README.md\")]\n" + now[root]
        now["crates/hyperion-base/README.md"] = "# Base\n"
        now["crates/hyperion-sim/README.md"] = "# Sim\n"
        ws = self.workspace(now)
        included = make_plan([("crates/hyperion-base/README.md", False)], ws, False)
        self.assertEqual(packages(included, "_doctest-run"), [BASE])
        # As the crate's docs, its links are rustdoc's: the crate's rustdoc runs.
        self.assertEqual(args(included, "_doc-check-for"), ["-p hyperion-base"])
        not_included = make_plan([("crates/hyperion-sim/README.md", False)], ws, False)
        self.assertNotIn("_nextest-run", calls(not_included))
        self.assertNotIn("_doc-check-for", calls(not_included))

    def test_no_test_reaches_a_change_that_no_test_uses(self) -> None:
        p = self.plan_change("crates/hyperion-base/src/lib.rs", "pub fn documented() {}", "pub fn documented() { }\npub fn new() {}")
        # Only the testkit's suite, whose clippy_bans reads base's sources; no doctest names either.
        self.assertEqual(packages(p, "_nextest-run"), [TESTKIT])
        self.assertEqual(args(p, "_nextest-run")[1], "")
        self.assertNotIn("_doctest-run", calls(p))
        self.assertIn("_clippy-for", calls(p))

    def test_a_build_script_or_a_proc_macro_crate_runs_every_test(self) -> None:
        now = dict(self.SOURCES)
        now["crates/hyperion-base/src/lib.rs"] = now["crates/hyperion-base/src/lib.rs"].replace("{ 1 }", "{ 4 }")
        ws = self.workspace(now)
        ws.build_scripts = {BASE}
        p = make_plan([("crates/hyperion-base/src/lib.rs", False)], ws, False)
        self.assertEqual(args(p, "_nextest-run")[1], "")  # every test of base, and none of its dependents'
        self.assertEqual(packages(p, "_nextest-run"), [BASE, TESTKIT])
        ws = self.workspace(now)
        ws.proc_macros = {BASE}
        p = make_plan([("crates/hyperion-base/src/lib.rs", False)], ws, False)
        self.assertEqual(args(p, "_nextest-run")[1], "")  # every test of base and its dependents
        self.assertIn(SIM, packages(p, "_nextest-run"))
        self.assertIn(SIM, packages(p, "_clippy-for"))

    def test_all_tests_restores_the_crates_whole_suites(self) -> None:
        p = self.plan_change("crates/hyperion-base/src/lib.rs", "{ 1 }", "{ 4 }", all_tests=True)
        self.assertEqual(args(p, "_nextest-run")[1], "")
        self.assertEqual(packages(p, "_nextest-run"), [BASE, TESTKIT])
        self.assertEqual(args(p, "_doctest-run"), ["-p hyperion-base", ""])
        self.assertEqual(p.details, [])

    # A source walker: lists a directory and keeps its `.rs` files.
    WALKER = ("        fn rust_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {\n"
              "            for entry in std::fs::read_dir(dir).unwrap() {\n"
              "                let path = entry.unwrap().path();\n"
              "                if path.extension().is_some_and(|e| e == \"rs\") { found.push(path); }\n"
              "            }\n        }\n")

    def walkers_workspace(self, changed: str, old: str, new: str) -> Workspace:
        """GraphMode's workspace with a walker's test in the sim and one in the surface crate's library,
        compiled out of the browser build as `tags.rs`'s is, and `changed` changed."""
        now = dict(self.SOURCES)
        now["crates/hyperion-sim/src/lib.rs"] = now["crates/hyperion-sim/src/lib.rs"].replace(
            "pub mod sky;", "pub mod sky;\npub mod substance;")
        now["crates/hyperion-sim/src/substance.rs"] = (
            "#[cfg(test)]\nmod tests {\n" + self.WALKER.replace("\n        ", "\n    ")[4:]
            + "    #[test]\n    fn nothing_reads_it_yet() {\n        let mut found = Vec::new();\n"
            "        rust_files(std::path::Path::new(\"src\"), &mut found);\n    }\n}\n")
        now["crates/hyperion-surface/src/lib.rs"] = "pub mod tags;\n"
        now["crates/hyperion-surface/src/tags.rs"] = (
            "pub fn tag() -> u32 { 5 }\n#[cfg(test)]\nmod tests {\n"
            "    #[test]\n    fn tag_is_five() { assert_eq!(super::tag(), 5); }\n"
            "    #[cfg(not(all(target_arch = \"wasm32\", target_os = \"unknown\")))]\n    mod native_only {\n"
            + self.WALKER + "        #[test]\n        fn call_sites_keep_the_rule() {\n"
            "            let mut found = Vec::new();\n"
            "            rust_files(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).parent().unwrap(), &mut found);\n"
            "        }\n    }\n}\n")
        then = {**FILES, **now}
        now[changed] = then.get(changed, "").replace(old, new)
        ws = self.workspace(now)
        ws.read_base = lambda path: then[path].encode() if path in then else None
        ws.rust[SURFACE] = rust_graph.CrateInfo(SURFACE, "crates/" + SURFACE, [BASE], [TESTKIT], [
            rust_graph.Target("lib", "hyperion_surface", "crates/hyperion-surface/src/lib.rs", True, True)])
        return ws

    def test_a_rust_change_runs_the_tests_that_read_the_rust_sources_natively(self) -> None:
        sim = "crates/hyperion-sim/src/sky/mod.rs"
        ws = self.walkers_workspace(sim, "* 3", "* 4")
        p = make_plan([(sim, False)], ws, False, explain=True)
        # Natively: the change's own test, the sim's walker's, and the surface crate's, which reads the
        # sim's sources though the surface crate does not depend on it; the testkit's, as RUST_READERS has it.
        self.assertEqual(packages(p, "_nextest-run"), [SIM, SURFACE, TESTKIT])
        self.assertEqual(packages(p, "_test-build-for"), [SIM, SURFACE, TESTKIT])
        self.assertEqual(args(p, "_nextest-run")[1],
                         "(binary_id(=hyperion-sim) & (test(/^sky::tests::/) | test(/^substance::tests::/))) | "
                         "(binary_id(=hyperion-surface) & (test(/^tags::tests::native_only::/))) | "
                         "package(hyperion-testkit)")
        # Not on wasip1, nor in the browser: the surface crate's walker's test is its only pick.
        for recipe in ("_wasip1-build", "_wasm-run"):
            self.assertEqual(args(p, recipe)[1], "(binary_id(=hyperion-sim) & (test(/^sky::tests::/)))", recipe)
        self.assertEqual(args(p, "_wasm-run")[2], "")
        self.assertNotIn("_browser", calls(p))
        self.assertEqual(packages(p, "_wasip1-build"), [SIM])
        # The native step and the test build name the reason first; the counts include the readers.
        native = next(s for s in p.steps if s.recipe[0] == "_nextest-run")
        self.assertEqual(native.causes[0], smart_ci.SOURCE_READERS)
        self.assertIn(smart_ci.SOURCE_READERS, next(s for s in p.steps if s.recipe[0] == "_test-build-for").causes)
        self.assertEqual(native.title, "nextest: hyperion-sim (2 of 3 tests), hyperion-surface (1 of 2 tests), "
                                       "hyperion-testkit")
        wasm = next(s for s in p.steps if s.recipe[0] == "_wasip1-build")
        self.assertIn("hyperion-sim (1 of 3 tests)", wasm.title)
        self.assertNotIn(smart_ci.SOURCE_READERS, wasm.causes)
        # --explain: the chain from the test to the walker, and the rule.
        details = "\n".join(p.details)
        self.assertIn("hyperion-surface: 1 test(s) that read the Rust sources, natively", details)
        self.assertIn("tags::tests::native_only::call_sites_keep_the_rule: crates/hyperion-surface/src/tags.rs "
                      "(fn tests:::native_only:call_sites_keep_the_rule) -> crates/hyperion-surface/src/tags.rs "
                      "(fn tests:::native_only:rust_files), a source walker", details)

    def test_a_rust_change_that_the_graph_does_not_read_still_runs_them(self) -> None:
        # A build script's crate runs its every test without the graph; the walkers' tests that may
        # list it run all the same: the surface crate's, whose directory is computed, and not the
        # sim's, which lists the sim's `src` alone.
        base = "crates/hyperion-base/src/lib.rs"
        ws = self.walkers_workspace(base, "{ 1 }", "{ 4 }")
        ws.build_scripts = {BASE}
        p = make_plan([(base, False)], ws, False)
        self.assertEqual(args(p, "_nextest-run")[1],
                         "package(hyperion-base) | "
                         "(binary_id(=hyperion-surface) & (test(/^tags::tests::native_only::/))) | "
                         "package(hyperion-testkit)")
        # With --all-tests, no graph: the changed crate's tests alone, as before.
        q = make_plan([(base, False)], ws, False, all_tests=True)
        self.assertEqual(packages(q, "_nextest-run"), [BASE, TESTKIT])
        self.assertEqual(q.details, [])
        # With --dependents, its dependents' every test, as without the graph, which chose none of them.
        d = make_plan([(base, False)], ws, True)
        self.assertEqual(packages(d, "_nextest-run"), [BASE, FIT, SERVER, SIM, SURFACE, TESTKIT])
        self.assertEqual(args(d, "_nextest-run")[1], "")

    def test_a_reader_that_the_change_reaches_through_code_keeps_its_wasip1_run(self) -> None:
        substance = "crates/hyperion-sim/src/substance.rs"
        p = make_plan([(substance, False)], self.walkers_workspace(substance, "found.push(path)",
                                                                  "found.insert(0, path)"), False)
        self.assertEqual(args(p, "_wasip1-build")[1], "(binary_id(=hyperion-sim) & (test(/^substance::tests::/)))")
        self.assertIn("binary_id(=hyperion-sim) & (test(/^substance::tests::/))", args(p, "_nextest-run")[1])
        self.assertIn("binary_id(=hyperion-surface) & (test(/^tags::tests::native_only::/))", args(p, "_nextest-run")[1])

    def test_a_rust_change_runs_the_readers_whose_walker_may_list_its_crate(self) -> None:
        # The sim's reader lists the sim's `src`; the surface crate's lists every crate, from a
        # directory it computes. A change to the surface crate runs the latter alone, and builds no
        # sim test.
        surface = "crates/hyperion-surface/src/tags.rs"
        p = make_plan([(surface, False)], self.walkers_workspace(surface, "{ 5 }", "{ 6 }"), False, explain=True)
        self.assertEqual(packages(p, "_nextest-run"), [SURFACE, TESTKIT])
        self.assertEqual(packages(p, "_test-build-for"), [SURFACE, TESTKIT])
        details = "\n".join(p.details)
        self.assertIn("(fn tests:::native_only:rust_files), a source walker: it lists a directory and keeps its `.rs` "
                      "files; reads every crate, since its directory is computed: `parent()` in "
                      "crates/hyperion-surface/src/tags.rs", details)
        # A comment in a crate no reader's code names: the computed one alone, and no sim test.
        protocol = "crates/hyperion-protocol/src/lib.rs"
        ws = self.walkers_workspace(protocol, "", "// A comment.\n")
        p = make_plan([(protocol, False)], ws, False)
        self.assertNotIn(SIM, packages(p, "_nextest-run"))
        self.assertNotIn(SIM, packages(p, "_test-build-for"))
        self.assertIn("binary_id(=hyperion-surface) & (test(/^tags::tests::native_only::/))", args(p, "_nextest-run")[1])
        # A change to the sim runs both, the sim's for the crate it lists.
        sim = "crates/hyperion-sim/src/sky/mod.rs"
        p = make_plan([(sim, False)], self.walkers_workspace(sim, "* 3", "* 4"), False, explain=True)
        self.assertIn("(fn tests::rust_files), a source walker: it lists a directory and keeps its `.rs` files; reads "
                      "hyperion-sim\n", "\n".join(p.details) + "\n")

    # Two slow tests of base's, `#[ignore]`d for two reasons.
    SLOW = ("    #[test]\n    #[ignore = \"slow: the long pass\"]\n    fn unit_slowly() { assert_eq!(super::unit(), 1); }\n"
            "    #[test]\n    #[ignore = \"cost: a large table\"]\n    fn other_slowly() { assert_eq!(super::other(), 2); }\n")

    def slow_plan(self, path: str, old: str, new: str, all_tests: bool = False, explain: bool = False,
                  kit: bool = False) -> Plan:
        """A plan for `path` changed, with base's slow tests beside its fast ones, at the base too;
        with `kit`, a slow test of the testkit's, every native test of which a base change runs."""
        then = dict(self.SOURCES)
        base = "crates/hyperion-base/src/lib.rs"
        then[base] = then[base].replace("    #[test]\n    fn other_is_two()", self.SLOW + "    #[test]\n    fn other_is_two()")
        if kit:
            then["crates/hyperion-testkit/src/lib.rs"] = (
                "#[cfg(test)]\nmod tests {\n    #[test]\n    #[ignore = \"slow: a chi-square\"]\n    fn chi_square() {}\n}\n")
        now = dict(then)
        if path in now:
            now[path] = now[path].replace(old, new)
        else:
            now[path] = new
        ws = self.workspace(now)
        ws.read_base = lambda p: then[p].encode() if p in then else None
        return make_plan([(path, False)], ws, False, all_tests, explain)

    def test_the_slow_tests_a_change_reaches_are_listed_and_left_out_of_the_run(self) -> None:
        base = "crates/hyperion-base/src/lib.rs"
        p = self.slow_plan(base, "{ 1 }", "{ 4 }")
        details = "\n".join(p.details)
        # The counts are the run's: base's fast tests, not its ignored ones, in the crate's line and
        # the unit's alike.
        self.assertIn("  hyperion-base: 1 of 2 tests\n    hyperion-base: 1 test(s): tests 1\n", details)
        self.assertIn("\n  slow tests reached (run by name): hyperion-base: 1: tests 1", details)
        self.assertNotIn("other_slowly", details)  # not reached
        native = next(s for s in p.suites if s.kind == "native")
        self.assertEqual(native.tests[BASE], {"tests::unit_is_one"})
        # Advisory: no step runs an ignored test.
        self.assertFalse(any("--run-ignored" in " ".join(step.recipe) for step in p.steps))
        # With --explain, each with its chain.
        explained = "\n".join(self.slow_plan(base, "{ 1 }", "{ 4 }", explain=True).details)
        self.assertIn("      hyperion-base tests::unit_slowly: crates/hyperion-base/src/lib.rs (fn tests::unit_slowly) -> "
                      "crates/hyperion-base/src/lib.rs (fn unit)", explained)
        # None reached: no line; and none with --all-tests, which has no graph.
        quiet = self.slow_plan("crates/hyperion-sim/src/sky/mod.rs", "* 3", "* 4")
        self.assertIn("hyperion-sim: 1 of 2 tests", "\n".join(quiet.details))
        self.assertNotIn("slow tests reached", "\n".join(quiet.details))
        self.assertEqual(self.slow_plan(base, "{ 1 }", "{ 4 }", all_tests=True).details, [])
        # A crate every test of which runs: every slow test of it; and one every native test of
        # which runs, as the testkit's do for a base change (RUST_READERS).
        data = self.slow_plan("crates/hyperion-base/data/table.bin", "", "")
        self.assertIn("  slow tests reached (run by name): every slow test of hyperion-base (2)", data.details)
        kit = self.slow_plan(base, "{ 1 }", "{ 4 }", kit=True)
        self.assertIn("  slow tests reached (run by name): every slow test of hyperion-testkit (1)", kit.details)
        self.assertIn("package(hyperion-testkit)", args(kit, "_nextest-run")[1])


class TheGuard(unittest.TestCase):
    """What the cross-Clippy guard counts as code that differs between Linux, macOS and Windows: a cfg
    that takes different values on the three (decision-test-coverage.md §3.4 addendum), a path through
    `std::os` or `core::os`, an OS interface, a platform's dependency table, a build script."""

    def flags(self, code: str) -> List[str]:
        return smart_ci.os_dependent_code(code)

    def test_cfgs_that_differ_between_the_platforms(self) -> None:
        for code in ("#[cfg(unix)]\nfn f() {}", "#[cfg(windows)]\nfn f() {}", "#[cfg(target_os = \"linux\")]\nfn f() {}",
                     "#![cfg(all(target_os = \"linux\", target_arch = \"x86_64\", target_env = \"gnu\"))]",
                     "#[cfg(any(target_family = \"wasm\", unix))]\nfn f() {}", "fn f() { if cfg!(windows) {} }",
                     "#[cfg_attr(target_os = \"macos\", doc = \"x\")]\nfn f() {}", "#[cfg(all(test, unix))]\nmod t {}",
                     "#[cfg(target_has_atomic = \"128\")]\nfn f() {}", "#[cfg(target_feature = \"neon\")]\nfn f() {}",
                     "#[cfg(target_arch = \"x86_64\")]\nfn f() {}", "#[cfg(target_vendor = \"apple\")]\nfn f() {}",
                     "#[cfg(not(target_family = \"unix\"))]\nfn f() {}"):
            with self.subTest(code=code):
                self.assertTrue(self.flags(code))
        self.assertEqual(self.flags("#[cfg(any(target_family = \"wasm\", unix))]\nfn f() {}"),
                         ['cfg(any(target_family = "wasm", unix))'])

    def test_cfgs_that_take_one_value_on_all_three_are_exempt(self) -> None:
        for code in ("#[cfg(all(target_arch = \"wasm32\", target_os = \"unknown\"))]\nmod wasm;",
                     "#[cfg(not(all(target_arch = \"wasm32\", target_os = \"unknown\")))]\nmod native;",
                     "#[cfg(target_family = \"wasm\")]\nfn f() {}", "#[cfg(not(target_family = \"wasm\"))]\nfn f() {}",
                     "#[cfg(target_pointer_width = \"64\")]\nfn f() {}", "#[cfg(target_endian = \"little\")]\nfn f() {}",
                     "#[cfg(target_feature = \"relaxed-simd\")]\ncompile_error!(\"x\");", "#[cfg(feature = \"testing\")]\nfn f() {}",
                     "#[cfg(any(test, target_arch = \"wasm32\"))]\nfn f() {}", "#[cfg(test)]\nmod tests {}",
                     "#[cfg_attr(test, allow(dead_code))]\nfn f() {}", "macro_rules! m { (#[cfg($c:meta)]) => {} }"):
            with self.subTest(code=code):
                self.assertEqual(self.flags(code), [])

    def test_comments_and_strings_are_no_code(self) -> None:
        self.assertEqual(self.flags('// #[cfg(unix)]\n/// cfg(windows), std::os::unix\nconst A: &str = "cfg(unix)";'), [])

    def test_paths_through_std_os(self) -> None:
        for code in ("use std::os::unix::ffi::OsStrExt;", "use std::{fs, os::unix::fs::PermissionsExt};",
                     "fn f() { let _ = ::std::os::raw::c_int::MAX; }", "use core::os;"):
            with self.subTest(code=code):
                self.assertTrue(self.flags(code))
        self.assertEqual(self.flags("use crate::os::thing;\nmod os;"), [])
        # WASI's module exists on no desktop target, so code that uses it is WebAssembly's alone.
        self.assertEqual(self.flags('#[cfg(target_os = "wasi")]\nuse std::os::wasi::fs::FileExt;'), [])
        self.assertEqual(self.flags("use std::{fs, os::wasi::ffi::OsStrExt};"), [])

    def test_architectures_features_and_c_integers(self) -> None:
        # std's architecture modules, CPU-feature detection and enabled features, and `c_long`, whose
        # width is Windows' own: each differs between x86-64 and Apple silicon, or Windows and the rest.
        for code in ("use std::arch::x86_64::_mm_add_pd;", "fn f() { core::arch::aarch64::vaddq_f64; }",
                     "use core::{arch::x86_64::*, mem};", 'fn f() -> bool { is_x86_feature_detected!("avx2") }',
                     '#[target_feature(enable = "avx2")]\nfn f() {}', "use std::ffi::c_long;",
                     "fn f(x: core::ffi::c_ulong) {}"):
            with self.subTest(code=code):
                self.assertTrue(self.flags(code))
        for code in ("use core::arch::wasm32::f32x4_add;", '#[target_feature(enable = "simd128")]\nfn f() {}',
                     "use std::ffi::c_int;", "fn arch() {}\nlet is_x86_feature_detected = 1;"):
            with self.subTest(code=code):
                self.assertEqual(self.flags(code), [])

    def test_the_manifests_platform_tables_and_os_interfaces(self) -> None:
        self.assertFalse(smart_ci.platform_differs('cfg(all(target_arch = "wasm32", target_os = "unknown"))'))
        self.assertFalse(smart_ci.platform_differs('cfg(not(all(target_arch = "wasm32", target_os = "unknown")))'))
        self.assertFalse(smart_ci.platform_differs("wasm32-unknown-unknown"))
        self.assertTrue(smart_ci.platform_differs("cfg(windows)"))
        self.assertTrue(smart_ci.platform_differs("x86_64-pc-windows-msvc"))
        member = {"id": "a", "name": "a", "targets": [{"kind": ["lib"]}], "dependencies": [
            {"name": "wasm-bindgen", "target": 'cfg(all(target_arch = "wasm32", target_os = "unknown"))'},
            {"name": "serde", "target": None}]}
        meta = {"workspace_members": ["a", "b", "c"], "packages": [
            member,
            {"id": "b", "name": "b", "targets": [{"kind": ["lib"]}, {"kind": ["custom-build"]}],
             "dependencies": [{"name": "libc", "target": None}]},
            {"id": "c", "name": "c", "targets": [{"kind": ["lib"]}],
             "dependencies": [{"name": "winapi", "target": "cfg(windows)"}]}]}
        found = smart_ci.manifest_os_dependence(meta)
        self.assertEqual(sorted(found), ["b", "c"])
        self.assertEqual(found["b"], ["a build script", "a dependency on libc"])
        self.assertEqual(found["c"], ["[target.'cfg(windows)'] dependencies", "a dependency on winapi"])

    def test_it_reads_every_target_clippy_lints(self) -> None:
        ws = workspace()
        files = dict(FILES)
        for path in ("crates/hyperion-sim/benches/sky.rs", "crates/hyperion-sim/tests/a.rs",
                     "crates/hyperion-sim/examples/e.rs", "crates/hyperion-sim/build.rs",
                     "crates/hyperion-sim/data/not_code.rs"):
            files[path] = "#[cfg(target_os = \"linux\")]\nfn f() {}\n"
        ws.files = sorted(files)
        ws.read = lambda path: files.get(path, "")
        found = " ".join(smart_ci.os_dependence(ws, SIM))
        for path in ("benches/sky.rs", "tests/a.rs", "examples/e.rs", "build.rs"):
            self.assertIn("crates/hyperion-sim/" + path, found)
        self.assertNotIn("data/not_code.rs", found)


class FakeRunner(smart_ci.Runner):
    """Records what `execute` runs, and reads the build phases a slot's child would run."""

    SCOPE = ["scope", "--"]  # the suites' capped scope, without the heavy-test lock

    def __init__(self, fail: Sequence[str] = (), output: Sequence[str] = ()) -> None:
        super().__init__(ROOT, "just")
        self.calls: List[Tuple[str, List[str]]] = []
        self.handoffs: List[dict] = []
        self.envs: List[Optional[Dict[str, str]]] = []  # what each `run` adds to the environment
        self.scopes: List[List[str]] = []  # the scope each `run` runs in
        self.fail = set(fail)
        self.output = list(output)  # what the suites print, to the run's `watch`

    def run(self, args: Sequence[str], env=None, watch=None, scope=()) -> int:
        self.calls.append(("run", list(args)))
        self.envs.append(dict(env) if env else None)
        self.scopes.append(list(scope))
        if watch is not None:
            for line in self.output:
                watch(line + "\n")
        return 1 if args[0] in self.fail else 0

    def suite_scope(self, env) -> List[str]:
        return list(self.SCOPE)

    def run_command(self, command: Sequence[str]) -> int:
        self.calls.append(("command", list(command)))
        self.handoffs.append(json.loads(Path(command[command.index("--build-phases") + 1]).read_text(encoding="utf-8")))
        return 1 if "slot" in self.fail else 0

    def spawn_held(self, args: Sequence[str], log) -> "FakeProcess":
        self.calls.append(("spawn", list(args)))
        return FakeProcess()


class FakeProcess:
    def wait(self) -> int:
        return 0


class Execution(unittest.TestCase):
    """The phases run in `ci`'s order, and a build slot is held only around the build phases."""

    SLOT = {"SMART_CI_SLOT": "/opt/build-slot"}

    def setUp(self) -> None:
        # What `execute` tells the person running it: not this suite's output.
        quiet = contextlib.redirect_stderr(io.StringIO())
        quiet.__enter__()
        self.addCleanup(quiet.__exit__, None, None, None)

    def test_without_a_slot_the_phases_run_directly_in_cis_order(self) -> None:
        p = plan("crates/hyperion-server/src/main.rs")
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env={}, runner=runner), 0)
        self.assertEqual([kind for kind, _ in runner.calls], ["spawn", "run", "run"])
        self.assertEqual(runner.calls[0][1][0], "_rustfmt-check")
        self.assertEqual(runner.calls[1][1][0], "_clippy-for")
        self.assertEqual(runner.calls[2][1][:3], ["_locked", "just", "_nextest-run"])

    def test_a_slot_is_held_around_each_group_that_builds_and_released_before_the_lock(self) -> None:
        p = plan("crates/hyperion-surface/src/field.rs")
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env=self.SLOT, runner=runner), 0)
        (setup_kind, setup), (build_kind, build), (locked_kind, locked) = runner.calls
        self.assertEqual((setup_kind, build_kind), ("command", "command"))
        self.assertEqual(build[0], "/opt/build-slot")
        self.assertEqual(build[2:4], ["-B", str(HERE / "smart_ci.py")])
        phases = smart_ci.phases_of(p)
        self.assertEqual(runner.handoffs, [{"setup": phases["setup"]},
                                           {"side": phases["side"], "unlocked": phases["unlocked"], "main": phases["main"]}])
        self.assertEqual(phases["unlocked"][0][0], "_vitest-run")
        self.assertEqual(phases["setup"][0], ["gen-surface"])
        # The suites run after the slot's command has ended: in the parent, under the lock alone.
        self.assertEqual((locked_kind, locked[:2]), ("run", ["_locked", "just"]))
        self.assertFalse(list((ROOT / "target" / "smart-ci").glob(f"build-phases-{os.getpid()}.json")))
        text = smart_ci.describe(p, [], "x", False, env=self.SLOT)
        self.assertIn("held for the setup and then the side, unlocked, main phases", text)

    def test_the_slots_child_runs_the_phases_it_is_given(self) -> None:
        runner = FakeRunner()
        phases = smart_ci.phases_of(plan("crates/hyperion-surface/src/field.rs"))
        self.assertEqual(smart_ci.run_build_phases(phases, runner), 0)
        self.assertEqual(runner.calls, [("run", smart_ci.flat(phases["setup"])), ("spawn", smart_ci.flat(phases["side"])),
                                        ("spawn", smart_ci.flat(phases["unlocked"])), ("run", smart_ci.flat(phases["main"]))])
        main_only = FakeRunner()
        self.assertEqual(smart_ci.run_build_phases({"main": phases["main"]}, main_only), 0)
        self.assertEqual(main_only.calls, [("run", smart_ci.flat(phases["main"]))])

    def test_a_held_slot_or_a_group_that_builds_nothing_takes_no_slot(self) -> None:
        held = FakeRunner()
        smart_ci.execute(plan("crates/hyperion-server/src/main.rs"), ROOT, "just",
                         env={**self.SLOT, "BUILD_SLOT_HELD": "1"}, runner=held)
        self.assertNotIn("command", [kind for kind, _ in held.calls])
        self.assertIn("BUILD_SLOT_HELD", smart_ci.describe(plan("crates/hyperion-server/src/main.rs"), [], "x", False,
                                                           env={"BUILD_SLOT_HELD": "1"}))
        for path in ("README.md", "tools/smart-ci/smart_ci.py"):
            with self.subTest(path=path):
                docs = FakeRunner()
                smart_ci.execute(plan(path), ROOT, "just", env=self.SLOT, runner=docs)
                self.assertEqual([kind for kind, _ in docs.calls], ["spawn"])
                self.assertNotIn("build slot", smart_ci.describe(plan(path), [], "x", False, env=self.SLOT))
        nothing = FakeRunner()
        smart_ci.execute(plan(".claude/skills/validate/SKILL.md"), ROOT, "just", env=self.SLOT, runner=nothing)
        self.assertEqual(nothing.calls, [])
        # TypeScript alone: the surface module's build in a slot, then tsc, oxlint and Prettier, and
        # vitest beside them, without one, and no heavy-test lock (decision-test-coverage.md R1).
        ts = FakeRunner()
        smart_ci.execute(plan("apps/hyperion/src/renderer/src/lib/format.ts"), ROOT, "just", env=self.SLOT, runner=ts)
        self.assertEqual([kind for kind, _ in ts.calls], ["command", "spawn", "spawn"])
        self.assertEqual(ts.calls[2][1][0], "_vitest-run")
        self.assertEqual(ts.handoffs, [{"setup": [["gen-surface"]]}])

    def test_a_full_plan_runs_cis_own_phases_with_the_slot_released_before_the_lock(self) -> None:
        p = plan("justfile")
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env=self.SLOT, runner=runner), 0)
        self.assertEqual(runner.handoffs, [{"setup": smart_ci.CI_PHASES["setup"]},
                                           {phase: smart_ci.CI_PHASES[phase] for phase in ("side", "unlocked", "main")}])
        self.assertEqual(runner.calls[-1], ("run", ["_locked", "just", "_test-run", "_wasm-fast-run"]))
        self.assertIn("[locked] just _test-run _wasm-fast-run", smart_ci.describe(p, [("justfile", False)], "x", False))

    def test_full_runs_cis_own_phases_whatever_the_diff(self) -> None:
        # Integration's run: `ci`'s phases (which AgreesWithCi holds to the justfile's `ci`), its two
        # build groups in the slot, the suites outside it, and no test-render, as `ci` has none.
        p = smart_ci.full_plan()
        self.assertEqual(smart_ci.phases_of(p), smart_ci.CI_PHASES)
        self.assertEqual(p.recipes(), ["ci"])
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env=self.SLOT, runner=runner), 0)
        self.assertEqual(runner.handoffs, [{"setup": smart_ci.CI_PHASES["setup"]},
                                           {phase: smart_ci.CI_PHASES[phase] for phase in ("side", "unlocked", "main")}])
        self.assertEqual(runner.calls[-1], ("run", ["_locked", "just", "_test-run", "_wasm-fast-run"]))
        # The command line reads no diff for it. (Its environment is this suite's own: no slot held
        # already, as `ci`'s side phase, which runs this suite inside one, would have it.)
        shown = io.StringIO()
        with contextlib.redirect_stdout(shown), unittest.mock.patch.dict(os.environ, self.SLOT):
            os.environ.pop("BUILD_SLOT_HELD", None)
            self.assertEqual(smart_ci.main(["--full", "--plan"]), 0)
        self.assertIn(smart_ci.FULL_RUN, shown.getvalue())
        self.assertIn("is held for the setup and then the side, unlocked, main phases", shown.getvalue())

    def test_the_smoke_harness_runs_in_the_suites_hold(self) -> None:
        # perf-ts Q1: one queue for the lock, not two; no `just test-render`, whose recipe takes it again.
        p = plan("crates/hyperion-server/src/main.rs", "apps/hyperion/src/renderer/src/view/shaders/sky.wgsl")
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env={}, runner=runner), 0)
        locked = [args for kind, args in runner.calls if args[:1] == ["_locked"]]
        self.assertEqual(len(locked), 1)
        self.assertEqual(locked[0][-1], "_test-render-run")
        self.assertIn("_nextest-run", locked[0])
        self.assertFalse(any("test-render" in args for _, args in runner.calls))
        built = [args for kind, args in runner.calls if kind == "spawn" and "_test-render-build" in args]
        self.assertEqual(len(built), 1)
        # A full plan's too: `ci`'s phases, with test-render's build beside them and its run last.
        full = FakeRunner()
        self.assertEqual(smart_ci.execute(plan("pnpm-lock.yaml"), ROOT, "just", env={}, runner=full), 0)
        self.assertEqual(full.calls[-1], ("run", ["_locked", "just", "_test-run", "_wasm-fast-run", "_test-render-run"]))
        self.assertTrue(any(kind == "spawn" and args[-1] == "_test-render-build" for kind, args in full.calls))
        self.assertNotIn(["_test-render-run"], smart_ci.phases_of(smart_ci.full_plan())["locked"])

    def test_integrations_runs_take_the_lock_with_priority_when_asked(self) -> None:
        # perf-build L5 part a: SMART_CI_PRIORITY=1 gives `_locked` HYPERION_HEAVY_PRIORITY=1, for any
        # plan: integration runs smart-ci over its unpushed commits (owner, 2026-10-12), not only --full.
        priority = {"SMART_CI_PRIORITY": "1"}
        for plan_, env, wanted in ((smart_ci.full_plan(), priority, {"HYPERION_HEAVY_PRIORITY": "1"}),
                                   (smart_ci.full_plan(), {}, None),
                                   (plan("justfile"), priority, {"HYPERION_HEAVY_PRIORITY": "1"}),
                                   (plan("crates/hyperion-server/src/main.rs"), priority, {"HYPERION_HEAVY_PRIORITY": "1"}),
                                   (plan("crates/hyperion-server/src/main.rs"), {"SMART_CI_PRIORITY": "0"}, None),
                                   (plan("crates/hyperion-server/src/main.rs"), {}, None)):
            with self.subTest(full=plan_.full, env=env):
                runner = FakeRunner()
                self.assertEqual(smart_ci.execute(plan_, ROOT, "just", env=env, runner=runner), 0)
                locked = [e for (kind, args), e in zip([c for c in runner.calls if c[0] == "run"], runner.envs)
                          if args[0] == "_locked"]
                self.assertEqual(locked, [wanted])
        for plan_ in (smart_ci.full_plan(), plan("justfile"), plan("crates/hyperion-server/src/main.rs")):
            with self.subTest(full=plan_.full):
                text = smart_ci.describe(plan_, [], "x", False, env=priority)
                self.assertIn("They take it with priority (SMART_CI_PRIORITY=1)", text)
                self.assertNotIn("with priority", smart_ci.describe(plan_, [], "x", False, env={}))
        # A plan with no suites takes no lock, and says nothing of priority.
        self.assertNotIn("heavy-test lock", smart_ci.describe(plan("README.md"), [], "x", False, env=priority))

    def test_integrations_runs_timestamp_their_recipe_lines(self) -> None:
        # perf-build Q4: `--full` or SMART_CI_PRIORITY=1 sets JUST_TIMESTAMP=true for every `just` smart-ci
        # runs, unless the environment sets it; a lane's runs are unchanged.
        stamp = {"JUST_TIMESTAMP": "true"}
        for full, env, wanted in ((True, {}, stamp), (False, {"SMART_CI_PRIORITY": "1"}, stamp),
                                  (True, {"SMART_CI_PRIORITY": "1"}, stamp), (False, {}, {}),
                                  (False, {"SMART_CI_PRIORITY": "0"}, {}),
                                  (True, {"JUST_TIMESTAMP": "false"}, {})):
            with self.subTest(full=full, env=env):
                self.assertEqual(smart_ci.timestamps(full, env), wanted)
        said = "Each recipe line is timestamped (JUST_TIMESTAMP=true)"
        self.assertIn(said, smart_ci.describe(smart_ci.full_plan(), [], "x", False, env={}))
        server = plan("crates/hyperion-server/src/main.rs")
        self.assertIn(said, smart_ci.describe(server, [], "x", False, env={"SMART_CI_PRIORITY": "1"}))
        self.assertNotIn(said, smart_ci.describe(server, [], "x", False, env={}))

    def test_a_failed_build_runs_no_suite(self) -> None:
        for env, fail in (({}, "_clippy-for"), (self.SLOT, "slot")):
            with self.subTest(env=env):
                runner = FakeRunner(fail=[fail])
                self.assertEqual(smart_ci.execute(plan("crates/hyperion-server/src/main.rs"), ROOT, "just", env=env,
                                                  runner=runner), 1)
                self.assertFalse(any(args[:1] == ["_locked"] for _, args in runner.calls))

    def test_the_slot_command_is_split_as_a_command_line(self) -> None:
        self.assertIsNone(smart_ci.slot_command({}))
        self.assertIsNone(smart_ci.slot_command({"SMART_CI_SLOT": " "}))
        self.assertEqual(smart_ci.slot_command({"SMART_CI_SLOT": "bash /opt/build-slot"}), ["bash", "/opt/build-slot"])


class TheGateStamp(unittest.TestCase):
    """What a plan gives its gate stamp (`stamp.py`): with `--hook-checks`, the workspace's Clippy and
    bindings' check, the ones that cover the commit hooks'; the run's record; and that a completed run
    stamps or withdraws, an interrupted one neither, and a range's only where the working tree is its
    end's."""

    BEGAN = {"tree": "a" * 40, "toolchain": {"rustc": "r"}, "environment": {}, "configuration": {}}

    @classmethod
    def record(cls, p: Plan, paths: Sequence[str] = ("crates/hyperion-base/src/math.rs",)) -> dict:
        change = smart_ci.Change([(path, False) for path in paths], "x", False, "c" * 40)
        return smart_ci.stamp_run(p, cls.BEGAN, "b" * 40, change, "rendering-and-planets", {})

    def test_only_the_workspaces_clippy_covers_the_hooks(self) -> None:
        # The changed crate's Clippy, a `-p` selection, as by default: the hook's is not covered, and the
        # plan says how to cover it.
        p = plan("crates/hyperion-base/src/math.rs")
        self.assertEqual(packages(p, "_clippy-for"), [BASE])
        self.assertFalse(stamp.covers(self.record(p), "clippy")[0])
        self.assertTrue(any("--hook-checks" in note for note in p.notes))
        # Nor with its dependents', whose external dependencies resolve with fewer features than the workspace's.
        self.assertFalse(stamp.covers(self.record(plan("crates/hyperion-base/src/math.rs", dependents=True)), "clippy")[0])
        w = make_plan([("crates/hyperion-base/src/math.rs", False)], workspace(), False, hook_checks=True)
        self.assertEqual(args(w, "_clippy-for"), ["--workspace"])
        self.assertEqual(calls(w).count("_clippy-for"), 1)
        self.assertTrue(stamp.covers(self.record(w), "clippy")[0])
        self.assertFalse(any("--hook-checks" in note for note in w.notes))
        # The rest of the plan is the same: the browser crates' Clippy, the tests.
        self.assertEqual([s.recipe for s in w.steps if s.recipe[0] != "_clippy-for"],
                         [s.recipe for s in p.steps if s.recipe[0] != "_clippy-for"])
        # With no Rust in the change, there is no Clippy to run.
        ts = make_plan([("apps/hyperion/src/renderer/src/lib/format.ts", False)], workspace(), False, hook_checks=True)
        self.assertNotIn("_clippy-for", calls(ts))

    def test_only_the_workspaces_bindings_check_covers_the_hooks(self) -> None:
        # The hook's `--workspace` run builds, links and runs every crate's library tests; the plan's own
        # covers the bindings' freshness over its selection alone.
        path = "crates/hyperion-protocol/src/lib.rs"
        p = plan(path)
        self.assertNotEqual(args(p, "_gen-protocol-check-for"), ["--workspace"])
        self.assertFalse(stamp.covers(self.record(p, [path]), "protocol-bindings")[0])
        self.assertTrue(any("Clippy and bindings' check" in note for note in p.notes))
        w = make_plan([(path, False)], workspace(), False, hook_checks=True)
        self.assertEqual(args(w, "_gen-protocol-check-for"), ["--workspace"])
        record = self.record(w, [path])
        self.assertTrue(all(stamp.covers(record, check)[0] for check in stamp.CHECKS))
        generated = make_plan([("packages/protocol/src/generated/BodyId.ts", False)], workspace(), False, hook_checks=True)
        self.assertEqual(args(generated, "_gen-protocol-check-for"), ["--workspace"])
        self.assertNotIn("_clippy-for", calls(generated))

    def test_the_record_holds_every_cause_of_each_step_and_a_full_plan_covers_every_hook(self) -> None:
        docs = [f"docs/note{i}.md" for i in range(6)]
        p = plan(*docs)
        record = self.record(p, docs)
        (prettier,) = record["steps"]
        self.assertEqual(prettier["recipe"][0], "_prettier-check")
        self.assertEqual(prettier["causes"], docs)
        self.assertEqual(record["changes"], [[path, False] for path in docs])
        self.assertEqual((record["tree"], record["head"], record["base"], record["base_ref"], record["exit"]),
                         ("a" * 40, "b" * 40, "c" * 40, "rendering-and-planets", 0))
        self.assertEqual(record["toolchain"], {"rustc": "r"})
        for check in stamp.CHECKS:
            with self.subTest(check=check):
                self.assertEqual(stamp.covers(record, check)[0], check == "test")
        full = smart_ci.stamp_run(plan("justfile"), self.BEGAN, "b" * 40, None, "--full", {})
        self.assertTrue(full["full"])
        self.assertIsNone(full["base"])
        self.assertTrue(all(stamp.covers(full, check)[0] for check in stamp.CHECKS))

    def test_a_completed_run_stamps_or_withdraws_and_an_interrupted_or_planned_one_does_neither(self) -> None:
        change = smart_ci.Change([("crates/hyperion-server/src/main.rs", False)], "x", False, "c" * 40)
        patches = [unittest.mock.patch.object(smart_ci, "changed_paths", return_value=change),
                   unittest.mock.patch.object(smart_ci, "load_workspace", return_value=workspace()),
                   unittest.mock.patch.object(smart_ci, "node_floor_met", return_value=True),
                   unittest.mock.patch.object(smart_ci, "stamp_before", return_value=self.BEGAN),
                   unittest.mock.patch.dict(os.environ)]  # main() may set JUST_TIMESTAMP
        with contextlib.ExitStack() as stack, contextlib.redirect_stdout(io.StringIO()):
            for patcher in patches:
                stack.enter_context(patcher)
            for status in (0, 1):
                with self.subTest(status=status), unittest.mock.patch.object(smart_ci, "execute", return_value=status), \
                        unittest.mock.patch.object(smart_ci, "stamp_after") as after:
                    self.assertEqual(smart_ci.main(["--hook-checks"]), status)
                    ((root, began, ended, p, given, base_ref, options), _), = after.call_args_list
                    self.assertEqual((root, began, ended, given, base_ref), (ROOT, self.BEGAN, status, change,
                                                                             smart_ci.INTEGRATION_BRANCH))
                    self.assertEqual(options, {"dependents": False, "all_tests": False, "hook_checks": True})
                    self.assertEqual(args(p, "_clippy-for"), ["--workspace"])
            with unittest.mock.patch.object(smart_ci, "execute", side_effect=KeyboardInterrupt), \
                    unittest.mock.patch.object(smart_ci, "stamp_after") as after:
                with self.assertRaises(KeyboardInterrupt):
                    smart_ci.main([])
                after.assert_not_called()
            with unittest.mock.patch.object(smart_ci, "execute") as run, \
                    unittest.mock.patch.object(smart_ci, "stamp_before") as before:
                self.assertEqual(smart_ci.main(["--plan"]), 0)
                run.assert_not_called()
                before.assert_not_called()

    def test_a_ranges_run_is_stamped_only_on_its_ends_tree_and_stamping_never_fails_a_run(self) -> None:
        p = plan("crates/hyperion-server/src/main.rs")
        ranged = smart_ci.Change([("crates/hyperion-server/src/main.rs", False)], "x", True, "c" * 40, "d" * 40)
        with contextlib.redirect_stderr(io.StringIO()) as said, \
                unittest.mock.patch.object(stamp, "record") as record:
            with unittest.mock.patch.object(stamp, "tree_of", return_value="e" * 40):
                smart_ci.stamp_after(ROOT, self.BEGAN, 0, p, ranged, "x", {})
            record.assert_not_called()
            self.assertIn("not the working tree", said.getvalue())
            with unittest.mock.patch.object(stamp, "tree_of", return_value=self.BEGAN["tree"]):
                smart_ci.stamp_after(ROOT, self.BEGAN, 0, p, ranged, "x", {})
            record.assert_called_once()
        with contextlib.redirect_stderr(io.StringIO()) as said, \
                unittest.mock.patch.object(stamp, "record", side_effect=RuntimeError("disk")):
            smart_ci.stamp_after(ROOT, self.BEGAN, 0, p, None, "x", {})
        self.assertIn("no stamp: disk", said.getvalue())


class SmallSuites(unittest.TestCase):
    """Suites small by their recorded timings run without the heavy-test lock, capped, holding no build
    slot; large ones, unknown ones, `--full` and the smoke harness take it."""

    # A change to the sim's sky: its one test there, natively and on wasip1, and the testkit's suite,
    # whose `clippy_bans` reads the sim's sources, natively and whole (`GraphMode`'s workspace).
    CHANGE = ("crates/hyperion-sim/src/sky/mod.rs", "* 3", "* 4")
    NATIVE = {"hyperion-sim": {"sky::tests::glows": 2.0, "tests::plain": 500.0}, "hyperion-testkit": {"kit": 0.1}}
    WASIP1 = {"hyperion-sim": {"sky::tests::glows": 3.0, "tests::plain": 900.0}}
    SLOT = {"SMART_CI_SLOT": "/opt/build-slot"}

    def setUp(self) -> None:
        quiet = contextlib.redirect_stderr(io.StringIO())
        quiet.__enter__()
        self.addCleanup(quiet.__exit__, None, None, None)

    def plan(self, wasm_sky: str = "") -> Plan:
        graph = GraphMode()
        now = dict(graph.SOURCES)
        path, old, new = self.CHANGE
        now[path] = now[path].replace(old, new)
        ws = graph.workspace(now)
        ws.wasm_sky = wasm_sky
        return make_plan([(path, False)], ws, False)

    def timings(self, native: Optional[dict] = None, wasip1: Optional[dict] = None) -> smart_ci.Timings:
        return smart_ci.Timings(None, json.loads(json.dumps({"native": self.NATIVE if native is None else native,
                                                             "wasip1": self.WASIP1 if wasip1 is None else wasip1})))

    def weighed(self, p: Plan, timings: smart_ci.Timings) -> Plan:
        smart_ci.weigh(p, timings)
        return p

    def test_a_small_plan_runs_without_the_lock_capped_and_in_a_slot_of_its_own(self) -> None:
        p = self.weighed(self.plan(), self.timings())
        self.assertEqual([s.kind for s in p.suites], ["native", "wasip1"])
        self.assertTrue(p.unlocked, p.weight)
        # glows and the testkit's one test natively, glows on wasip1: not the sim's `plain`, which the
        # step's filter leaves out.
        found = self.timings().weigh(p.suites)
        self.assertEqual((found.tests, round(found.seconds, 1), found.unknown), (3, 5.1, []))
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env=self.SLOT, runner=runner), 0)
        # The builds in the slot's command; the suites after it, in the parent, in their own capped
        # scope at 4 test threads, inside it in a slot of their own (so that build-slot's gates
        # apply to them: the freeze of 2026-10-13), and no `_locked`.
        self.assertEqual([kind for kind, _ in runner.calls][-2:], ["command", "run"])
        self.assertNotIn("_nextest-run", smart_ci.flat(runner.handoffs[-1].get("main", [])))
        kind, suites = runner.calls[-1]
        self.assertEqual(suites, smart_ci.flat(smart_ci.phases_of(p)["locked"]))
        self.assertEqual([suites[0], suites[3]], ["_nextest-run", "_wasm-run"])
        self.assertEqual(runner.scopes[-1], [*FakeRunner.SCOPE, "/opt/build-slot"])
        self.assertEqual(runner.envs[-1], {"NEXTEST_TEST_THREADS": "4", "RUST_TEST_THREADS": "4", "CARGO_BUILD_JOBS": "4"})
        self.assertFalse(any(args[:1] == ["_locked"] for _, args in runner.calls))
        self.assertIn("in a build slot of their own (/opt/build-slot)", smart_ci.describe(p, [], "x", False, env=self.SLOT))
        text = smart_ci.describe(p, [], "x", False, env={"SMART_CI_PRIORITY": "1"})
        self.assertIn("The suites run without the heavy-test lock: 3 test(s) and 5 s of test time", text)
        self.assertIn(f"within {smart_ci.SMALL_TESTS} tests and {smart_ci.SMALL_SECONDS:.0f} s", text)
        self.assertIn("SMART_CI_PRIORITY=1 has nothing to do", text)
        self.assertIn("and hold no build slot", text)
        # With no slot command, or inside a slot already (BUILD_SLOT_HELD), the scope alone.
        for env in ({}, {**self.SLOT, "BUILD_SLOT_HELD": "1"}):
            with self.subTest(env=env):
                bare = FakeRunner()
                self.assertEqual(smart_ci.execute(p, ROOT, "just", env=env, runner=bare), 0)
                self.assertEqual(bare.scopes[-1], FakeRunner.SCOPE)

    def test_the_scope_is_vitests_where_systemd_allows_it(self) -> None:
        # The caps of `_vitest-run`'s scope, which R1 ruled no heavy run.
        justfile = (ROOT / "justfile").read_text(encoding="utf-8")
        vitest = justfile[justfile.index("\n_vitest-run "):justfile.index("\n\n", justfile.index("\n_vitest-run "))]
        for cap in smart_ci.UNLOCKED_SCOPE[1::2]:
            self.assertIn(f"-p {cap}", vitest)
        self.assertEqual(smart_ci.UNLOCKED_SCOPE, ("-p", "MemoryMax=5G", "-p", "MemorySwapMax=0", "-p", "CPUQuota=400%"))
        runner = smart_ci.Runner(ROOT, "just")
        with unittest.mock.patch.object(smart_ci.shutil, "which", return_value="/usr/bin/systemd-run"), \
                unittest.mock.patch.object(smart_ci.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)):
            self.assertEqual(runner.suite_scope({"HEAVY_SLICE": "agents.slice"}),
                             ["systemd-run", "--user", "--scope", "--quiet", "--slice=agents.slice",
                              *smart_ci.UNLOCKED_SCOPE, "--"])
        with unittest.mock.patch.object(smart_ci.shutil, "which", return_value=None):
            self.assertEqual(runner.suite_scope({}), [])  # macOS, Windows: 4 test threads alone

    def test_a_large_plan_takes_the_lock(self) -> None:
        slow = {**self.NATIVE, "hyperion-sim": {"sky::tests::glows": smart_ci.SMALL_SECONDS}}
        p = self.weighed(self.plan(), self.timings(native=slow))
        self.assertFalse(p.unlocked)
        self.assertIn(f"more than {smart_ci.SMALL_TESTS} tests or {smart_ci.SMALL_SECONDS:.0f} s", p.weight)
        many = {**self.NATIVE, "hyperion-testkit": {f"kit_{i}": 0.01 for i in range(smart_ci.SMALL_TESTS)}}
        p = self.weighed(self.plan(), self.timings(native=many))
        self.assertFalse(p.unlocked)
        self.assertIn(f"{smart_ci.SMALL_TESTS + 2} test(s)", p.weight)
        runner = FakeRunner()
        self.assertEqual(smart_ci.execute(p, ROOT, "just", env={}, runner=runner), 0)
        self.assertEqual(runner.calls[-1][1][:3], ["_locked", "just", "_nextest-run"])
        self.assertEqual(runner.scopes[-1], [])
        self.assertIn(f"The suites take the heavy-test lock: {smart_ci.SMALL_TESTS + 2} test(s)",
                      smart_ci.describe(p, [], "x", False, env={}))
        # More listed tests than that, or a filter made coarser to fit a command line: large, uncounted.
        names = {f"t{i}" for i in range(smart_ci.SMALL_TESTS + 1)}
        listed = smart_ci.Suite("native", crates={SIM}, tests={SIM: names}, filter="package(hyperion-sim)")
        recorded = smart_ci.Timings(None, {"native": {SIM: {name: 0.01 for name in names}}})
        coarse = smart_ci.Suite("wasip1", crates={SIM}, coarse=True)
        found = recorded.weigh([listed, coarse])
        self.assertEqual((found.tests, found.unknown), (0, []))
        self.assertEqual(found.large, [f"{smart_ci.SMALL_TESTS + 1} listed tests of hyperion-sim, more than {smart_ci.SMALL_TESTS}",
                                       "the tests of hyperion-sim on wasip1, too many for one command line's filter"])

    def test_unknown_timings_take_the_lock(self) -> None:
        for name, timings in (("none at all", smart_ci.Timings()),
                              ("not on wasip1", self.timings(wasip1={"hyperion-sim": {"tests::plain": 1.0}})),
                              ("none of the testkit's", self.timings(native={"hyperion-sim": {"sky::tests::glows": 1.0}}))):
            with self.subTest(name):
                p = self.weighed(self.plan(), timings)
                self.assertFalse(p.unlocked)
                self.assertIn("no recorded time, which counts as large", p.weight)
        p = self.weighed(self.plan(), self.timings(wasip1={}))
        self.assertIn("hyperion-sim sky::tests::glows on wasip1", p.weight)
        # Without the graph (no Rust graph, or --all-tests), the suites are not listed: unknown.
        q = plan("crates/hyperion-server/src/main.rs")
        smart_ci.weigh(q, self.timings())
        self.assertFalse(q.unlocked)
        self.assertIn("which no graph lists", q.weight)
        # And unweighed, a plan takes the lock.
        self.assertFalse(self.plan().unlocked)

    def test_full_and_the_smoke_harness_always_take_the_lock(self) -> None:
        full = smart_ci.full_plan()
        smart_ci.weigh(full, self.timings())
        self.assertFalse(full.unlocked)
        self.assertIn("the full `just ci`'s always do", smart_ci.describe(full, [], "x", False, env={}))
        render = plan("crates/hyperion-server/src/main.rs", "apps/hyperion/src/renderer/src/view/shaders/sky.wgsl")
        smart_ci.weigh(render, self.timings())
        self.assertFalse(render.unlocked)
        self.assertIn("smoke harness always does", render.weight)
        for p in (full, render):
            runner = FakeRunner()
            self.assertEqual(smart_ci.execute(p, ROOT, "just", env={}, runner=runner), 0)
            self.assertEqual(runner.calls[-1][1][:2], ["_locked", "just"])

    def test_a_test_that_reads_a_fixture_built_once_a_run_is_large(self) -> None:
        # `sky::testing::shared` builds the luminosity tables once a run and binary: the tests' recorded
        # times leave the build out, which a small run pays at once.
        graph = GraphMode()
        now = dict(graph.SOURCES)
        sky = "crates/hyperion-sim/src/sky/mod.rs"
        now[sky] = ("pub fn glow() -> u32 { hyperion_base::unit() * 4 }\n#[cfg(test)]\nmod testing;\n"
                    "#[cfg(test)]\nmod tests {\n    #[test]\n"
                    "    fn glows() { assert_eq!(super::glow(), super::testing::shared::tables()); }\n}\n")
        now["crates/hyperion-sim/src/sky/testing.rs"] = "pub(crate) mod shared;\n"
        now["crates/hyperion-sim/src/sky/testing/shared.rs"] = "pub(crate) fn tables() -> u32 { 4 }\n"
        p = self.weighed(make_plan([(sky, False)], graph.workspace(now), False), self.timings())
        self.assertEqual(p.suites[0].shared, {SIM: {"sky::tests::glows"}})
        self.assertFalse(p.unlocked)
        self.assertIn("read a fixture built once a run", p.weight)
        self.assertEqual(set(smart_ci.RUN_SHARED), {"crates/hyperion-sim/src/sky/testing/shared.rs"})
        self.assertTrue((ROOT / next(iter(smart_ci.RUN_SHARED))).is_file())

    def test_a_term_whose_generator_changed_is_unknown(self) -> None:
        # A `#[test_case]`'s cases, in the changed file: the change may add one that no run has timed.
        graph = GraphMode()
        now = dict(graph.SOURCES)
        sky = "crates/hyperion-sim/src/sky/mod.rs"
        now[sky] = now[sky].replace("* 3", "* 4").replace(
            "    fn glows()", "    #[test_case(1)]\n    fn cases(n: u32) { assert_eq!(super::glow(), 4 * n); }\n"
                             "    #[test]\n    fn glows()")
        recorded = {**self.NATIVE, SIM: {**self.NATIVE[SIM], "sky::tests::cases::_1_expects": 0.1}}
        p = self.weighed(make_plan([(sky, False)], graph.workspace(now), False),
                         self.timings(native=recorded, wasip1={SIM: {**self.WASIP1[SIM], "sky::tests::cases::_1_expects": 0.1}}))
        self.assertEqual(p.suites[0].fresh, {SIM: {"test(/^sky::tests::cases::/)"}})
        self.assertFalse(p.unlocked)
        self.assertIn("whose generator changed", p.weight)

    def test_a_test_that_wasm_sky_leaves_out_is_not_wasip1s(self) -> None:
        p = self.weighed(self.plan(wasm_sky="package(hyperion-sim) & test(/^sky::/)"), self.timings(wasip1={}))
        wasip1 = next(s for s in p.suites if s.kind == "wasip1")
        self.assertEqual(wasip1.tests, {})
        self.assertTrue(p.unlocked, p.weight)
        # One smart-ci cannot read leaves the wasip1 suite unlisted, and the plan still made.
        unread = self.weighed(self.plan(wasm_sky="M"), self.timings())
        self.assertFalse(next(s for s in unread.suites if s.kind == "wasip1").listed)
        self.assertFalse(unread.unlocked)

    def test_a_run_records_what_it_timed(self) -> None:
        output = [
            "    Starting 2 tests across 2 binaries (5 tests skipped)",
            "        PASS [   1.250s] (1/2) hyperion-sim sky::tests::glows",
            "\x1b[32m        LEAK\x1b[0m [   0.010s] (2/2) hyperion-testkit kit",
            "     Summary [   1.300s] 2 tests run: 2 passed, 5 skipped",
            "   Doc-tests hyperion_sim",
            "all doctests ran in 8.73s; merged doctests compilation took 2.35s",
            "    Starting 1 test across 1 binary",
            "        FAIL [   0.500s] (1/1) hyperion-sim sky::tests::glows",
            "     Running src/lib.rs (/t/wasm32-unknown-unknown/slow-test/deps/hyperion_surface-1.wasm)",
            "test result: ok. 266 passed; 0 failed; 15 ignored; 0 filtered out; finished in 70.43s",
            "     Running tests/bake.rs (/t/bake-2.wasm)",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.50s",
        ]
        record = smart_ci.RunRecord(["native", "wasip1"], {"/t/wasm32-unknown-unknown/slow-test/deps/hyperion_surface-1.wasm":
                                                           SURFACE, "/t/bake-2.wasm": SURFACE})
        for line in output:
            record.line(line + "\n")
        self.assertEqual(record.runs, [("native", 2, {SIM: {"sky::tests::glows": 1.25}, TESTKIT: {"kit": 0.01}}),
                                       ("wasip1", 1, {})])
        self.assertEqual(record.doctests, {"hyperion_sim": 8.73})  # rustdoc's whole time, its compilation in it
        self.assertEqual(record.browser, {SURFACE: 70.93})
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "smart-ci" / smart_ci.TIMINGS_FILE
            timings = smart_ci.Timings.load(path)
            p = self.plan()
            runner = FakeRunner(output=output[:4])
            self.assertEqual(smart_ci.execute(p, ROOT, "just", env={}, runner=runner, timings=timings), 0)
            again = smart_ci.Timings.load(path)
            self.assertEqual(again.data["native"], {SIM: {"sky::tests::glows": 1.25}, TESTKIT: {"kit": 0.01}})
            # The wasip1 run printed nothing: its listed test stays unknown, so the plan takes the lock.
            smart_ci.weigh(p, again)
            self.assertFalse(p.unlocked)
            self.assertIn("hyperion-sim sky::tests::glows on wasip1", p.weight)
            # A failed run records its passes and its doctests, but no browser suite, cut short.
            failed = smart_ci.Timings.load(path)
            failed.record(record, p.suites, complete=False)
            self.assertEqual(failed.data["browser"], {})
            self.assertEqual(failed.data["doctests"], {"hyperion_sim": 8.73})

    def test_doctests_keep_their_largest_time_and_a_crate_without_a_total(self) -> None:
        record = smart_ci.RunRecord([], {})
        for line in ("   Doc-tests hyperion_surface", "running 2 tests", "test result: ok. 2 passed; finished in 0.25s",
                     "running 1 test", "test result: ok. 1 passed; finished in 0.50s", "   Doc-tests hyperion_sim",
                     "all doctests ran in 3.00s; merged doctests compilation took 1.00s"):
            record.line(line)
        timings = smart_ci.Timings(None, {"doctests": {"hyperion_sim": 8.0}})
        timings.record(record, [], complete=True)
        # The surface crate printed no total: its tests' times, summed. The sim's run of some of its
        # doctests took less than its last: the larger is kept.
        self.assertEqual(timings.data["doctests"], {"hyperion_surface": 0.75, "hyperion_sim": 8.0})
        last = smart_ci.RunRecord([], {})
        for line in ("   Doc-tests hyperion_base", "test result: ok. 0 passed; finished in 0.00s"):
            last.line(line)
        timings.record(last, [], complete=True)  # the output ended with the crate's doctests
        self.assertEqual(timings.data["doctests"]["hyperion_base"], 0.0)

    def test_a_save_keeps_what_another_run_saved_meanwhile(self) -> None:
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / smart_ci.TIMINGS_FILE
            mine, theirs = smart_ci.Timings.load(path), smart_ci.Timings.load(path)
            for timings, line in ((mine, "PASS [ 1.000s] hyperion-sim a"), (theirs, "PASS [ 2.000s] hyperion-base b")):
                record = smart_ci.RunRecord(["native"], {})
                for text in ("Starting 1 test across 1 binary", line):
                    record.line(text)
                timings.record(record, [], complete=True)
            theirs.save()
            mine.save()
            self.assertEqual(smart_ci.Timings.load(path).data["native"], {SIM: {"a": 1.0}, BASE: {"b": 2.0}})

    def test_the_runner_streams_the_output_and_reads_it_to_its_end_when_interrupted(self) -> None:
        # The real `Runner.run`, on Python for `just`: each line to the console and to `watch`.
        runner = smart_ci.Runner(ROOT, sys.executable)
        code = "import sys; print('one'); print('two', file=sys.stderr); sys.exit(3)"
        seen: List[str] = []
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(runner.run(["-u", "-c", code], watch=seen.append), 3)  # unbuffered, as cargo's lines are
        self.assertEqual(seen, ["one\n", "two\n"])
        # An interrupt while it runs: its output read to its end, then the interrupt raised.
        lines: List[str] = []

        def interrupted(text: str) -> None:
            lines.append(text)
            if len(lines) == 1:
                raise KeyboardInterrupt

        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(KeyboardInterrupt):
            runner.run(["-u", "-c", "print('one'); print('two'); print('three')"], watch=interrupted)
        self.assertEqual(lines, ["one\n", "two\n", "three\n"])

    def test_a_complete_run_marks_what_it_did_not_run_there(self) -> None:
        # The plan lists `glows` on wasip1, where (say) it is compiled out: a complete run that does not
        # run it records it as not run there, and it no longer counts as unknown, or as a test.
        p = self.plan()
        timings = self.timings(wasip1={})
        record = smart_ci.RunRecord(["native", "wasip1"], {})
        for line in ("Starting 2 tests across 2 binaries", "PASS [ 2.000s] hyperion-sim sky::tests::glows",
                     "PASS [ 0.100s] hyperion-testkit kit", "Starting 0 tests across 1 binary"):
            record.line(line)
        timings.record(record, p.suites, complete=True)
        self.assertEqual(timings.data["wasip1"], {SIM: {"sky::tests::glows": None}})
        found = timings.weigh(p.suites)
        self.assertEqual((found.tests, found.unknown), (2, []))
        # Natively too; but not after a run that started more tests than it timed, or one that failed.
        for started, complete, marked in ((1, True, True), (2, True, False), (1, False, False)):
            with self.subTest(started=started, complete=complete):
                native = self.timings()
                lines = smart_ci.RunRecord(["native"], {})
                for line in (f"Starting {started} tests across 2 binaries", "PASS [ 0.100s] hyperion-testkit kit"):
                    lines.line(line)
                native.record(lines, p.suites, complete=complete)
                self.assertEqual(native.data["native"][SIM]["sky::tests::glows"], None if marked else 2.0)
                self.assertEqual(native.data["native"][TESTKIT], {"kit": 0.1})

    def test_terms_count_the_recorded_tests_they_match(self) -> None:
        suite = smart_ci.Suite("native", crates={PROTOCOL}, terms={PROTOCOL: {"test(/^cases::/)", "test(/^ts::export_bindings_x$/)"}},
                               filter="binary_id(=hyperion-protocol) & (test(/^cases::/) | test(/^ts::export_bindings_x$/))")
        timings = smart_ci.Timings(None, {"native": {PROTOCOL: {"cases::one": 1.0, "cases::two": 2.0, "other": 5.0}}})
        found = timings.weigh([suite])
        self.assertEqual(found.unknown, ["hyperion-protocol's tests test(/^ts::export_bindings_x$/)"])
        # nextest's default filter skips the export test: a complete run marks its term as matching nothing.
        record = smart_ci.RunRecord(["native"], {})
        for line in ("Starting 2 tests across 1 binary", "PASS [ 1.000s] hyperion-protocol cases::one",
                     "PASS [ 2.000s] hyperion-protocol cases::two"):
            record.line(line)
        timings.record(record, [suite], complete=True)
        self.assertIsNone(timings.data["native"][PROTOCOL]["test(/^ts::export_bindings_x$/)"])
        found = timings.weigh([suite])
        self.assertEqual((found.tests, found.seconds, found.unknown), (2, 3.0, []))

    def test_doctests_and_browser_suites_count_by_crate(self) -> None:
        suites = [smart_ci.Suite("doctests", crates={SIM}), smart_ci.Suite("browser", crates={SURFACE})]
        timings = smart_ci.Timings(None, {"doctests": {"hyperion_sim": 11.0}, "browser": {SURFACE: 71.0}})
        found = timings.weigh(suites)
        self.assertEqual((found.tests, found.seconds, found.unknown), (0, 82.0, []))
        self.assertEqual(smart_ci.Timings().weigh(suites).unknown,
                         ["hyperion-sim's doctests", "hyperion-surface's browser suite"])

    def test_the_timings_survive_a_bad_file(self) -> None:
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "t.json"
            for text in ("not json", '{"version": 0, "native": {"a": {"b": 1}}}', '{"version": 1, "native": []}'):
                path.write_text(text, encoding="utf-8")
                self.assertEqual(smart_ci.Timings.load(path).data["native"], {})
            path.write_text('{"version": 1, "native": {"hyperion-testkit": {"kit": "slow"}}}', encoding="utf-8")
            found = smart_ci.Timings.load(path).weigh([smart_ci.Suite("native", crates={TESTKIT}, tests={TESTKIT: {"kit"}})])
            self.assertEqual(found.tests, 0)
            # A binary's table that is not one is dropped, and a run records over it.
            path.write_text('{"version": 1, "native": {"hyperion-sim": 5}}', encoding="utf-8")
            timings = smart_ci.Timings.load(path)
            self.assertEqual(timings.data["native"], {})
            record = smart_ci.RunRecord(["native"], {})
            for line in ("Starting 1 test across 1 binary", "PASS [ 1.000s] hyperion-sim a"):
                record.line(line)
            timings.record(record, [], complete=True)
            timings.save()
            self.assertEqual(smart_ci.Timings.load(path).data["native"], {SIM: {"a": 1.0}})

    def test_the_nextest_runs_are_the_locked_steps(self) -> None:
        self.assertEqual(smart_ci.nextest_kinds(smart_ci.CI_PHASES["locked"]), ["native", "wasip1"])
        self.assertEqual(smart_ci.nextest_kinds(smart_ci.phases_of(self.plan())["locked"]), ["native", "wasip1"])
        self.assertEqual(smart_ci.nextest_kinds([["_wasm-run", "", "", "hyperion-base"], ["_doctest-run", "-p x", ""]]), [])
        self.assertEqual(smart_ci.WEIGHED | {"_test-render-run"},
                         {r[0] for r in smart_ci.phases_of(plan("crates/hyperion-surface/src/field.rs",
                                                                    "apps/hyperion/src/renderer/src/view/shaders/sky.wgsl"))["locked"]})


class TheNodeFloor(unittest.TestCase):
    """The Node floor's preflight (`tools/node-floor/check.cjs`), and smart-ci's run of it before a plan
    that runs vitest or tsc."""

    def test_a_plan_that_runs_vitest_or_tsc_needs_the_floor(self) -> None:
        self.assertTrue(smart_ci.needs_node_floor(plan("apps/hyperion/src/renderer/src/lib/format.ts")))
        self.assertTrue(smart_ci.needs_node_floor(plan("packages/protocol/src/hex.ts")))
        self.assertTrue(smart_ci.needs_node_floor(plan("Cargo.toml")))
        for path in ("crates/hyperion-server/src/main.rs", "README.md", ".oxlintrc.json"):
            with self.subTest(path=path):
                self.assertFalse(smart_ci.needs_node_floor(plan(path)))

    def test_smart_ci_runs_the_preflight_and_says_when_there_is_no_node(self) -> None:
        seen: List[List[str]] = []

        def older(command: List[str], cwd: str) -> "subprocess.CompletedProcess[bytes]":
            seen.append(command)
            self.assertEqual(cwd, str(ROOT))
            return subprocess.CompletedProcess(command, 1)

        self.assertFalse(smart_ci.node_floor_met(ROOT, older))
        self.assertEqual(seen, [["node", str(ROOT / "tools" / "node-floor" / "check.cjs")]])
        self.assertTrue(smart_ci.node_floor_met(ROOT, lambda command, cwd: subprocess.CompletedProcess(command, 0)))

        def missing(command: List[str], cwd: str) -> "subprocess.CompletedProcess[bytes]":
            raise FileNotFoundError(2, "No such file or directory", "node")

        with contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertFalse(smart_ci.node_floor_met(ROOT, missing))
        self.assertIn("no `node` runs", err.getvalue())

    @unittest.skipIf(shutil.which("node") is None, "no `node` on PATH")
    def test_the_preflight_stops_a_node_below_the_floor_in_one_line(self) -> None:
        version = subprocess.run(["node", "-p", "process.versions.node"], capture_output=True, text=True,
                                 check=True).stdout.strip()
        # A prerelease or nightly Node (26.1.0-pre) is compared by its numbers alone.
        running = version.split("-")[0]
        major = int(running.split(".")[0])
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "package.json"

            def check(node_range: str) -> "subprocess.CompletedProcess[str]":
                manifest.write_text(json.dumps({"engines": {"node": node_range}}), encoding="utf-8")
                return subprocess.run(["node", str(ROOT / smart_ci.NODE_FLOOR_CHECK), str(manifest)],
                                      capture_output=True, text=True)

            for node_range in (f">={running}", f">={major}.0.0", ">=0.0.0", f">= {major - 1}.99.99"):
                with self.subTest(range=node_range):
                    passed = check(node_range)
                    self.assertEqual((passed.returncode, passed.stdout, passed.stderr), (0, "", ""))
            older = check(f">={major + 1}.0.0")
            self.assertEqual(older.returncode, 1)
            self.assertEqual(older.stdout, "")
            self.assertEqual(older.stderr,
                             f"error: Node {version} is older than HYPERION's floor {major + 1}.0.0 (engines.node in "
                             f"package.json); install Node {major + 1} or newer\n")
            unread = check("^26.0.0")
            self.assertEqual(unread.returncode, 2)
            self.assertIn('engines.node is "^26.0.0"', unread.stderr)
            manifest.write_text("{", encoding="utf-8")
            broken = subprocess.run(["node", str(ROOT / smart_ci.NODE_FLOOR_CHECK), str(manifest)],
                                    capture_output=True, text=True)
            self.assertEqual(broken.returncode, 2)
            self.assertEqual(len(broken.stderr.splitlines()), 1, broken.stderr)

    def test_the_packages_run_it_before_vitest_and_tsc(self) -> None:
        for manifest in ("apps/hyperion/package.json", "packages/protocol/package.json"):
            scripts = json.loads((ROOT / manifest).read_text(encoding="utf-8"))["scripts"]
            for name in ("test", "typecheck"):
                with self.subTest(manifest=manifest, script=name):
                    self.assertTrue(scripts[name].startswith(f"node ../../{smart_ci.NODE_FLOOR_CHECK} && "),
                                    scripts[name])
        self.assertRegex(json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["engines"]["node"],
                         r"^>=\d+\.\d+\.\d+$")


class Parsing(unittest.TestCase):
    def test_name_status(self) -> None:
        text = "M\0a.rs\0D\0b.md\0A\0c d.md\0"
        self.assertEqual(smart_ci.parse_name_status(text), [("a.rs", False), ("b.md", True), ("c d.md", False)])

    def test_pnpm_workspace(self) -> None:
        text = "packages:\n  - apps/*\n  - 'packages/*'\nallowBuilds:\n  electron: true\n"
        self.assertEqual(smart_ci.pnpm_workspace_patterns(text), ["apps/*", "packages/*"])

    def test_format_check_excludes(self) -> None:
        text = json.dumps({"scripts": {"format:check": 'prettier --check . "!.claude/**" "!target/**"'}})
        self.assertEqual(smart_ci.format_check_excludes(text), [".claude/**", "target/**"])


# --------------------------------------------------------------------------------------------------
# Against the real justfile and workspace.


def just() -> str:
    return os.environ.get("SMART_CI_JUST", "just")


def dump() -> dict:
    out = subprocess.run([just(), "--dump", "--dump-format", "json"], cwd=str(ROOT), capture_output=True, text=True,
                         check=True)
    return json.loads(out.stdout)


def dry_run(*recipe: str) -> List[str]:
    """The lines of `just --dry-run <recipe>` that run `just`, as it echoes them."""
    out = subprocess.run([just(), "--dry-run", *recipe], cwd=str(ROOT), capture_output=True, text=True, check=True)
    return [line.strip() for line in out.stderr.splitlines() if line.strip().startswith("just ")]


class TheCallersJustSettings(unittest.TestCase):
    """The suite reads just as just prints by default, whatever `JUST_*` its caller sets."""

    def test_a_caller_that_timestamps_just_changes_nothing_it_reads(self) -> None:
        # Integration's runs set JUST_TIMESTAMP=true for every `just` smart-ci runs, `_smart-ci-test`
        # among them (perf-build Q4), and `dry_run` once read no line of `[HH:MM:SS] just …`; with
        # JUST_COLOR=always none of `\x1b[1mjust …` either. The module's setup takes them away.
        plain = dry_run("test-wasm-sky")
        self.assertTrue(plain)
        for caller in ({"JUST_TIMESTAMP": "true"}, {"JUST_COLOR": "always"}):
            with self.subTest(caller=caller), unittest.mock.patch.dict(os.environ, caller):
                self.assertEqual(dry_run("test-wasm-sky"), [],
                                 "the setting no longer changes what just echoes: pick one that does")
                with without_just_settings():
                    self.assertEqual(dry_run("test-wasm-sky"), plain)


def line_text(fragments: list) -> str:
    """A recipe line from the dump, its interpolations as `{{}}`."""
    return "".join(f if isinstance(f, str) else "{{}}" for f in fragments)


def arg_text(arg) -> str:
    """A dependency's argument: a string, or a variable's name."""
    if isinstance(arg, str):
        return arg
    if isinstance(arg, list) and len(arg) == 2 and arg[0] == "variable":
        return arg[1]
    return json.dumps(arg)


def max_args(recipe: dict) -> Optional[int]:
    """How many arguments the recipe takes at most; None if variadic."""
    if any(p["kind"] in ("plus", "star") for p in recipe["parameters"]):
        return None
    return len(recipe["parameters"])


def min_args(recipe: dict) -> int:
    return sum(1 for p in recipe["parameters"] if p["default"] is None and p["kind"] != "star")


def just_calls(line: str, recipes: Dict[str, dict]) -> List[Tuple[str, List[str]]]:
    """The recipes one shell line runs through `just`, with their arguments, as just groups them: each
    recipe takes as many of the words after it as it has parameters, a variadic one all of them, and a
    command given to a variadic recipe that is itself `just ...` is followed."""
    out: List[Tuple[str, List[str]]] = []
    for match in re.finditer(r"(?:^|[;&|(]\s*|\s)just\s+([^;&|>]*)", line):
        try:
            words = shlex.split(match.group(1))
        except ValueError:
            words = match.group(1).split()
        out += group(words, recipes)
    return out


def group(words: List[str], recipes: Dict[str, dict]) -> List[Tuple[str, List[str]]]:
    out: List[Tuple[str, List[str]]] = []
    i = 0
    while i < len(words):
        name = words[i]
        if name not in recipes:
            raise AssertionError(f"`just {' '.join(words)}`: {name} is not a recipe")
        most = max_args(recipes[name])
        taken = words[i + 1:] if most is None else words[i + 1:i + 1 + most]
        out.append((name, taken))
        if most is None and taken[:1] == ["just"]:
            out += group(taken[1:], recipes)
        i += 1 + len(taken)
    return out


def step_calls(recipe: dict, recipes: Dict[str, dict]) -> List[Tuple[str, Optional[List[str]]]]:
    """A recipe's dependencies, with their arguments, then the recipes its body runs through `just`."""
    deps = [(d["recipe"], [arg_text(a) for a in d["arguments"]]) for d in recipe["dependencies"]]
    body = [(name, None) for fragments in recipe["body"] for name, _ in just_calls(line_text(fragments), recipes)]
    return deps + body


def ci_steps(recipes: Dict[str, dict]) -> List[str]:
    """Every recipe `just ci` runs: its dependencies and the recipes its body runs through `just`."""
    ci = recipes["ci"]
    steps = [d["recipe"] for d in ci["dependencies"]]
    for fragments in ci["body"]:
        steps += [name for name, _ in just_calls(line_text(fragments), recipes)]
    return list(dict.fromkeys(steps))


class AgreesWithCi(unittest.TestCase):
    """smart-ci runs `just ci`'s own steps, and knows every one of them."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.dump = dump()
        cls.recipes = cls.dump["recipes"]

    def test_every_step_of_ci_is_one_smart_ci_knows(self) -> None:
        steps = ci_steps(self.recipes)
        unknown = [s for s in steps if s not in smart_ci.CI_STEPS]
        self.assertEqual(unknown, [], "`just ci` runs steps that smart-ci does not know: add each to CI_STEPS in "
                                      "tools/smart-ci/smart_ci.py, with the rule that runs it, so that smart-ci "
                                      "runs it when a change needs it")
        gone = [s for s in smart_ci.CI_STEPS if s not in steps]
        self.assertEqual(gone, [], "CI_STEPS names steps that `just ci` no longer runs")

    def test_each_step_calls_what_smart_ci_expects(self) -> None:
        for step, (how, expected) in smart_ci.CI_STEPS.items():
            with self.subTest(step=step):
                actual = step_calls(self.recipes[step], self.recipes)
                names = [name for name, _ in actual]
                self.assertEqual(sorted(set(names)), sorted({name for name, _ in expected}),
                                 f"`{step}` calls other recipes than CI_STEPS says: update smart-ci with it")
                for name, wanted in expected:
                    if wanted is not None:
                        self.assertIn((name, wanted), actual, f"`{step}` calls {name} with other arguments")
                if how == "scoped":
                    # The whole step is its scoped recipes: no command of its own that smart-ci would miss.
                    lines = [line_text(f).strip() for f in self.recipes[step]["body"]]
                    own = [line for line in lines if line and not re.match(r"^just\s", line)]
                    self.assertEqual(own, [], f"`{step}` runs commands of its own besides its scoped recipes")

    def test_the_full_plans_phases_are_cis(self) -> None:
        # `ci`'s dependencies are its setup; its body backgrounds the side steps and then vitest, each
        # a job of its own, runs the cargo steps, then the Rust suites under `_locked`.
        ci = self.recipes["ci"]
        setup = [[d["recipe"], *(arg_text(a) for a in d["arguments"])] for d in ci["dependencies"]]
        phases: Dict[str, List[List[str]]] = {"setup": setup, "side": [], "unlocked": [], "main": [], "locked": []}
        jobs = iter(["side", "unlocked"])
        for fragments in ci["body"]:
            line = line_text(fragments).strip()
            if not re.match(r"^just\s", line):
                continue
            called = [[name, *taken] for name, taken in just_calls(line, self.recipes)]
            if called[0][0] == "_locked":
                phases["locked"] += [call for call in called[1:] if call[0] != "_locked"]
            elif re.search(r"&\s*$", line):
                phases[next(jobs)] += called
            else:
                phases["main"] += called
        self.assertEqual(phases, smart_ci.CI_PHASES, "`ci`'s phases differ from CI_PHASES in "
                                                    "tools/smart-ci/smart_ci.py: update it with them")
        self.assertEqual(list(smart_ci.CI_PHASES), list(smart_ci.PHASES))

    def test_test_renders_steps_are_the_recipes(self) -> None:
        # `test-render` is `gen-surface`, the client's build, then the harness under `_locked`: the
        # steps a plan runs in its own phases (RENDER_PHASES), its run in the suites' hold.
        recipe = self.recipes["test-render"]
        deps = [[d["recipe"], *(arg_text(a) for a in d["arguments"])] for d in recipe["dependencies"]]
        self.assertEqual(deps, smart_ci.RENDER_PHASES["setup"] + smart_ci.RENDER_PHASES["side"])
        body = [just_calls(line_text(f).strip(), self.recipes) for f in recipe["body"]]
        self.assertEqual([[name for name, _ in calls_] for calls_ in body],
                         [["_locked"] + [r[0] for r in smart_ci.RENDER_PHASES["locked"]]])
        self.assertEqual(max_args(self.recipes["_test-render-run"]), None, "it takes test-render's arguments")
        for phase in smart_ci.RENDER_PHASES:
            self.assertIn(phase, smart_ci.PHASES)

    def test_every_recipe_says_whether_it_builds(self) -> None:
        recipes = set(smart_ci.CI_STEPS) | set(smart_ci.NARROWER.values()) | set(smart_ci.SELF_BUILT.values())
        recipes |= RENDER_RECIPES
        recipes |= {name for _, calls_ in smart_ci.CI_STEPS.values() for name, _ in calls_}
        self.assertEqual(recipes - smart_ci.BUILDS - smart_ci.NO_BUILDS, set(),
                         "say in BUILDS or NO_BUILDS (tools/smart-ci/smart_ci.py) whether each runs a cargo build, "
                         "which holds a build slot")
        self.assertEqual(smart_ci.BUILDS & smart_ci.NO_BUILDS, set())
        # A recipe said not to build runs no cargo but formatting and tool checks, unless it runs only
        # under the heavy-test lock, from builds made before it.
        locked = {name for recipe in smart_ci.CI_PHASES["locked"] + smart_ci.RENDER_PHASES["locked"]
                  for name in recipe} | {"_locked"}
        locked |= {name for step in locked if step in smart_ci.CI_STEPS for name, _ in smart_ci.CI_STEPS[step][1]}
        for name in sorted(smart_ci.NO_BUILDS - locked):
            with self.subTest(recipe=name):
                body = "\n".join(line_text(f) for f in self.recipes[name]["body"])
                self.assertFalse(re.search(r"\bcargo\s+(?!fmt\b|nextest --version)", body), body)

    def test_no_cargo_runs_in_the_locked_browser_step(self) -> None:
        # perf-wasm W3: `_browser run`, under the heavy-test lock, runs the binaries `prepare` built,
        # through the runner, and no cargo, which could wait there for another build's lock.
        body = "\n".join(line_text(f) for f in self.recipes["_browser"]["body"])
        run = re.search(r"^\s*run\)\n(.*?)^\s*;;", body, re.M | re.S)
        self.assertIsNotNone(run, "the `run)` case of `_browser`")
        self.assertNotRegex(run.group(1), r"\bcargo\b")
        self.assertIn("wasm-bindgen-test-runner", run.group(1))

    def test_the_heavy_lock_gives_way_to_a_run_with_priority(self) -> None:
        # `_locked` itself, on a lock file of its own (HYPERION_HEAVY_LOCK), never the real one.
        with tempfile.TemporaryDirectory() as directory:
            lock = os.path.join(directory, "heavy.lock")
            priority = os.path.join(directory, "heavy.priority")
            env = {**os.environ, "HYPERION_HEAVY_LOCK": lock}
            env.pop("HYPERION_HEAVY_PRIORITY", None)
            env.pop("HYPERION_HEAVY_HELD", None)

            def locked(*command: str, priority_run: bool = False) -> "subprocess.Popen[str]":
                return subprocess.Popen([just(), "_locked", *command], cwd=str(ROOT), text=True,
                                        env={**env, "HYPERION_HEAVY_PRIORITY": "1"} if priority_run else env,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                        start_new_session=os.name != "nt")
            # With priority: the file names it while it waits, and is gone, as is the variable, once it holds the lock;
            # and the command is told that it holds the lock (HYPERION_HEAVY_HELD, which build-slot reads).
            check = (f'test ! -e "{priority}" && test -z "${{HYPERION_HEAVY_PRIORITY:-}}"'
                     ' && test "${HYPERION_HEAVY_HELD:-}" = 1')
            run = locked("bash", "-c", check, priority_run=True)
            self.assertEqual(run.wait(timeout=60), 0, run.stdout.read() if run.stdout else "")
            self.assertFalse(os.path.exists(priority))
            # A file of an ended process, or one 5 minutes old, is ignored: the lock is taken at once.
            ended = subprocess.Popen([sys.executable, "-c", "pass"])
            ended.wait()
            for pid, age in ((ended.pid, 0), (os.getpid(), 600)):
                with self.subTest(pid=pid, age=age):
                    Path(priority).write_text(f"{pid}\n", encoding="utf-8")
                    if age:
                        os.utime(priority, (time.time() - age, time.time() - age))
                    run = locked("true")
                    self.assertEqual(run.wait(timeout=60), 0)
            if os.name == "nt":
                return  # no fcntl, no process groups to kill
            # While the lock is held, a run with priority names itself in the file and takes the lock
            # at its release, removing the file.
            import fcntl
            Path(priority).unlink(missing_ok=True)
            with open(lock, "w") as held:
                fcntl.flock(held, fcntl.LOCK_EX)
                run = locked("true", priority_run=True)
                deadline = time.monotonic() + 30
                while not os.path.exists(priority) and time.monotonic() < deadline:
                    time.sleep(0.1)
                named = Path(priority).read_text(encoding="utf-8").strip()
                self.assertTrue(named.isdigit() and named != str(os.getpid()), named)
                with self.assertRaises(subprocess.TimeoutExpired):
                    run.wait(timeout=1)
                fcntl.flock(held, fcntl.LOCK_UN)
            self.assertEqual(run.wait(timeout=60), 0)
            self.assertIn("waiting, with priority", run.communicate()[0])
            self.assertFalse(os.path.exists(priority))
            # Ended while it waits (it blocks on the lock in the background, so a signal ends it at
            # once), it removes the file and leaves nothing waiting.
            with open(lock, "w") as held:
                fcntl.flock(held, fcntl.LOCK_EX)
                run = locked("true", priority_run=True)
                deadline = time.monotonic() + 30
                while not os.path.exists(priority) and time.monotonic() < deadline:
                    time.sleep(0.1)
                self.assertTrue(os.path.exists(priority))
                os.killpg(run.pid, signal.SIGTERM)
                run.wait(timeout=10)
                deadline = time.monotonic() + 10
                while os.path.exists(priority) and time.monotonic() < deadline:
                    time.sleep(0.1)
                self.assertFalse(os.path.exists(priority), "the priority file outlived its run")
                fcntl.flock(held, fcntl.LOCK_UN)
            # Nothing of the ended run takes the lock afterwards.
            with open(lock, "w") as probe:
                fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
            # A live run with priority: another steps aside, though the lock is free.
            Path(priority).write_text(f"{os.getpid()}\n", encoding="utf-8")
            run = locked("true")
            try:
                with self.assertRaises(subprocess.TimeoutExpired):
                    run.wait(timeout=3)
            finally:
                os.killpg(run.pid, signal.SIGKILL)
                output = run.communicate()[0]
            self.assertIn(f"stepping aside for the run with priority ({os.getpid()})", output)

    def test_the_heavy_runs_scope_caps_its_cpu(self) -> None:
        # `_locked`'s scope has 12 cores by default and `HEAVY_CPU_QUOTA`'s share when it is set, so
        # that a heavy run leaves the desktop and I/O completion some (the freeze of 2026-10-13); on
        # a lock file of its own, never the real one. Where systemd cannot make a user scope (macOS,
        # Windows) the run is uncapped, and there is nothing to read.
        if shutil.which("systemd-run") is None or subprocess.run(
                ["systemd-run", "--user", "--scope", "--quiet", "true"], capture_output=True).returncode != 0:
            self.skipTest("no systemd user scopes here")
        show = 'cat "/sys/fs/cgroup$(sed -n "s/^0:://p" /proc/self/cgroup)/cpu.max"'
        with tempfile.TemporaryDirectory() as directory:
            env = {**os.environ, "HYPERION_HEAVY_LOCK": os.path.join(directory, "heavy.lock")}
            for name in ("HYPERION_HEAVY_PRIORITY", "HYPERION_HEAVY_HELD", "HEAVY_CPU_QUOTA"):
                env.pop(name, None)
            for quota, wanted in ((None, "1200000 100000"), ("300%", "300000 100000")):
                with self.subTest(quota=quota):
                    run = subprocess.run([just(), "_locked", "bash", "-c", show], cwd=str(ROOT), capture_output=True,
                                         text=True, timeout=60,
                                         env={**env, "HEAVY_CPU_QUOTA": quota} if quota else env)
                    self.assertEqual(run.returncode, 0, run.stderr)
                    self.assertEqual(run.stdout.strip(), wanted)

    def test_the_narrower_prettier_runs_the_check_format_check_runs(self) -> None:
        script = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["scripts"]["format:check"]
        self.assertTrue(script.startswith("prettier --check "), script)
        for wide, narrow in smart_ci.NARROWER.items():
            self.assertIn(wide, self.recipes)
            body = " ".join(line_text(f) for f in self.recipes[narrow]["body"])
            self.assertIn("prettier --check", body)

    def test_the_planned_recipes_exist_and_take_their_arguments(self) -> None:
        # Every plan of the rule tests, and one with everything at once.
        cases = [("crates/hyperion-surface/src/wasm.rs", "crates/hyperion-protocol/src/lib.rs",
                  "apps/hyperion/src/renderer/src/lib/format.ts", "tools/gpu-replay/src/main.rs", "README.md",
                  "crates/hyperion-sim/src/tables/mge.rs", "tools/smart-ci/smart_ci.py", "clippy.toml"),
                 ("docs/agent/plans/a plan.md", "tools/electron-node/node", "package.json",
                  "apps/hyperion/src/renderer/src/view/shaders/sky.wgsl"),
                 ("docs/measurements/descent-spike/2026-10-07-effect-low.json", "crates/hyperion-fit/manifests/mge.toml")]
        known = set(smart_ci.CI_STEPS) | {name for _, calls_ in smart_ci.CI_STEPS.values() for name, _ in calls_}
        known |= set(smart_ci.NARROWER.values()) | set(smart_ci.SELF_BUILT.values()) | RENDER_RECIPES
        plans = [plan(*paths, dependents=dependents) for paths in cases for dependents in (False, True)]
        plans += [plan(*paths, all_tests=True) for paths in cases]
        # And the plans with the code's graph, whose steps carry filters.
        graph_mode = GraphMode()
        plans += [graph_mode.plan_change("crates/hyperion-base/src/lib.rs", "{ 1 }", "{ 4 }"),
                  graph_mode.plan_change("crates/hyperion-sim/src/sky/mod.rs", "* 3", "* 4")]
        for p in plans:
            for phase in smart_ci.PHASES:
                words = [w for s in p.steps if s.phase == phase for w in s.recipe]
                grouped = group(words, self.recipes) if words else []
                self.assertEqual([g[0] for g in grouped], [s.recipe[0] for s in p.steps if s.phase == phase],
                                 f"the {phase} phase does not parse into its steps: {words}")
                for step in (s for s in p.steps if s.phase == phase):
                    recipe = self.recipes[step.recipe[0]]
                    given = len(step.recipe) - 1
                    most = max_args(recipe)
                    self.assertGreaterEqual(given, min_args(recipe), step.recipe)
                    if most is not None:
                        self.assertEqual(given, most, f"{step.recipe}: give every parameter, so that the phase "
                                                      "parses into its steps")
                    else:
                        self.assertIs(step, [s for s in p.steps if s.phase == phase][-1],
                                      f"{step.recipe[0]} is variadic, so it must end its phase")
                self.assertLessEqual({s.recipe[0] for s in p.steps}, known,
                                     "smart-ci runs only `ci`'s steps, and `test-render`'s")

    def test_the_crate_lists_are_the_justfiles(self) -> None:
        assignments = self.dump["assignments"]
        self.assertTrue(assignments["wasip1_crates"]["value"].startswith("-p "))
        self.assertTrue(assignments["browser_crates"]["value"])


# A string that names a file by its extension (`Cargo.toml`, `.golden`), for the source walkers' guard.
NAMES_A_FILE = re.compile(r"\.(\w{1,8})$")


def unlisted_source_readers(graph: rust_graph.Graph) -> List[str]:
    """The source walkers' guard: the tests ("unit test") that most likely read the Rust sources but
    reach no source walker, so that smart-ci runs them only for a change their code reaches. Such a
    test reaches a listing of a directory (`Graph._lists_directory`), and the code on its way there,
    the test's own function, the listing and every item between, names a crate's sources: `src`, a
    string ending `/src`, `crates`, another workspace crate's directory name, or `../<crate dir>`.
    Two narrowings, by rule: a listing that keeps files by another name (`Cargo.toml`) and holds no
    `rs` lists something else, and does not count; and the test's own crate's directory name is how
    the crate names itself (the server's program name, `hyperion-server`, in its tests' arguments),
    while a test that lists its own crate's sources names `src`."""
    crate_dirs = {posixpath.basename(info.dir) for info in graph.crates.values()}

    def names_sources(literal: str, own: str) -> bool:
        return literal in ("src", "crates") or literal.endswith("/src") or literal in crate_dirs - {own} or any(
            literal == f"../{d}" or literal.startswith(f"../{d}/") for d in crate_dirs)

    def keeps_other_files(literals: Sequence[str]) -> bool:
        found = [NAMES_A_FILE.search(literal) for literal in literals]
        return not any(literal == "rs" or literal.endswith(".rs") for literal in literals) and any(
            m is not None and m.group(1) != "rs" for m in found)

    literals = {rust_graph.item_node(file, item[0]): facts.item_literals[index]
                for file, facts in graph.rs.items() for index, item in enumerate(facts.items)}
    listings = {rust_graph.item_node(file, item[0]) for file, facts in graph.rs.items()
                for index, item in enumerate(facts.items)
                if graph._lists_directory(facts, index) and not keeps_other_files(facts.item_literals[index])}
    reverse: Dict[str, Set[str]] = {}
    for source, targets in graph.edges.items():
        for target in targets:
            reverse.setdefault(target, set()).add(source)
    toward = set(listings)  # every node whose code reaches a listing
    frontier = list(toward)
    while frontier:
        for source in reverse.get(frontier.pop(), ()):
            if source not in toward:
                toward.add(source)
                frontier.append(source)
    listers, _ = graph.tests_reaching(listings, reverse)
    walkers, _ = graph.source_readers(reverse)
    nodes = {(unit, t.name): t.node for unit, tests in graph.tests.items() for t in tests}
    unseen: List[str] = []
    for unit, names in listers.items():
        for name in names:
            if name in walkers.get(unit, {}):
                continue
            own = posixpath.basename(graph.crates[graph.units[unit].crate].dir)
            way = {nodes[(unit, name)]}
            frontier = list(way)
            while frontier:
                for target in graph.edges.get(frontier.pop(), ()):
                    if target in toward and target not in way:
                        way.add(target)
                        frontier.append(target)
            if any(names_sources(s, own) for node in way for s in literals.get(node, ())):
                unseen.append(f"{unit} {name}")
    return sorted(unseen)


# The guard's message: the two ways out.
UNLISTED_SOURCE_READERS = (
    "these tests list a directory, and the code on their way to the listing names a crate's sources, but they "
    "reach no source walker, so smart-ci runs them only for a change their code reaches: if one reads the Rust "
    "sources, name the `rs` extension in the function that lists the directory (`extension() == \"rs\"` or "
    "`ends_with(\".rs\")`); if it reads none, narrow this guard's test of what reads them "
    "(`unlisted_source_readers`), rather than list the test here")


class TheSourceWalkersGuard(unittest.TestCase):
    """The guard (`unlisted_source_readers`) along the chain, on a test modelled on the testkit's
    `no_target_feature_attributes`: its strings name the crates it lists, not `src`, and those that
    name `src` are elsewhere or nowhere."""

    def fixture(self, keep: str) -> test_rust_graph.Fixture:
        """A testkit whose `clippy_bans` lists the sources of base and the sim, its walker keeping the
        `.rs` files by `keep`; and a listing of the crates' manifests."""
        return test_rust_graph.Fixture([test_rust_graph.crate("base"), test_rust_graph.crate("sim"),
                                        test_rust_graph.crate("testkit", tests=["clippy_bans"])], {
            "crates/base/src/lib.rs": "",
            # A listing of data, and a test that runs the sim by its own crate's name, as the server's
            # tests run it by its program's.
            "crates/sim/src/lib.rs": "pub fn scan(dir: &std::path::Path) -> usize { std::fs::read_dir(dir).unwrap().count() }\n"
                                     "#[cfg(test)]\nmod tests {\n    #[test]\n    fn runs_by_its_name() {\n"
                                     "        let _args = [\"sim\", \"--data\"];\n"
                                     "        super::scan(std::path::Path::new(\"universes\"));\n    }\n}\n",
            "crates/testkit/src/lib.rs": "",
            "crates/testkit/tests/clippy_bans.rs": (
                "use std::path::{Path, PathBuf};\nconst EXT: &str = \"rs\";\n"
                "const CRATES: [&str; 2] = [\"base\", \"sim\"];\n"
                "fn workspace_root() -> PathBuf {\n"
                "    Path::new(env!(\"CARGO_MANIFEST_DIR\")).parent().and_then(Path::parent).unwrap().to_path_buf()\n}\n"
                "fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {\n"
                "    for entry in std::fs::read_dir(dir).unwrap() {\n"
                "        let path = entry.unwrap().path();\n"
                "        if path.is_dir() { rust_files(&path, files); }\n"
                f"        else if path.extension().is_some_and(|e| e == {keep}) {{ files.push(path); }}\n    }}\n}}\n"
                "#[test]\nfn no_target_feature_attributes() {\n    let mut files = Vec::new();\n"
                "    for name in CRATES { rust_files(&workspace_root().join(\"crates\").join(name), &mut files); }\n}\n"
                "fn workspace_crates() -> Vec<String> {\n"
                "    std::fs::read_dir(workspace_root().join(\"crates\")).unwrap()\n"
                "        .map(|e| e.unwrap().path()).filter(|p| p.join(\"Cargo.toml\").is_file())\n"
                "        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect()\n}\n"
                "#[test]\nfn every_crate_has_a_manifest() { assert!(!workspace_crates().is_empty()); }\n"),
        })

    def test_a_walker_that_keeps_rs_by_a_constant_is_no_walker_and_its_test_is_flagged(self) -> None:
        graph = self.fixture("EXT").graph()
        self.assertEqual(graph.source_walkers(), set())
        # Its `crates`, on the test's own way to the listing, names the sources; the manifests'
        # listing keeps `Cargo.toml` and holds no `rs`, so its test is not flagged.
        self.assertEqual(unlisted_source_readers(graph), ["testkit::clippy_bans no_target_feature_attributes"])

    def test_a_walker_that_names_rs_is_one_and_nothing_is_flagged(self) -> None:
        graph = self.fixture("\"rs\"").graph()
        self.assertEqual(graph.source_walkers(), {"crates/testkit/tests/clippy_bans.rs#fn::rust_files"})
        self.assertEqual(unlisted_source_readers(graph), [])


@functools.lru_cache(maxsize=None)
def real_workspace() -> Workspace:
    """The real workspace, loaded once for the classes that read it."""
    return smart_ci.load_workspace(ROOT, just())


@functools.lru_cache(maxsize=None)
def real_graph() -> rust_graph.Graph:
    """The real workspace's code graph, built once."""
    return smart_ci.choose_tests([], real_workspace()).graph


class RealWorkspace(unittest.TestCase):
    """The names smart-ci builds on exist, and the real workspace loads."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.ws = real_workspace()

    def test_the_named_crates_and_files_exist(self) -> None:
        crates = set(self.ws.crates)
        named = {smart_ci.SURFACE_CRATE, smart_ci.PROTOCOL_CRATE, *smart_ci.FIT_CHECK_CRATES,
                 *(reader for _, reader, _, _ in smart_ci.RUST_READERS)}
        self.assertLessEqual(named, crates)
        self.assertLessEqual(set(self.ws.wasip1) | set(self.ws.browser), crates)
        for path in (smart_ci.GENERATOR_VERSION_FILE, smart_ci.CATALOGUE, smart_ci.NODE_FLOOR_CHECK):
            self.assertIn(path, self.ws.files)
        self.assertTrue(any(self.ws.read(f) and smart_ci.EXPORTS.search(self.ws.read(f))
                            for f in self.ws.files if f.startswith(f"crates/{smart_ci.SURFACE_CRATE}/src/")))
        self.assertTrue(self.ws.catalogue_sources)
        self.assertIn("apps/hyperion", self.ws.ts_packages)
        self.assertIn(smart_ci.FIXTURES.rstrip("/").rsplit("/", 1)[0], self.ws.ts_packages)
        for prefix in list(smart_ci.FULL_CI_PREFIXES) + [smart_ci.BINDINGS, smart_ci.FIXTURES]:
            self.assertTrue(any(f.startswith(prefix) for f in self.ws.files), prefix)
        for path in smart_ci.FULL_CI_ROOT:
            self.assertIn(path, self.ws.files)

    def test_the_browser_lists_default_to_the_features_the_workspace_gives(self) -> None:
        # `_browser`'s native lists, as `ci` runs them, link the libraries `--workspace` built (the
        # features of registry crates, which `unified_features` does not model, still differ).
        dumped = dump()
        defaults = {p["name"]: p["default"] for p in dumped["recipes"]["_browser"]["parameters"]}
        self.assertEqual(defaults["crates"], ["variable", "browser_crates"])
        self.assertEqual(defaults["native"], ["variable", "browser_native"])
        self.assertEqual(dumped["assignments"]["browser_native"]["value"], self.ws.cargo_packages(self.ws.browser),
                         "set `browser_native` in the justfile to `browser_crates` with the features `--workspace` "
                         "gives them")

    def test_the_guard_finds_cross_clippys_crates_and_no_others(self) -> None:
        # decision-test-coverage.md L4's guard: a crate whose code differs between Linux, macOS and
        # Windows is in CROSS_CLIPPY_CRATES, which smart-ci's cross-Clippy lints, and a crate there has
        # such code (the guard keeps the list honest).
        found = {crate: smart_ci.os_dependence(self.ws, crate) for crate in sorted(self.ws.crates)}
        unlisted = {crate: why for crate, why in found.items() if why and crate not in smart_ci.CROSS_CLIPPY_CRATES}
        self.assertEqual(unlisted, {}, "these crates have code that differs between Linux, macOS and Windows: add "
                                       "each to CROSS_CLIPPY_CRATES in tools/smart-ci/smart_ci.py")
        stale = sorted(crate for crate in smart_ci.CROSS_CLIPPY_CRATES if not found.get(crate))
        self.assertEqual(stale, [], "CROSS_CLIPPY_CRATES lists crates with no code that differs between the "
                                    "platforms (or no longer in the workspace): drop them")

    def test_the_desktop_cfgs_are_rustcs(self) -> None:
        # The guard's three targets are `cross-clippy`'s, with the cfg values rustc gives them.
        self.assertEqual(sorted(dump()["assignments"]["cross_targets"]["value"].split()),
                         sorted(smart_ci.DESKTOP_CFGS))
        for triple, text in smart_ci.DESKTOP_CFGS.items():
            printed = subprocess.run(["rustc", "--print", "cfg", "--target", triple], cwd=str(ROOT),
                                     capture_output=True, text=True, check=True).stdout.split()
            words = sorted(w for w in printed if smart_ci._is_target_option(w.partition("=")[0]))
            self.assertEqual(words, sorted(text.split()), f"update DESKTOP_CFGS[{triple!r}] in smart_ci.py from "
                                                          f"`rustc --print cfg --target {triple}`")

    def test_the_surface_module_has_readers(self) -> None:
        graph = smart_ci.TsGraph(self.ws)
        self.assertTrue(graph.surface_readers())

    def test_a_real_single_crate_plan(self) -> None:
        p = make_plan([("crates/hyperion-server/src/main.rs", False)], self.ws, False)
        self.assertFalse(p.is_full)
        self.assertNotIn("_vitest-run", calls(p))
        self.assertEqual(packages(p, "_clippy-for"), [SERVER])

    def test_the_real_graph(self) -> None:
        # The golden reader the graph names exists, and the workspace reads into a graph: its units,
        # its tests, every file in a module tree.
        for _, reader in smart_ci.GOLDEN_READERS:
            self.assertIn(reader, self.ws.files)
        graph = real_graph()
        self.assertTrue(any(u.is_lib for u in graph.units.values()))
        self.assertGreater(sum(len(t) for t in graph.tests.values()), 1000)
        self.assertEqual(graph.missing_mods, [], "a `mod` declaration names no file the graph knows")
        self.assertEqual([f for f in graph.orphans if "/src/" in f], [], "sources in no module tree: they widen")

    def test_a_test_that_lists_a_crates_sources_reaches_a_source_walker(self) -> None:
        # The source walkers' guard: a test whose code on its way to a listing of a directory names a
        # crate's sources most likely reads them, and smart-ci runs it for a Rust change only if its
        # code reaches a source walker (`unlisted_source_readers`).
        self.assertEqual(unlisted_source_readers(real_graph()), [], UNLISTED_SOURCE_READERS)

    def test_the_real_graph_finds_source_walkers_and_a_sim_source_runs_their_tests_natively(self) -> None:
        graph = real_graph()
        self.assertTrue(graph.source_walkers())
        readers, _ = graph.source_readers()
        self.assertTrue(readers)
        p = make_plan([("crates/hyperion-sim/src/lib.rs", False)], self.ws, False)
        native = smart_ci.Filter(args(p, "_nextest-run")[1] or "all()")
        self.assertTrue(any(native.matches(graph.units[unit].crate, unit, name)
                            for unit, names in readers.items() for name in names))
        self.assertEqual(next(s for s in p.steps if s.recipe[0] == "_nextest-run").causes[0], smart_ci.SOURCE_READERS)

    def test_the_real_readers_that_compute_their_directory_read_every_crate(self) -> None:
        # The scope's motivating cases: the surface crate's tag check, whose `src` names its own file
        # while it lists every crate from a directory it climbs to, and the testkit's ban on
        # `target_feature`, which lists the crates it names under `crates/`. Any narrower scope holds
        # its reader's own crate.
        graph = real_graph()
        readers, chains = graph.source_readers()
        scopes = {(unit, name): graph.source_scope(unit, name, chains[f"{unit} {name}"][0])
                  for unit, names in readers.items() for name in names}
        every = {key for key, (crates, _) in scopes.items() if crates is None}
        self.assertTrue(any(unit == SURFACE and name.startswith("tags::") for unit, name in every), scopes)
        self.assertTrue(any(unit == f"{TESTKIT}::clippy_bans" for unit, _ in every), scopes)
        for (unit, name), (crates, _) in scopes.items():
            if crates is not None:
                self.assertIn(graph.units[unit].crate, crates, (unit, name))


# --------------------------------------------------------------------------------------------------
# The justfile's `wasm_sky`, the tests that `ci`'s wasip1 run leaves to `just test-wasm-sky`: none of
# them pins a value on every target (decision-test-coverage.md §3.3, W-R1, and its addendum). Every
# such pin stays in `ci` on both targets.


# nextest's filter expressions, as smart-ci reads them (`smart_ci.Filter`).
Filter = smart_ci.Filter


# What pins a value on every target: a golden, a float's bits written as a literal, a constant named
# as a pin (`*_BITS`, `*FINGERPRINT*`, `*_DIGEST`), an assertion that a digest is a constant, or one
# against a hexadecimal literal (bits, a digest or an id written out).
PIN_CONSTANT = re.compile(r"[A-Z0-9_]*FINGERPRINT[A-Z0-9_]*|[A-Z0-9_]+_(?:BITS|DIGEST)")
ASSERTS = {"assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne"}
DIGEST_CALL = re.compile(r"\w*digest")
CONSTANT_NAME = re.compile(r"[A-Z][A-Z0-9_]*")
Token = Tuple[str, str]


def unfolded(tokens: Sequence[Token]) -> List[Token]:
    """The tokens with each run of numbers that `rust_graph.lex` reads as one (`1, 2, `) split into its
    numbers and commas."""
    out: List[Token] = []
    for kind, value in tokens:
        if kind == "n" and "," in value:
            for number in value.split(",")[:-1]:  # the run ends with a comma
                out += [("n", number.strip()), ("p", ",")]
        else:
            out.append((kind, value))
    return out


def closing(tokens: Sequence[Token], at: int) -> int:
    """The index of the bracket that closes the one at `at`; the end if none does."""
    depth = 0
    for j in range(at, len(tokens)):
        if tokens[j][0] == "p" and tokens[j][1] in "([{":
            depth += 1
        elif tokens[j][0] == "p" and tokens[j][1] in ")]}":
            depth -= 1
            if depth == 0:
                return j
    return len(tokens)


def top_level(tokens: Sequence[Token], *separators: Tuple[str, ...]) -> List[List[Token]]:
    """The tokens split at each separator (a run of punctuation) outside brackets."""
    parts: List[List[Token]] = [[]]
    depth = j = 0
    while j < len(tokens):
        kind, value = tokens[j]
        if kind == "p" and value in "([{":
            depth += 1
        elif kind == "p" and value in ")]}":
            depth -= 1
        found = next((s for s in separators if depth == 0 and tuple(v for _, v in tokens[j:j + len(s)]) == s
                      and all(k == "p" for k, _ in tokens[j:j + len(s)])), None)
        if found:
            parts.append([])
            j += len(found)
            continue
        parts[-1].append(tokens[j])
        j += 1
    return parts


def is_constant(operand: Sequence[Token]) -> bool:
    """A literal number, or a path to an UPPER_CASE constant (`&` or `-` before it)."""
    rest = list(operand)
    while rest[:1] in ([("p", "&")], [("p", "-")]):
        rest = rest[1:]
    if len(rest) == 1 and rest[0][0] == "n":
        return True
    if not rest or len(rest) % 2 == 0 or rest[-1][0] != "i" or not CONSTANT_NAME.fullmatch(rest[-1][1]):
        return False
    return all(t == ("p", "::") if i % 2 else t[0] == "i" for i, t in enumerate(rest))


def is_hex(operand: Sequence[Token]) -> bool:
    """A hexadecimal literal (`&` before it)."""
    rest = [t for t in operand if t != ("p", "&")]
    return len(rest) == 1 and rest[0][0] == "n" and rest[0][1].lower().startswith("0x")


def calls_digest(operand: Sequence[Token]) -> bool:
    return any(k == "i" and DIGEST_CALL.fullmatch(v) and operand[i + 1:i + 2] == [("p", "(")]
               for i, (k, v) in enumerate(operand))


def pin_markers(tokens: Sequence[Token]) -> List[str]:
    """What the tokens pin a value with, as the guard names it."""
    tokens = unfolded(tokens)
    found: Set[str] = set()
    for i, (kind, value) in enumerate(tokens):
        if kind != "i":
            continue
        after = [v for _, v in tokens[i + 1:i + 3]]
        if value == "golden" and after[:1] == ["!"]:
            found.add("golden!")
        elif value == "golden" and after[:1] == ["::"] and after[1:2] and after[1].startswith("check"):
            found.add(f"golden::{after[1]}")
        elif value == "check_golden":
            found.add(value)
        elif value == "from_bits" and after[:1] == ["("] and tokens[i + 2:i + 3] \
                and tokens[i + 2][0] == "n" and tokens[i + 2][1].lower().startswith("0x"):
            found.add("from_bits(0x…)")
        elif PIN_CONSTANT.fullmatch(value):
            found.add(value)
        elif value in ASSERTS and after == ["!", "("]:
            args = top_level(tokens[i + 3:closing(tokens, i + 2)], (",",))
            # `assert_eq!(a, b)`, or `assert!(a == b)` (the lexer reads `==` as two `=`).
            operands = args[:2] if value.endswith(("_eq", "_ne")) else top_level(args[0], ("=", "="), ("!", "="))
            if len(operands) == 2 and any(calls_digest(a) and is_constant(b)
                                          for a, b in (operands, operands[::-1])):
                found.add(f"{value}!(a digest, a constant)")
            if len(operands) == 2 and any(is_hex(operand) for operand in operands):
                found.add(f"{value}!(…, 0x…)")
    return sorted(found)


class WasmSkyScan:
    """The tests a filter selects in the code's graph (not ignored: the fast runs run no other), and
    what each pins a value with: in its own body, or in the test code it reaches through the graph's
    edges (a helper, a constant), wherever that is."""

    def __init__(self, graph: rust_graph.Graph, read: Callable[[str], str], wasm_sky: str) -> None:
        self.graph = graph
        self.read = read
        self.filter = Filter(wasm_sky)
        test_files = set().union(*(unit.test_files for unit in graph.units.values()))
        self.test_code = {rust_graph.item_node(path, item[0]) for path, facts in graph.rs.items()
                          for item in facts.items if item[5] or path in test_files}
        # The testkit's golden checks, its public `check…` functions and its macro, which a test may
        # call by a name its tokens do not show.
        golden = smart_ci.GOLDEN_READERS[0][1]
        facts = graph.rs.get(golden)
        self.golden_checks = {rust_graph.item_node(golden, item[0]) for item in (facts.items if facts else [])
                              if (item[1] == "fn" and item[3] == "pub" and item[2].startswith("check"))
                              or item[1] == "macro"}
        self.moved = [(unit, test) for unit in sorted(graph.units.values(), key=lambda u: u.id) if unit.runs
                      for test in graph.tests.get(unit.id, []) if not test.ignored
                      and self.filter.matches(unit.crate, unit.id, test.name)]
        self._tokens: Dict[str, Dict[str, List[Token]]] = {}

    def _walk(self, node: str, into: Callable[[str], bool]) -> Set[str]:
        """The nodes reached from `node` through the graph's edges, entering only those `into` takes,
        or an import or glob, which names an item without being one."""
        seen, todo = {node}, [node]
        while todo:
            for target in self.graph.edges.get(todo.pop(), ()):
                if target not in seen and (into(target) or target.split("#", 1)[-1].startswith(("use:", "glob:"))):
                    seen.add(target)
                    todo.append(target)
        return seen

    def reached(self, node: str) -> Set[str]:
        """The test code a test runs: its own item and every item of test code it names."""
        return {n for n in self._walk(node, self.test_code.__contains__) if n in self.test_code}

    def pins(self, node: str) -> Dict[str, List[str]]:
        """Each item of test code the test reaches that pins a value, with what it pins it with."""
        out: Dict[str, List[str]] = {}
        for item in sorted(self.reached(node)):
            path, ident = item.split("#", 1)
            if path not in self._tokens:
                self._tokens[path] = rust_graph.item_tokens(self.read(path))
            found = pin_markers(self._tokens[path].get(ident, []))
            named = {target for n in self._walk(item, lambda _: False) for target in self.graph.edges.get(n, ())}
            found += sorted(check.split("#", 1)[1] for check in named & self.golden_checks)  # named, or imported
            if found:
                out[rust_graph.describe_node(item)] = found
        return out


class TheWasmSkyGuard(unittest.TestCase):
    """The guard's reading of a filter and of the pins, on a small workspace."""

    FILTER = ("package(hyperion-sim) & test(/^sky::band::/) & not (test(/golden|pinned/) "
              "| test(=sky::band::tests::excepted))")
    SOURCES = {
        "crates/hyperion-testkit/src/lib.rs": "pub mod golden;\n",
        "crates/hyperion-testkit/src/golden.rs": "pub fn check(_: &str, _: &str, _: &str) {}\n",
        "crates/hyperion-sim/src/lib.rs": "pub mod sky;\n",
        "crates/hyperion-sim/src/sky/mod.rs": "pub mod band;\n#[cfg(test)]\nmod testing;\n",
        "crates/hyperion-sim/src/sky/testing.rs": (
            "pub const SKY_DIGEST: u64 = 0x12;\npub fn sky() -> u64 { SKY_DIGEST }\npub fn plain() -> u64 { 1 }\n"),
        "crates/hyperion-sim/src/sky/band.rs": (
            "pub fn march() -> f64 { 1.0 }\n"
            "#[cfg(test)]\nmod tests {\n"
            "    use super::*;\n    use hyperion_testkit::golden::check;\n"
            "    const STEP_BITS: u64 = 0x3ff0_0000_0000_0000;\n"
            "    fn helper() { assert_eq!(f64_digest(&[1.0, 2.0]), 0x1234, \"the digest\"); }\n"
            "    #[test]\n    fn plain() { assert_eq!(march(), 1.0); }\n"
            "    #[test]\n    fn same_target() { assert_eq!(f64_digest(&[march()]), f64_digest(&[1.0]), \"{STEP}\"); }\n"
            "    #[test]\n    fn same_bits() { assert!(f64_digest(&[march()]) == f64_digest(&[1.0])); }\n"
            "    #[test]\n    fn by_a_helper() { helper(); }\n"
            "    #[test]\n    fn by_a_constant() { assert_eq!(march().to_bits(), STEP_BITS); }\n"
            "    #[test]\n    fn by_its_bits() { assert_eq!(march(), f64::from_bits(0x3ff0_0000_0000_0000)); }\n"
            "    #[test]\n    fn by_a_macro() { hyperion_testkit::golden!(\"band\", \"text\"); }\n"
            "    #[test]\n    fn by_a_check() { check(\"\", \"band\", \"text\"); }\n"
            "    #[test]\n    fn by_a_digest() { assert!(super::super::testing::sky() == 0x12); }\n"
            "    #[test]\n    fn by_an_assert() { assert!(f64_digest(&[march()]) == crate::sky::PINNED); }\n"
            "    #[test]\n    fn by_its_fingerprint() { let _ = SERIAL_FINGERPRINT; }\n"
            "    #[test]\n    fn by_a_literal() { assert_ne!(&0xdead, march().to_bits()); }\n"
            "    #[test]\n    fn by_a_count() { assert_eq!(march() as u32, 1, \"0x12\"); }\n"
            "    #[test]\n    fn plain_from_testing() { assert_eq!(super::super::testing::plain(), 1); }\n"
            "    #[test]\n    fn the_bits_are_pinned() { assert_eq!(march().to_bits(), STEP_BITS); }\n"
            "    #[test]\n    fn excepted() { helper(); }\n"
            "    #[test]\n    #[ignore = \"slow: a pin, but no fast run runs it\"]\n    fn slow() { helper(); }\n"
            "}\n"),
    }

    def scan(self) -> WasmSkyScan:
        ws = GraphMode().workspace(self.SOURCES)
        files = dict(FILES)
        files.update(self.SOURCES)
        return WasmSkyScan(smart_ci.choose_tests([], ws).graph, lambda p: files.get(p, ""), self.FILTER)

    def test_the_filter_reads_as_nextest_does(self) -> None:
        f = Filter(self.FILTER)
        self.assertTrue(f.matches(SIM, SIM, "sky::band::tests::plain"))
        self.assertFalse(f.matches(SIM, SIM, "sky::band::tests::the_bits_are_pinned"))
        self.assertFalse(f.matches(SIM, SIM, "sky::band::tests::excepted"))
        self.assertFalse(f.matches(SIM, SIM, "sky::caps::tests::plain"))
        self.assertFalse(f.matches(BASE, BASE, "sky::band::tests::plain"))
        self.assertEqual(f.exact_tests(), ["sky::band::tests::excepted"])
        # `&` before `|`, `-` as `& not`, `!` as `not`, and each matcher.
        self.assertTrue(Filter("none() | all() & test(a)").matches(SIM, SIM, "a"))
        self.assertFalse(Filter("all() - test(=a)").matches(SIM, SIM, "a"))
        self.assertTrue(Filter("!test(~b) and package(hyperion-*)").matches(SIM, SIM, "a"))
        self.assertTrue(Filter("binary_id(#hyperion-sim::*) or test(/^x$/)").matches(SIM, "hyperion-sim::sky", "y"))
        for bad in ("kind(lib)", "test(a) &", "(test(a)", "test(a) test(b)", "test(/[/)"):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                Filter(bad)
        # A filter of many terms (smart-ci's, up to MAX_FILTER) is read without recursing once a term.
        many = Filter(" | ".join(f"(binary_id(=b) & test(=t{i}))" for i in range(5000)))
        self.assertTrue(many.matches(SIM, "b", "t4999"))
        self.assertFalse(many.matches(SIM, "b", "t5000"))

    def test_it_finds_each_pin_in_a_test_or_the_test_code_it_reaches(self) -> None:
        scan = self.scan()
        moved = {t.name.rsplit("::", 1)[1]: t for _, t in scan.moved}
        self.assertEqual(sorted(moved), sorted([
            "plain", "same_target", "same_bits", "by_a_helper", "by_a_constant", "by_its_bits", "by_a_macro",
            "by_a_check", "by_a_digest", "by_an_assert", "by_its_fingerprint", "by_a_literal", "by_a_count",
            "plain_from_testing"]))
        pins = {name: sorted(m for found in scan.pins(t.node).values() for m in found) for name, t in moved.items()}
        self.assertEqual(pins, {
            "plain": [], "same_target": [], "same_bits": [], "plain_from_testing": [], "by_a_count": [],
            "by_a_helper": ["assert_eq!(a digest, a constant)", "assert_eq!(…, 0x…)"],
            "by_a_literal": ["assert_ne!(…, 0x…)"],
            "by_a_constant": ["STEP_BITS", "STEP_BITS"],  # the test's and the constant's own
            "by_its_bits": ["from_bits(0x…)"],
            "by_a_macro": ["golden!"],
            "by_a_check": ["fn::check"],
            "by_a_digest": ["SKY_DIGEST", "SKY_DIGEST", "assert!(…, 0x…)"],
            "by_an_assert": ["assert!(a digest, a constant)"],
            "by_its_fingerprint": ["SERIAL_FINGERPRINT"],
        })

    def test_the_wasip1_runs_leave_it_out(self) -> None:
        self.assertEqual(smart_ci.less_wasm_sky("", "M"), "not (M)")
        self.assertEqual(smart_ci.less_wasm_sky("a | b", "M"), "(a | b) & not (M)")
        self.assertEqual(smart_ci.less_wasm_sky("a | b", ""), "a | b")
        self.assertEqual(smart_ci.just_string(["concatenate", "a", ["concatenate", "b", "c"]]), "abc")
        ws = workspace()
        ws.wasm_sky = "M"
        p = make_plan([("crates/hyperion-sim/src/sky/colour.rs", False)], ws, False)
        self.assertEqual(args(p, "_wasm-run"), ["-p hyperion-sim", "not (M)", ""])
        self.assertEqual(args(p, "_wasip1-build"), ["-p hyperion-sim", "not (M)"])
        self.assertIn("less `wasm_sky`", [s.title for s in p.steps if s.recipe[0] == "_wasm-run"][0])
        # And smart-ci's own tests, this guard, since the change is to the sim's sources.
        self.assertIn("_smart-ci-test", calls(p))
        self.assertNotIn("_smart-ci-test", calls(plan("crates/hyperion-base/src/math.rs")))
        # With the graph, its selection less the filter.
        graph_mode = GraphMode()
        now = dict(graph_mode.SOURCES)
        now["crates/hyperion-base/src/lib.rs"] = now["crates/hyperion-base/src/lib.rs"].replace("{ 1 }", "{ 4 }")
        chosen = make_plan([("crates/hyperion-base/src/lib.rs", False)], graph_mode.workspace(now), False)
        ws = graph_mode.workspace(now)
        ws.wasm_sky = "M"
        less = make_plan([("crates/hyperion-base/src/lib.rs", False)], ws, False)
        self.assertEqual(args(less, "_wasm-run")[1], f"({args(chosen, '_wasm-run')[1]}) & not (M)")
        self.assertEqual(args(less, "_wasip1-build")[1], args(less, "_wasm-run")[1])


class TheWasmSkyFilter(unittest.TestCase):
    """The justfile's `wasm_sky` leaves out of `ci`'s and smart-ci's wasip1 runs no test that pins a
    value: a test that pins one is named `golden` or `pinned`, or is one of the filter's exceptions."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.ws = real_workspace()
        cls.graph = real_graph()
        cls.scan = WasmSkyScan(cls.graph, cls.ws.read, cls.ws.wasm_sky)

    def test_no_test_it_leaves_out_pins_a_value(self) -> None:
        self.assertTrue(self.scan.moved, "`wasm_sky` selects no test")
        found = {test.name: pins for _, test in self.scan.moved if (pins := self.scan.pins(test.node))}
        self.assertEqual(found, {}, "these tests pin a value on every target, so `ci`'s wasip1 run keeps them: "
                                    "name each `…_golden` or `…_pinned`, or add it to the exceptions of `wasm_sky` "
                                    "in the justfile")

    def test_its_exceptions_are_tests(self) -> None:
        names = {test.name for tests in self.graph.tests.values() for test in tests}
        self.assertTrue(self.scan.filter.exact_tests())
        self.assertEqual([t for t in self.scan.filter.exact_tests() if t not in names], [],
                         "`wasm_sky` names tests that do not exist: renamed, they would leave `ci`'s wasip1 run")

    def test_it_reads_the_sources_whose_change_runs_it(self) -> None:
        # smart-ci runs this file's tests for a change to WASM_SKY_SOURCES: the files of the tests the
        # filter selects and of the test code they reach are all there, the reader follows each, and
        # the golden checks it looks for exist.
        read = {rust_graph.file_of(node) for _, test in self.scan.moved for node in self.scan.reached(test.node)}
        self.assertEqual(sorted(f for f in read if not smart_ci.WASM_SKY_SOURCES.search(f)), [],
                         "the guard reads files outside WASM_SKY_SOURCES (tools/smart-ci/smart_ci.py): widen it")
        sources = [f for f in self.ws.files if smart_ci.WASM_SKY_SOURCES.search(f)]
        self.assertLessEqual(read, set(sources))
        self.assertEqual([f for f in sources if not self.graph.rs[f].ok], [])
        golden = smart_ci.GOLDEN_READERS[0][1]
        self.assertTrue(smart_ci.WASM_SKY_SOURCES.search(golden))
        self.assertLessEqual({f"{golden}#fn::check", f"{golden}#fn::check_embedded", f"{golden}#macro::golden"},
                             self.scan.golden_checks)

    def test_no_macro_makes_tests_where_it_selects(self) -> None:
        # The guard sees the tests the source names; one that a macro or a case attribute makes would
        # pass it unseen.
        files = {rust_graph.file_of(test.node) for _, test in self.scan.moved}
        made = sorted(term for terms in self.graph.generators.values() for node, term in terms.items()
                      if rust_graph.file_of(node) in files)
        self.assertEqual(made, [])

    def test_ci_and_smart_ci_leave_it_out_and_test_wasm_sky_runs_it(self) -> None:
        wasm_sky = self.ws.wasm_sky
        self.assertTrue(wasm_sky)
        # `ci`'s run passes `not (wasm_sky)` to `_wasm-run`, whose filter, when it has one, may select
        # none (smart-ci's, for a change that reaches only `wasm_sky`'s tests).
        dumped = dump()
        fast = dumped["recipes"]["_wasm-fast-run"]["dependencies"]
        self.assertEqual([(d["recipe"], [arg_text(a) for a in d["arguments"]]) for d in fast],
                         [("_wasm-run", ["wasip1_crates", "wasip1_filter", "browser_crates"])])
        evaluated = subprocess.run([just(), "--evaluate", "wasip1_filter"], cwd=str(ROOT), capture_output=True,
                                   text=True, check=True).stdout
        self.assertEqual(evaluated, f"not ({wasm_sky})")
        self.assertIn("_wasip1-nextest '-p x' --no-tests=warn 'f'", dry_run("_wasm-run", "-p x", "f", "")[0])
        self.assertIn("_wasip1-nextest '-p x' \"\" ''", dry_run("_wasm-run", "-p x", "", "")[0])
        first, listing, run = dry_run("test-wasm-sky")
        self.assertIn("--no-run", first)
        self.assertTrue(listing.startswith("just _wasip1-build ") and listing.endswith(f"'{wasm_sky}'"), listing)
        self.assertTrue(run.startswith("just _locked ") and run.endswith(f"'{wasm_sky}'"), run)
        slow = dumped["recipes"]["test-wasm-slow"]
        self.assertIn("test-wasm-sky", [d["recipe"] for d in slow["dependencies"]])
        p = make_plan([("crates/hyperion-sim/src/sky/band.rs", False)], self.ws, False, all_tests=True)
        self.assertEqual(args(p, "_wasm-run")[1], f"not ({wasm_sky})")

    def test_every_wasip1_crate_keeps_tests_in_cis_run(self) -> None:
        # `ci`'s wasip1 run passes with no test (`--no-tests=warn`), so a `wasm_sky` grown too wide
        # would leave it running nothing: every crate it runs keeps a test outside the filter.
        kept = {unit.crate for unit in self.graph.units.values() if unit.runs and unit.crate in self.ws.wasip1
                for test in self.graph.tests.get(unit.id, []) if not test.ignored
                and not self.scan.filter.matches(unit.crate, unit.id, test.name)}
        self.assertEqual(sorted(set(self.ws.wasip1) - kept), [])


if __name__ == "__main__":
    unittest.main(verbosity=1)
