#!/usr/bin/env python3
"""WCAG contrast of HYPERION's colour tokens, against the UX guide's thresholds.

Usage:
    contrast.py                         Every pairing the guide requires, PASS/FAIL.
    contrast.py FG BG [--min RATIO]     One pairing; FG and BG are tokens (`--text` or `text`)
                                        or hex colours (#rgb, #rrggbb, or either with an alpha
                                        digit pair, which is ignored). Default minimum 6.

Tokens are read from the `:root` block of apps/hyperion/src/renderer/src/styles.css, so the check
follows the stylesheet. The guide (Colour): text and the meaning-carrying parts of symbols reach
6:1 against their surface; control outlines 3:1; solid status fills carry `--surface-0` text.
Decorative hairlines (`--line`) have no minimum. Only hex values can be scored: a required token
with any other value (rgb(), a name) is reported as not a hex value. Exit status is 1 when a
required pairing fails or cannot be scored, and 2 on a usage error (a malformed colour, an
unknown token, a bad --min) or when styles.css or its `:root` block is missing.
"""

from __future__ import annotations

import math
import re
import sys
from pathlib import Path
from typing import NoReturn

TEXT_MIN = 6.0  # NASA-STD-3001 Vol. 2 App. F minimum, adopted by the guide for text and symbols
OUTLINE_MIN = 3.0  # WCAG 2.2 SC 1.4.11 non-text contrast, adopted by the guide for control outlines
RATIO_MAX = 21.0  # white on black: no WCAG contrast ratio can be higher
SURFACES = ("--surface-0", "--surface-1", "--surface-2")
TEXT_TOKENS = (
    "--text",
    "--text-muted",
    "--accent",
    "--status-nominal",
    "--status-advisory",
    "--status-caution",
    "--status-warning",
    "--target",
)
OUTLINE_TOKENS = ("--line-strong",)
FILL_TOKENS = ("--status-nominal", "--status-advisory", "--status-caution", "--status-warning", "--accent", "--target")
HEX = re.compile(r"^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$", re.I)
COLOUR_FUNCTION = re.compile(r"^(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|color-mix)\(", re.I)
USAGE = "usage: contrast.py [FG BG [--min RATIO]]"


def fail(message: str) -> NoReturn:
    print(f"contrast.py: {message}", file=sys.stderr)
    sys.exit(2)


def stylesheet() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        candidate = parent / "apps" / "hyperion" / "src" / "renderer" / "src" / "styles.css"
        if candidate.exists():
            return candidate
    fail("apps/hyperion/src/renderer/src/styles.css not found")


def tokens() -> dict[str, str]:
    """Every custom property in the `:root` block, with its value as written (hex or not)."""
    css = re.sub(r"/\*.*?\*/", "", stylesheet().read_text(encoding="utf-8"), flags=re.S)
    root = re.search(r":root\s*\{(.*?)\}", css, re.S)
    if root is None:
        fail("no :root block in styles.css")
    return {m.group(1): m.group(2).strip() for m in re.finditer(r"(--[\w-]+)\s*:\s*([^;]+)", root.group(1))}


def resolve(name: str, table: dict[str, str]) -> str:
    if name.startswith("#"):
        if not HEX.match(name):
            fail(f"{name} is not a hex colour (#rgb, #rgba, #rrggbb or #rrggbbaa)")
        return name
    key = name if name.startswith("--") else f"--{name}"
    if key not in table:
        known = ", ".join(sorted(k for k, v in table.items() if HEX.match(v)))
        fail(f"unknown token {name}; known: {known}")
    if not HEX.match(table[key]):
        fail(f"{key} is {table[key]}, not a hex value; this script scores hex colours only")
    return table[key]


def parse_args(args: list[str]) -> tuple[list[str], float]:
    """The two colours and the minimum ratio. Only --min is an option: `--text` is a token."""
    colours: list[str] = []
    minimum = TEXT_MIN
    i = 0
    while i < len(args):
        arg = args[i]
        if arg == "--min" or arg.startswith("--min="):
            if "=" in arg:
                value, i = arg.partition("=")[2], i + 1
            elif i + 1 < len(args):
                value, i = args[i + 1], i + 2
            else:
                fail(f"--min needs a ratio\n{USAGE}")
            try:
                minimum = float(value)
            except ValueError:
                fail(f"--min needs a ratio such as 4.5, not {value!r}\n{USAGE}")
            if not (math.isfinite(minimum) and 1 <= minimum <= RATIO_MAX):
                fail(f"--min {value}: a contrast ratio lies between 1 and {RATIO_MAX:g}")
            continue
        colours.append(arg)
        i += 1
    if len(colours) != 2:
        fail(USAGE)
    return colours, minimum


def luminance(hex_colour: str) -> float:
    h = hex_colour.lstrip("#")
    if len(h) in (3, 4):
        h = "".join(c * 2 for c in h[:3])
    r, g, b = (int(h[i : i + 2], 16) / 255 for i in (0, 2, 4))

    def channel(c: float) -> float:  # sRGB to linear, WCAG 2.2 definition of relative luminance
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)


def ratio(a: str, b: str) -> float:
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def main() -> None:
    args = sys.argv[1:]
    if args and args[0] in ("-h", "--help"):
        print(__doc__)
        return
    table = tokens()

    if args:
        (fg, bg), minimum = parse_args(args)
        r = ratio(resolve(fg, table), resolve(bg, table))
        verdict = "PASS" if r >= minimum else "FAIL"
        print(f"{fg} on {bg}: {r:.2f}:1 (minimum {minimum:g}:1) {verdict}")
        if fg.startswith("#") or bg.startswith("#"):
            print("Note: components never use literal colours; this ratio is for choosing or checking a token value.")
        sys.exit(0 if r >= minimum else 1)

    rows: list[tuple[str, str, float]] = []
    for fg in TEXT_TOKENS:
        for bg in SURFACES:
            rows.append((fg, bg, TEXT_MIN))
    for fg in OUTLINE_TOKENS:
        for bg in SURFACES:
            rows.append((fg, bg, OUTLINE_MIN))
    for fill in FILL_TOKENS:
        rows.append(("--surface-0", fill, TEXT_MIN))

    failed = 0
    print(f"Tokens from {stylesheet()}\n")
    print(f"{'foreground':<20}{'background':<20}{'ratio':>8}  {'min':>4}  verdict")
    for fg, bg, minimum in rows:
        missing = [t for t in (fg, bg) if t not in table]
        unscored = [t for t in (fg, bg) if t in table and not HEX.match(table[t])]
        if missing or unscored:
            verdict = "MISSING TOKEN" if missing else "; ".join(f"NOT A HEX VALUE ({t}: {table[t]})" for t in unscored)
            print(f"{fg:<20}{bg:<20}{'—':>8}  {minimum:>4g}  {verdict}")
            failed += 1
            continue
        r = ratio(table[fg], table[bg])
        ok = r >= minimum
        failed += not ok
        print(f"{fg:<20}{bg:<20}{r:>7.2f}:1 {minimum:>4g}  {'PASS' if ok else 'FAIL'}")
    colours = {k for k, v in table.items() if HEX.match(v) or COLOUR_FUNCTION.match(v)}
    extra = sorted(colours - set(TEXT_TOKENS) - set(OUTLINE_TOKENS) - set(SURFACES) - {"--line"})
    if extra:
        print(f"\nTokens with no rule here (check their pairings by hand): {', '.join(extra)}")
    print(f"\n{failed} failing pairing(s)." if failed else "\nAll required pairings pass.")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
