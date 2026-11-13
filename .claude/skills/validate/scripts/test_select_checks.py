#!/usr/bin/env python3
"""Tests for select_checks.py's reading of its arguments:
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


if __name__ == "__main__":
    unittest.main()
