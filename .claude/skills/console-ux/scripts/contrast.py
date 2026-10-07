#!/usr/bin/env python3
"""WCAG contrast of HYPERION's colour tokens, against the UX guide's thresholds.

Usage:
    contrast.py                         Every pairing the guide requires, PASS/FAIL.
    contrast.py FG BG [--min RATIO] [--coverage C] [--blend srgb|linear]
                                        One pairing; FG and BG are tokens (`--text` or `text`)
                                        or hex colours (#rgb, #rrggbb, or either with an alpha
                                        digit pair, which is ignored). Default minimum 6.

Tokens are read from the `:root` block of apps/hyperion/src/renderer/src/styles.css, so the check
follows the stylesheet. The guide (Colour): text and the meaning-carrying parts of symbols reach
6:1 against their surface; control outlines 3:1; solid status fills carry `--surface-0` text.
Decorative hairlines (`--line`) have no minimum. Only hex values can be scored: a required token
with any other value (rgb(), a name) is reported as not a hex value. Exit status is 1 when a
required pairing fails or cannot be scored, and 2 on a usage error (a malformed colour, an
unknown token, a bad --min, --coverage or --blend) or when styles.css or its `:root` block is
missing.

A stroke as drawn (decision-thin-line-contrast, items 1 and 4): an antialiased line's brightest
pixel holds only part of its colour where the line falls between pixels, so a line or an outline
on a canvas, in an SVG or in a view scores its pair's ratio, to within 1%, only at 2 device pixels
or more, under exact area coverage (the view's ramp meets it). Chromium's 2D canvas, as the smoke
page's spatial check reads it, peaks at about 15/16 for a 2 px line at 45° at its worst phase:
`text-muted surface-0 --coverage 0.9375` is that line, 6.43:1 against its pair's 7.22:1.
`--coverage C` (0 < C <= 1) blends FG over BG at that coverage before scoring, as such a
pixel holds it, and `--blend` names the space the blend is taken in: `srgb`, the default, blends
the sRGB-encoded values, as Canvas 2D, SVG and the DOM do; `linear` blends linear light, as a view
does through its canvas's sRGB view. For example, `text-muted surface-0 --coverage 0.5 --blend
linear` is a 1 device px `--text-muted` line at its worst in a view (4.11:1), and `--coverage 0.39`
a 0.78 device px one on a canvas (1.97:1).
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
USAGE = "usage: contrast.py [FG BG [--min RATIO] [--coverage C] [--blend srgb|linear]]"
BLENDS = ("srgb", "linear")


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


def option_value(args: list[str], i: int, name: str, needs: str) -> tuple[str, int]:
    """The value of the option `name` at args[i], given as `--name=V` or `--name V`, and the next index."""
    arg = args[i]
    if "=" in arg:
        return arg.partition("=")[2], i + 1
    if i + 1 < len(args):
        return args[i + 1], i + 2
    fail(f"{name} needs {needs}\n{USAGE}")


def is_option(arg: str, name: str) -> bool:
    return arg == name or arg.startswith(f"{name}=")


def parse_args(args: list[str]) -> tuple[list[str], float, float, str]:
    """The two colours, the minimum ratio, the coverage and the blend. `--text` is a token, not an option."""
    colours: list[str] = []
    minimum = TEXT_MIN
    coverage = 1.0
    blend = "srgb"
    i = 0
    while i < len(args):
        arg = args[i]
        if is_option(arg, "--min"):
            value, i = option_value(args, i, "--min", "a ratio")
            try:
                minimum = float(value)
            except ValueError:
                fail(f"--min needs a ratio such as 4.5, not {value!r}\n{USAGE}")
            if not (math.isfinite(minimum) and 1 <= minimum <= RATIO_MAX):
                fail(f"--min {value}: a contrast ratio lies between 1 and {RATIO_MAX:g}")
            continue
        if is_option(arg, "--coverage"):
            value, i = option_value(args, i, "--coverage", "a coverage")
            try:
                coverage = float(value)
            except ValueError:
                fail(f"--coverage needs a coverage such as 0.5, not {value!r}\n{USAGE}")
            if not (math.isfinite(coverage) and 0 < coverage <= 1):
                fail(f"--coverage {value}: a coverage lies above 0 and at most 1")
            continue
        if is_option(arg, "--blend"):
            blend, i = option_value(args, i, "--blend", "srgb or linear")
            if blend not in BLENDS:
                fail(f"--blend {blend}: srgb or linear\n{USAGE}")
            continue
        colours.append(arg)
        i += 1
    if len(colours) != 2:
        fail(USAGE)
    return colours, minimum, coverage, blend


def encoded(hex_colour: str) -> tuple[float, float, float]:
    """A hex colour's sRGB-encoded channels, each 0 to 1."""
    h = hex_colour.lstrip("#")
    if len(h) in (3, 4):
        h = "".join(c * 2 for c in h[:3])
    r, g, b = (int(h[i : i + 2], 16) / 255 for i in (0, 2, 4))
    return r, g, b


def channel(c: float) -> float:
    """An sRGB-encoded channel in linear light, by WCAG 2.2's definition of relative luminance."""
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def relative(r: float, g: float, b: float) -> float:
    """WCAG 2.2's relative luminance of linear-light channels."""
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def luminance(hex_colour: str) -> float:
    r, g, b = encoded(hex_colour)
    return relative(channel(r), channel(g), channel(b))


def blended_luminance(fg: str, bg: str, coverage: float, blend: str) -> float:
    """The luminance of a pixel `coverage` covered by FG over BG, blended in `blend`'s space."""
    f, b = encoded(fg), encoded(bg)
    if blend == "linear":
        mixed = [coverage * channel(x) + (1 - coverage) * channel(y) for x, y in zip(f, b)]
    else:
        mixed = [channel(coverage * x + (1 - coverage) * y) for x, y in zip(f, b)]
    return relative(mixed[0], mixed[1], mixed[2])


def contrast(la: float, lb: float) -> float:
    high, low = max(la, lb), min(la, lb)
    return (high + 0.05) / (low + 0.05)


def ratio(a: str, b: str) -> float:
    return contrast(luminance(a), luminance(b))


def main() -> None:
    args = sys.argv[1:]
    if args and args[0] in ("-h", "--help"):
        print(__doc__)
        return
    table = tokens()

    if args:
        (fg, bg), minimum, coverage, blend = parse_args(args)
        fg_hex, bg_hex = resolve(fg, table), resolve(bg, table)
        r = contrast(blended_luminance(fg_hex, bg_hex, coverage, blend), luminance(bg_hex))
        verdict = "PASS" if r >= minimum else "FAIL"
        drawn = "" if coverage == 1 else f" at coverage {coverage:g}, blended in {blend}"
        # Within 1% of the minimum, rounding can carry a failure up to the minimum itself, so the
        # ratio is cut, not rounded, to three decimals there: a failure never prints as the minimum.
        near = abs(r - minimum) < 0.01 * minimum
        shown = f"{math.floor(r * 1000) / 1000:.3f}" if near else f"{r:.2f}"
        print(f"{fg} on {bg}{drawn}: {shown}:1 (minimum {minimum:g}:1) {verdict}")
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
