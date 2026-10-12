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
import re
import shlex
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import unittest.mock
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import rust_graph  # noqa: E402
import smart_ci  # noqa: E402
from smart_ci import Crate, Plan, Workspace, make_plan  # noqa: E402
import test_rust_graph  # noqa: E402


def load_tests(loader: unittest.TestLoader, tests: unittest.TestSuite, pattern: Optional[str]) -> unittest.TestSuite:
    """These tests and the Rust graph's own, every class of them: `just ci` runs this file."""
    tests.addTests(loader.loadTestsFromModule(test_rust_graph))
    return tests

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
    "_rustfmt-check", "_clippy-for", "_cross-clippy-for", "_browser-clippy-for", "_relaxed-simd-refused",
    "_gen-protocol-check-for", "fit-check", "_test-build-for", "_nextest-run", "_doctest-run", "_wasip1-build", "_wasip1-nextest",
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
        self.assertEqual(calls(p), ["_rustfmt-check", "_cross-clippy-for", "_clippy-for", "_test-build-for",
                                    "_nextest-run", "_doctest-run"])
        self.assertEqual(args(p, "_rustfmt-check"), ["-p hyperion-server"])
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-server"])
        self.assertEqual(args(p, "_cross-clippy-for"), ["-p hyperion-server", ""])
        self.assertEqual(args(p, "_test-build-for"), ["-p hyperion-server"])
        for recipe in ("_nextest-run", "_doctest-run"):
            self.assertEqual(args(p, recipe), ["-p hyperion-server", ""])

    def test_a_sim_change_runs_the_sims_checks_wasip1_and_fit_check_but_no_typescript(self) -> None:
        p = plan("crates/hyperion-sim/src/galaxy/disc.rs")
        # No cross-Clippy: the sim is the same code on Linux, macOS and Windows (CROSS_CLIPPY_CRATES).
        # smart-ci's own tests: their guard of the justfile's `wasm_sky` reads the sim's sources.
        self.assertEqual(calls(p), [
            "_wasm-preflight", "_rustfmt-check", "_smart-ci-test", "_clippy-for", "fit-check",
            "_test-build-for", "_wasip1-build", "_nextest-run", "_doctest-run", "_wasm-run"])
        self.assertEqual(args(p, "_wasm-preflight"), ["wasip1"])
        # The features `--workspace` gives the sim: the fitting crate asks for `testing`.
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-sim --features hyperion-sim/testing"])
        # The testkit's source scans read the sim's sources: its native suite runs, not its doctests.
        self.assertEqual(packages(p, "_nextest-run"), [SIM, TESTKIT])
        self.assertEqual(packages(p, "_doctest-run"), [SIM])
        # On wasip1, the features of `ci`'s selection there, where nothing asks for `testing`.
        self.assertEqual(args(p, "_wasip1-build"), ["-p hyperion-sim", ""])
        self.assertEqual(args(p, "_wasm-run"), ["-p hyperion-sim", "", ""])
        self.assertIn("hyperion-fit, hyperion-server", " ".join(p.notes))
        self.assertIn("Clippy for the other platforms skips hyperion-sim", " ".join(p.notes))

    def test_dependents_adds_the_reverse_dependents_clippy_and_tests(self) -> None:
        p = plan("crates/hyperion-sim/src/galaxy/disc.rs", dependents=True)
        self.assertEqual(packages(p, "_clippy-for"), [FIT, SERVER, SIM])
        self.assertEqual(packages(p, "_nextest-run"), [FIT, SERVER, SIM, TESTKIT])
        self.assertEqual(packages(p, "_rustfmt-check"), [SIM])
        self.assertFalse(any("Not checked" in note for note in p.notes))
        base = plan("crates/hyperion-base/src/math.rs", dependents=True)
        self.assertEqual(packages(base, "_clippy-for"), [BASE, FIT, SERVER, SIM, SURFACE])

    def test_a_surface_change_rebuilds_the_module_and_runs_the_vitest_files_that_import_it(self) -> None:
        p = plan("crates/hyperion-surface/src/field.rs")
        self.assertEqual(calls(p), [
            "gen-surface", "_wasm-preflight", "_rustfmt-check", "_relaxed-simd-refused", "_vitest-run",
            "_clippy-for", "_browser-clippy-for", "_test-build-for", "_wasip1-build", "_browser", "_nextest-run",
            "_doctest-run", "_wasm-run"])
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
            "_gen-protocol-check-for", "_test-build-for", "_nextest-run", "_doctest-run"])
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
        self.assertEqual(calls(p), ["gen-surface", "_wasm-preflight", "_vitest-run", "fit-check", "_test-build-for",
                                    "_wasip1-build", "_nextest-run", "_doctest-run", "_wasm-run"])
        self.assertEqual(args(p, "_vitest-run"), ["hyperion", "src/renderer/src/lib/orbit.test.ts"])
        fit_data = plan("crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv")
        self.assertEqual(packages(fit_data, "_nextest-run"), [FIT])
        self.assertEqual(args(fit_data, "_vitest-run"), ["hyperion", "src/tools/atmosphereData.test.ts"])
        self.assertIn("fit-check", calls(fit_data))
        # A README in a data directory is data: a dataset's files are hashed through its PROVENANCE.toml.
        self.assertEqual(packages(plan("crates/hyperion-fit/data/iprt_phase_a/README.md"), "_nextest-run"), [FIT])
        self.assertEqual(packages(plan("crates/hyperion-sim/tests/data/sse/README.md"), "_nextest-run"), [SIM])
        manifest = plan("crates/hyperion-fit/manifests/mge.toml")
        self.assertEqual(calls(manifest), ["fit-check", "_test-build-for", "_nextest-run", "_doctest-run"])

    def test_the_sims_tables_run_the_fitting_crates_tests(self) -> None:
        p = plan("crates/hyperion-sim/src/tables/mge.rs")
        self.assertEqual(packages(p, "_nextest-run"), [FIT, SIM, TESTKIT])
        self.assertEqual(packages(p, "_doctest-run"), [FIT, SIM])
        self.assertEqual(packages(p, "_clippy-for"), [SIM])

    def test_the_generator_version_runs_every_crates_tests(self) -> None:
        p = plan("crates/hyperion-base/src/version.rs")
        self.assertEqual(packages(p, "_nextest-run"), sorted(workspace().crates))
        self.assertIn("src/renderer/src/wasm/handleRequest.test.ts", args(p, "_vitest-run")[1])

    def test_a_crates_manifest_adds_its_dependents(self) -> None:
        p = plan("crates/hyperion-surface/Cargo.toml")
        self.assertEqual(packages(p, "_clippy-for"), [FIT, SERVER, SIM, SURFACE])
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
            "_clippy-for", "_browser-clippy-for", "_gen-protocol-check-for", "fit-check", "_test-build-for",
            "_wasip1-build", "_browser"])
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
        self.assertNotIn("_nextest-run", calls(make_plan([("crates/hyperion-sim/README.md", False)], ws, False)))

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

    def test_a_small_plan_runs_without_the_lock_capped_and_holding_no_slot(self) -> None:
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
        # scope at 4 test threads, and no `_locked`.
        self.assertEqual([kind for kind, _ in runner.calls][-2:], ["command", "run"])
        self.assertNotIn("_nextest-run", smart_ci.flat(runner.handoffs[-1].get("main", [])))
        kind, suites = runner.calls[-1]
        self.assertEqual(suites, smart_ci.flat(smart_ci.phases_of(p)["locked"]))
        self.assertEqual([suites[0], suites[3]], ["_nextest-run", "_wasm-run"])
        self.assertEqual(runner.scopes[-1], FakeRunner.SCOPE)
        self.assertEqual(runner.envs[-1], {"NEXTEST_TEST_THREADS": "4", "RUST_TEST_THREADS": "4", "CARGO_BUILD_JOBS": "4"})
        self.assertFalse(any(args[:1] == ["_locked"] for _, args in runner.calls))
        text = smart_ci.describe(p, [], "x", False, env={"SMART_CI_PRIORITY": "1"})
        self.assertIn("The suites run without the heavy-test lock: 3 test(s) and 5 s of test time", text)
        self.assertIn(f"within {smart_ci.SMALL_TESTS} tests and {smart_ci.SMALL_SECONDS:.0f} s", text)
        self.assertIn("SMART_CI_PRIORITY=1 has nothing to do", text)

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
        recipes = set(smart_ci.CI_STEPS) | set(smart_ci.NARROWER.values()) | RENDER_RECIPES
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

            def locked(*command: str, priority_run: bool = False) -> "subprocess.Popen[str]":
                return subprocess.Popen([just(), "_locked", *command], cwd=str(ROOT), text=True,
                                        env={**env, "HYPERION_HEAVY_PRIORITY": "1"} if priority_run else env,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                        start_new_session=os.name != "nt")
            # With priority: the file names it while it waits, and is gone, as is the variable, once it holds the lock.
            check = f'test ! -e "{priority}" && test -z "${{HYPERION_HEAVY_PRIORITY:-}}"'
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
        known |= set(smart_ci.NARROWER.values()) | RENDER_RECIPES
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
        for path in (smart_ci.GENERATOR_VERSION_FILE, smart_ci.CATALOGUE):
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

        def dry_run(*recipe: str) -> List[str]:
            out = subprocess.run([just(), "--dry-run", *recipe], cwd=str(ROOT), capture_output=True, text=True,
                                 check=True)
            return [line.strip() for line in out.stderr.splitlines() if line.strip().startswith("just ")]

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
