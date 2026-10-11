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
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

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
        self.assertEqual(calls(p), [
            "_wasm-preflight", "_rustfmt-check", "_clippy-for", "fit-check",
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

    def __init__(self, fail: Sequence[str] = ()) -> None:
        super().__init__(ROOT, "just")
        self.calls: List[Tuple[str, List[str]]] = []
        self.handoffs: List[dict] = []
        self.envs: List[Optional[Dict[str, str]]] = []  # what each `run` adds to the environment
        self.fail = set(fail)

    def run(self, args: Sequence[str], env=None) -> int:
        self.calls.append(("run", list(args)))
        self.envs.append(dict(env) if env else None)
        return 1 if args[0] in self.fail else 0

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

    def test_integrations_run_takes_the_lock_with_priority_when_asked(self) -> None:
        # perf-build L5 part a: `--full` with SMART_CI_PRIORITY=1 gives `_locked` HYPERION_HEAVY_PRIORITY=1.
        for plan_, env, wanted in ((smart_ci.full_plan(), {"SMART_CI_PRIORITY": "1"}, {"HYPERION_HEAVY_PRIORITY": "1"}),
                                   (smart_ci.full_plan(), {}, None),
                                   (plan("justfile"), {"SMART_CI_PRIORITY": "1"}, None),
                                   (plan("crates/hyperion-server/src/main.rs"), {"SMART_CI_PRIORITY": "1"}, None)):
            with self.subTest(full=plan_.full, env=env):
                runner = FakeRunner()
                self.assertEqual(smart_ci.execute(plan_, ROOT, "just", env=env, runner=runner), 0)
                locked = [e for (kind, args), e in zip([c for c in runner.calls if c[0] == "run"], runner.envs)
                          if args[0] == "_locked"]
                self.assertEqual(locked, [wanted])
        self.assertIn("with priority", smart_ci.describe(smart_ci.full_plan(), [], "x", False,
                                                         env={"SMART_CI_PRIORITY": "1"}))
        self.assertIn("is ignored", smart_ci.describe(plan("justfile"), [], "x", False, env={"SMART_CI_PRIORITY": "1"}))

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


class RealWorkspace(unittest.TestCase):
    """The names smart-ci builds on exist, and the real workspace loads."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.ws = smart_ci.load_workspace(ROOT, just())

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
        chosen = smart_ci.choose_tests([], self.ws)
        graph = chosen.graph
        self.assertTrue(any(u.is_lib for u in graph.units.values()))
        self.assertGreater(sum(len(t) for t in graph.tests.values()), 1000)
        self.assertEqual(graph.missing_mods, [], "a `mod` declaration names no file the graph knows")
        self.assertEqual([f for f in graph.orphans if "/src/" in f], [], "sources in no module tree: they widen")


if __name__ == "__main__":
    unittest.main(verbosity=1)
