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

import json
import os
import re
import shlex
import subprocess
import sys
import unittest
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import smart_ci  # noqa: E402
from smart_ci import Crate, Plan, Workspace, make_plan  # noqa: E402

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
    "crates/hyperion-server/src/main.rs": "",
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


def plan(*paths: str, deleted: Iterable[str] = (), dependents: bool = False) -> Plan:
    gone = set(deleted)
    return make_plan(sorted((p, p in gone) for p in set(paths) | gone), workspace(), dependents)


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
CARGO_RECIPES = {
    "_rustfmt-check", "_clippy-for", "_cross-clippy-for", "_browser-clippy-for", "_relaxed-simd-refused",
    "_gen-protocol-check-for", "fit-check", "_test-build-for", "_nextest-run", "_doctest-run", "_wasip1-nextest",
    "_browser", "_wasm-preflight", "gen-surface",
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
        self.assertEqual(args(p, "_vitest-run"), ["", ""])
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
        for recipe in ("_test-build-for", "_nextest-run", "_doctest-run"):
            self.assertEqual(args(p, recipe), ["-p hyperion-server"])

    def test_a_sim_change_runs_the_sims_checks_wasip1_and_fit_check_but_no_typescript(self) -> None:
        p = plan("crates/hyperion-sim/src/galaxy/disc.rs")
        self.assertEqual(calls(p), [
            "_wasm-preflight", "_rustfmt-check", "_cross-clippy-for", "_clippy-for", "fit-check",
            "_test-build-for", "_wasip1-nextest", "_nextest-run", "_doctest-run", "_wasip1-nextest"])
        self.assertEqual(args(p, "_wasm-preflight"), ["wasip1"])
        # The features `--workspace` gives the sim: the fitting crate asks for `testing`.
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-sim --features hyperion-sim/testing"])
        # The testkit's source scans read the sim's sources: its native suite runs, not its doctests.
        self.assertEqual(packages(p, "_nextest-run"), [SIM, TESTKIT])
        self.assertEqual(packages(p, "_doctest-run"), [SIM])
        # On wasip1, the features of `ci`'s selection there, where nothing asks for `testing`.
        self.assertEqual(args(p, "_wasip1-nextest", 0), ["-p hyperion-sim", "--no-run"])
        self.assertEqual(args(p, "_wasip1-nextest", 1), ["-p hyperion-sim", ""])
        self.assertIn("hyperion-fit, hyperion-server", " ".join(p.notes))

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
            "gen-surface", "_wasm-preflight", "_rustfmt-check", "_relaxed-simd-refused", "_cross-clippy-for",
            "_clippy-for", "_browser-clippy-for", "_test-build-for", "_wasip1-nextest", "_browser", "_nextest-run",
            "_doctest-run", "_vitest-run", "_wasip1-nextest", "_browser"])
        self.assertEqual(args(p, "_wasm-preflight"), ["wasip1", "browser"])
        self.assertEqual(args(p, "_clippy-for"), ["-p hyperion-surface --features hyperion-surface/testing"])
        self.assertEqual(args(p, "_browser-clippy-for"), ["hyperion-surface"])
        self.assertEqual(args(p, "_browser", 0), ["prepare", "hyperion-surface"])
        self.assertEqual(args(p, "_browser", 1), ["run", "hyperion-surface"])
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
            "gen-surface", "_rustfmt-check", "_typecheck-ts", "_oxlint", "_cross-clippy-for", "_clippy-for",
            "_gen-protocol-check-for", "_test-build-for", "_nextest-run", "_doctest-run", "_vitest-run"])
        self.assertEqual(args(p, "_gen-protocol-check-for"), ["-p hyperion-protocol"])
        self.assertEqual(args(p, "_vitest-run"), ["", ""])

    def test_the_bindings_run_their_check_and_the_typescript_side(self) -> None:
        p = plan("packages/protocol/src/generated/BodyId.ts")
        self.assertEqual(calls(p), ["gen-surface", "_prettier-check", "_typecheck-ts", "_oxlint",
                                    "_gen-protocol-check-for", "_vitest-run"])
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
        self.assertIn("test-render", p.recipes())
        self.assertIn("_vitest-run", calls(p))
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
        self.assertIn("test-render", plan("pnpm-lock.yaml").recipes())

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
        self.assertEqual(calls(p), ["gen-surface", "_wasm-preflight", "fit-check", "_test-build-for", "_wasip1-nextest",
                                    "_nextest-run", "_doctest-run", "_vitest-run", "_wasip1-nextest"])
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
        self.assertEqual(args(node, "_browser", 0), ["prepare", "hyperion-base hyperion-surface hyperion-testkit"])
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
        hooks = plan(".pre-commit-config.yaml")
        self.assertEqual(calls(hooks), ["_prettier-check"])
        self.assertTrue(hooks.notes)

    def test_measurements_run_the_clients_suite(self) -> None:
        p = plan("docs/measurements/descent-spike/2026-10-07-effect-low.json")
        self.assertEqual(args(p, "_vitest-run"), ["hyperion", ""])
        self.assertNotIn("_typecheck-ts", calls(p))

    def test_a_typescript_package_file_runs_vitest_over_every_package(self) -> None:
        self.assertEqual(args(plan("packages/protocol/src/hex.ts"), "_vitest-run"), ["", ""])

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
        order = ["setup", "side", "main", "locked"]
        phases = [order.index(step.phase) for step in p.steps]
        self.assertEqual(phases, sorted(phases))
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "side"], [
            "_rustfmt-check", "_prettier-check", "_typecheck-ts", "_oxlint", "_relaxed-simd-refused",
            "gpu-replay-check", "_cross-clippy-for", "_smart-ci-test"])
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "main"], [
            "_clippy-for", "_browser-clippy-for", "_gen-protocol-check-for", "fit-check", "_test-build-for",
            "_wasip1-nextest", "_browser"])
        self.assertEqual([s.recipe[0] for s in p.steps if s.phase == "locked"], [
            "_nextest-run", "_doctest-run", "_vitest-run", "_wasip1-nextest", "_browser"])

    def test_every_step_names_its_causes(self) -> None:
        p = plan("crates/hyperion-surface/src/field.rs", "README.md")
        for step in p.steps:
            self.assertTrue(step.causes, step.title)
        text = smart_ci.describe(p, [("crates/hyperion-surface/src/field.rs", False), ("README.md", False)], "test", False)
        self.assertIn("just _clippy-for", text)
        self.assertIn("because of crates/hyperion-surface/src/field.rs", text)

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
                 ("docs/agent/plans/a plan.md", "tools/electron-node/node", "package.json"),
                 ("docs/measurements/descent-spike/2026-10-07-effect-low.json", "crates/hyperion-fit/manifests/mge.toml")]
        known = set(smart_ci.CI_STEPS) | {name for _, calls_ in smart_ci.CI_STEPS.values() for name, _ in calls_}
        known |= set(smart_ci.NARROWER.values())
        for paths in cases:
            for dependents in (False, True):
                p = plan(*paths, dependents=dependents)
                for phase in ("setup", "side", "main", "locked"):
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
                    self.assertLessEqual({s.recipe[0] for s in p.steps}, known, "smart-ci runs only `ci`'s steps")

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

    def test_the_surface_module_has_readers(self) -> None:
        graph = smart_ci.TsGraph(self.ws)
        self.assertTrue(graph.surface_readers())

    def test_a_real_single_crate_plan(self) -> None:
        p = make_plan([("crates/hyperion-server/src/main.rs", False)], self.ws, False)
        self.assertFalse(p.is_full)
        self.assertNotIn("_vitest-run", calls(p))
        self.assertEqual(packages(p, "_clippy-for"), [SERVER])


if __name__ == "__main__":
    unittest.main(verbosity=1)
