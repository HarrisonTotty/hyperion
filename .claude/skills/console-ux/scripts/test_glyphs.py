#!/usr/bin/env python3
"""Tests for glyphs.py's command line: `python3 -B .claude/skills/console-ux/scripts/test_glyphs.py`.

Standard library only. The fonts are stood in for by the coverage the UX guide states (Typography),
so the tests run without `pnpm install`; the last class reads the bundled fonts when they are there.
"""

from __future__ import annotations

import contextlib
import io
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import glyphs  # noqa: E402  (the script's directory is put on the path first)

# Typography: "B612 and B612 Mono, as bundled, hold Latin-1 (U+0020–007E and U+00A0–00FF ...), `Œ`
# and `œ`, and a few marks: `–`, `—`, `−`, `′`, `″`, `…`, `•`, curly quotes, `€`, `™`, `↑` and `↓`."
STATED = frozenset(
    [*range(0x20, 0x7F), *range(0xA0, 0x100), 0x152, 0x153, 0x2013, 0x2014, 0x2212, 0x2032, 0x2033, 0x2026, 0x2022]
    + [0x2018, 0x2019, 0x201C, 0x201D, 0x20AC, 0x2122, 0x2191, 0x2193]
)


def run(*args: str) -> tuple[int, str, str]:
    """glyphs.py's exit status, stdout and stderr for these arguments, on the stated coverage."""
    out, err = io.StringIO(), io.StringIO()
    with mock.patch.object(glyphs, "coverage", lambda font: STATED), mock.patch.object(sys, "argv", ["glyphs.py", *args]):
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            try:
                glyphs.main()
                status = 0
            except SystemExit as stop:
                status = int(stop.code or 0)
    return status, out.getvalue(), err.getvalue()


class Spaces(unittest.TestCase):
    def test_a_thin_space_fails_and_names_the_spaces_both_faces_hold(self) -> None:
        status, out, _ = run("12\u2009km")
        self.assertEqual(status, 1)
        self.assertIn("U+2009", out)
        self.assertIn("THIN SPACE", out)
        self.assertIn("U+0020 and the no-break U+00A0", out)
        self.assertNotIn("Name the quantity in words", out)  # that advice is for other glyphs

    def test_a_narrow_no_break_space_from_an_escape_fails(self) -> None:
        status, out, _ = run("\\u202f")
        self.assertEqual(status, 1)
        self.assertIn("NARROW NO-BREAK SPACE", out)

    def test_the_no_break_space_passes(self) -> None:
        status, out, _ = run("12\u00a0km")
        self.assertEqual(status, 0)
        self.assertIn("U+00A0", out)
        self.assertIn("NO-BREAK SPACE", out)

    def test_ascii_whitespace_only_separates(self) -> None:
        status, out, _ = run("a b\tc\n", "d\r\x0c")
        self.assertEqual(status, 0)
        codes = [line.split()[1] for line in out.splitlines()[1:] if line.strip()]
        self.assertEqual(codes, ["U+0061", "U+0062", "U+0063", "U+0064"])

    def test_nothing_but_separators_is_a_usage_error(self) -> None:
        status, _, err = run(" \t ")
        self.assertEqual(status, 2)
        self.assertIn("no characters to check", err)

    def test_a_space_and_a_missing_glyph_give_both_advices(self) -> None:
        status, out, _ = run("μ\u2009")
        self.assertEqual(status, 1)
        self.assertIn("Name the quantity in words", out)
        self.assertIn("U+0020 and the no-break U+00A0", out)


class Glyphs(unittest.TestCase):
    def test_a_greek_letter_fails_with_the_words_advice(self) -> None:
        status, out, _ = run("μ")
        self.assertEqual(status, 1)
        self.assertIn("Name the quantity in words; only a unit symbol or a key's own sign is drawn", out)

    def test_the_micro_sign_passes(self) -> None:
        self.assertEqual(run("µ")[0], 0)


@unittest.skipUnless(all(glyphs.font_files(package) for package in glyphs.FONTS.values()), "the bundled fonts are not installed (pnpm install)")
class BundledFonts(unittest.TestCase):
    """The guide's statement about spaces, against the fonts the client ships."""

    def test_only_the_space_and_the_no_break_space(self) -> None:
        for font in glyphs.FONTS:
            points = glyphs.coverage(font)
            self.assertIn(0x20, points, font)
            self.assertIn(0xA0, points, font)
            for space in (0x2002, 0x2003, 0x2009, 0x200A, 0x202F, 0x205F, 0x3000):
                self.assertNotIn(space, points, f"{font} U+{space:04X}")


if __name__ == "__main__":
    unittest.main()
