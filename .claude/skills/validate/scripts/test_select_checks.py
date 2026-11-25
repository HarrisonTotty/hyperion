#!/usr/bin/env python3
"""Tests for select_checks.py's reading of its arguments and its routing to the smoke harness:
`python3 -I -B .claude/skills/validate/scripts/test_select_checks.py`.

Standard library only; nothing runs git but `parse_args`'s check of a word that is not a task ID,
against this clone.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import select_checks as sc  # noqa: E402  (the script's directory is put on the path first)


class TaskIds(unittest.TestCase):
    def test_a_task_a_subtask_and_a_sub_subtask(self) -> None:
        self.assertEqual(sc.normal_task("P02.T5"), "P02.T5")
        self.assertEqual(sc.normal_task("r04.t3.C,"), "R04.T3.c")
        self.assertEqual(sc.normal_task("p14.T54.B.l"), "P14.T54.b.l")
        self.assertEqual(sc.normal_task("P10.T3.d.a."), "P10.T3.d.a")

    def test_what_is_not_a_task_id(self) -> None:
        for word in ("P14.T54.b.l.a", "P1.T2", "P14.54", "HEAD~1", "P14.T54.b-l"):
            self.assertIsNone(sc.normal_task(word), word)

    def test_the_arguments_take_a_sub_subtask(self) -> None:
        task, base, feature, warnings = sc.parse_args(sc.repo_root(), ["P14.T54.b.l", "--feature", "galaxy-generation"])
        self.assertEqual((task, base, feature, warnings), ("P14.T54.b.l", "HEAD", "galaxy-generation", []))


# The files that choose what each quality setting makes (R12.T7.a): `tools/smart-ci/test_smart_ci.py`
# holds the same list.
LADDER = [
    "apps/hyperion/src/renderer/src/view/photoreal/renderer.ts",
    "apps/hyperion/src/renderer/src/view/terrain/materialSet.ts",
    "apps/hyperion/src/renderer/src/view/terrain/terrainPass.ts",
    "apps/hyperion/src/renderer/src/view/quality/qualitySetting.ts",
    "apps/hyperion/src/renderer/src/view/atmosphere/settings.ts",
    "apps/hyperion/src/renderer/src/view/surface/decoration/decorationSetting.ts",
]

# The airless twins' engine (plan R07, perf-bodies-pass): `tools/smart-ci/test_smart_ci.py` holds the
# same list.
TWINS = ["apps/hyperion/src/renderer/src/view/atmosphere/sourceTableEngine.ts"]


class TheHarness(unittest.TestCase):
    def test_what_a_setting_makes_runs_the_ladder_part_alone(self) -> None:
        for path in LADDER:
            with self.subTest(path=path):
                self.assertEqual(sc.harness_runs([path], set()), ([], [path], []))
        # With a shader, or where the catalogue imports it, the whole of test-render runs, the ladder in it.
        shader = "apps/hyperion/src/renderer/src/view/shaders/sky.wgsl"
        self.assertEqual(sc.harness_runs([shader, LADDER[0]], set()), ([shader], [LADDER[0]], []))
        self.assertEqual(sc.harness_runs([LADDER[3]], {LADDER[3]}), ([LADDER[3]], [], []))
        for path in ("apps/hyperion/src/renderer/src/view/terrain/cube.ts",
                     "apps/hyperion/src/renderer/src/view/photoreal/renderer.test.ts"):
            self.assertEqual(sc.harness_runs([path], set()), ([], [], []), path)

    def test_the_airless_twins_engine_runs_the_twins_part_alone(self) -> None:
        self.assertEqual(sc.harness_runs(TWINS, set()), ([], [], TWINS))
        self.assertEqual(sc.harness_runs([*TWINS, LADDER[0]], set()), ([], [LADDER[0]], TWINS))
        # Where the catalogue imports it, the whole of test-render runs, the twins' part in it.
        self.assertEqual(sc.harness_runs(TWINS, set(TWINS)), (TWINS, [], []))
        for path in ("apps/hyperion/src/renderer/src/view/atmosphere/sourceTableEngine.test.ts",
                     "apps/hyperion/src/renderer/src/view/atmosphere/surfaceLighting.ts"):
            self.assertEqual(sc.harness_runs([path], set()), ([], [], []), path)


if __name__ == "__main__":
    unittest.main()
