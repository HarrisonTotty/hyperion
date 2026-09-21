#!/usr/bin/env python3
"""WCAG contrast of HYPERION's colour tokens, against the UX guide's thresholds.

Usage:
    contrast.py                         Every pairing the guide requires, PASS/FAIL.
    contrast.py FG BG [--min RATIO]     One pairing; FG and BG are tokens (`--text` or `text`)
                                        or hex colours (#rrggbb). Default minimum 6.

Tokens are read from the `:root` block of apps/hyperion/src/renderer/src/styles.css, so the check
follows the stylesheet. The guide (Colour): text and the meaning-carrying parts of symbols reach
6:1 against their surface; control outlines 3:1; solid status fills carry `--surface-0` text.
Decorative hairlines (`--line`) have no minimum. Exit status is 1 when a required pairing fails.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

TEXT_MIN = 6.0  # NASA-STD-3001 Vol. 2 App. F minimum, adopted by the guide for text and symbols
OUTLINE_MIN = 3.0  # WCAG 2.2 SC 1.4.11 non-text contrast, adopted by the guide for control outlines
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


def stylesheet() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        candidate = parent / "apps" / "hyperion" / "src" / "renderer" / "src" / "styles.css"
        if candidate.exists():
            return candidate
    sys.exit("contrast.py: apps/hyperion/src/renderer/src/styles.css not found")


def tokens() -> dict[str, str]:
    css = stylesheet().read_text(encoding="utf-8")
    root = re.search(r":root\s*\{(.*?)\}", css, re.S)
    if root is None:
        sys.exit("contrast.py: no :root block in styles.css")
    return {m.group(1): m.group(2).lower() for m in re.finditer(r"(--[\w-]+)\s*:\s*(#[0-9a-fA-F]{3,8})\b", root.group(1))}


def resolve(name: str, table: dict[str, str]) -> str:
    if name.startswith("#"):
        return name
    key = name if name.startswith("--") else f"--{name}"
    if key not in table:
        sys.exit(f"contrast.py: unknown token {name}; known: {', '.join(sorted(table))}")
    return table[key]


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
        minimum = TEXT_MIN
        if "--min" in args:
            i = args.index("--min")
            minimum = float(args[i + 1])
            args = args[:i] + args[i + 2 :]
        if len(args) != 2:
            sys.exit("usage: contrast.py FG BG [--min RATIO]")
        r = ratio(resolve(args[0], table), resolve(args[1], table))
        verdict = "PASS" if r >= minimum else "FAIL"
        print(f"{args[0]} on {args[1]}: {r:.2f}:1 (minimum {minimum:g}:1) {verdict}")
        sys.exit(0 if r >= minimum else 1)

    rows: list[tuple[str, str, float, float]] = []
    for fg in TEXT_TOKENS:
        for bg in SURFACES:
            rows.append((fg, bg, TEXT_MIN, 0.0))
    for fg in OUTLINE_TOKENS:
        for bg in SURFACES:
            rows.append((fg, bg, OUTLINE_MIN, 0.0))
    for fill in FILL_TOKENS:
        rows.append(("--surface-0", fill, TEXT_MIN, 0.0))

    failed = 0
    print(f"Tokens from {stylesheet()}\n")
    print(f"{'foreground':<20}{'background':<20}{'ratio':>8}  {'min':>4}  verdict")
    for fg, bg, minimum, _ in rows:
        if fg not in table or bg not in table:
            print(f"{fg:<20}{bg:<20}{'—':>8}  {minimum:>4g}  MISSING TOKEN")
            failed += 1
            continue
        r = ratio(table[fg], table[bg])
        ok = r >= minimum
        failed += not ok
        print(f"{fg:<20}{bg:<20}{r:>7.2f}:1 {minimum:>4g}  {'PASS' if ok else 'FAIL'}")
    extra = sorted(set(table) - set(TEXT_TOKENS) - set(OUTLINE_TOKENS) - set(SURFACES) - {"--line"})
    if extra:
        print(f"\nTokens with no rule here (check their pairings by hand): {', '.join(extra)}")
    print(f"\n{failed} failing pairing(s)." if failed else "\nAll required pairings pass.")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
