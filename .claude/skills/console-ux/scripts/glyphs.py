#!/usr/bin/env python3
"""Check which characters the bundled B612 and B612 Mono fonts can draw.

Usage:
    glyphs.py "☉ ° µ ↑"        Report each non-space character: in B612? in B612 Mono?
    glyphs.py --ranges          Print every code point range each font covers.

Reads the regular-weight WOFF files that @fontsource bundles (apps/hyperion/node_modules), so
the answer is what the client actually ships, not what the full upstream font contains. A
character missing here falls back to a system font that will not match (UX guide, Typography):
draw it as an SVG instead. Exit status is 1 when a checked character is missing from either
font. Uses only the Python standard library.
"""

from __future__ import annotations

import struct
import sys
import unicodedata
import zlib
from functools import cache
from pathlib import Path

FONTS = {"B612": "b612", "B612 Mono": "b612-mono"}


def repo_root() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "apps" / "hyperion").is_dir():
            return parent
    return here.parents[4]


def font_files(package: str) -> list[Path]:
    """Every regular-weight upright WOFF of the package: one per unicode-range subset."""
    root = repo_root()
    candidates = [
        root / "apps" / "hyperion" / "node_modules" / "@fontsource" / package / "files",
        root / "node_modules" / "@fontsource" / package / "files",
    ]
    for directory in candidates:
        files = sorted(directory.glob(f"{package}-*-400-normal.woff")) if directory.is_dir() else []
        if files:
            return files
    # pnpm keeps the real files under node_modules/.pnpm/@fontsource+<package>@<version>/…
    store = root / "node_modules" / ".pnpm"
    return sorted(store.glob(f"@fontsource+{package}@*/node_modules/@fontsource/{package}/files/{package}-*-400-normal.woff"))


def woff_table(data: bytes, tag: bytes) -> bytes | None:
    if data[:4] != b"wOFF":
        raise ValueError("not a WOFF 1 file")
    (num_tables,) = struct.unpack_from(">H", data, 12)
    for i in range(num_tables):
        entry_tag, offset, comp_len, orig_len, _ = struct.unpack_from(">4sIIII", data, 44 + 20 * i)
        if entry_tag == tag:
            raw = data[offset : offset + comp_len]
            return zlib.decompress(raw) if comp_len < orig_len else raw
    return None


def cmap_codepoints(cmap: bytes) -> set[int]:
    """Code points mapped to a real glyph, from the best Unicode subtable (format 12 or 4)."""
    _, count = struct.unpack_from(">HH", cmap, 0)
    subtables = {}
    for i in range(count):
        platform, encoding, offset = struct.unpack_from(">HHI", cmap, 4 + 8 * i)
        (fmt,) = struct.unpack_from(">H", cmap, offset)
        subtables[(platform, encoding, fmt)] = offset
    for key in ((3, 10, 12), (0, 4, 12), (0, 6, 12)):
        if key in subtables:
            return format12(cmap, subtables[key])
    for key in ((3, 1, 4), (0, 3, 4), (0, 1, 4), (0, 0, 4)):
        if key in subtables:
            return format4(cmap, subtables[key])
    raise ValueError(f"no Unicode cmap subtable among {sorted(subtables)}")


def format4(cmap: bytes, base: int) -> set[int]:
    (seg_x2,) = struct.unpack_from(">H", cmap, base + 6)
    seg = seg_x2 // 2
    ends = struct.unpack_from(f">{seg}H", cmap, base + 14)
    starts = struct.unpack_from(f">{seg}H", cmap, base + 16 + seg_x2)
    deltas = struct.unpack_from(f">{seg}h", cmap, base + 16 + 2 * seg_x2)
    range_base = base + 16 + 3 * seg_x2
    offsets = struct.unpack_from(f">{seg}H", cmap, range_base)
    points = set()
    for i in range(seg):
        for c in range(starts[i], ends[i] + 1):
            if c == 0xFFFF:
                continue
            if offsets[i] == 0:
                glyph = (c + deltas[i]) & 0xFFFF
            else:
                at = range_base + 2 * i + offsets[i] + 2 * (c - starts[i])
                (glyph,) = struct.unpack_from(">H", cmap, at)
                if glyph:
                    glyph = (glyph + deltas[i]) & 0xFFFF
            if glyph:
                points.add(c)
    return points


def format12(cmap: bytes, base: int) -> set[int]:
    (groups,) = struct.unpack_from(">I", cmap, base + 12)
    points = set()
    for i in range(groups):
        start, end, glyph = struct.unpack_from(">III", cmap, base + 16 + 12 * i)
        points.update(c for c in range(start, end + 1) if glyph + (c - start))
    return points


@cache
def coverage(font: str) -> frozenset[int]:
    package = FONTS[font]
    files = font_files(package)
    if not files:
        raise FileNotFoundError(f"no @fontsource/{package} WOFF files; run `pnpm install`")
    points: set[int] = set()
    for path in files:
        table = woff_table(path.read_bytes(), b"cmap")
        if table is not None:
            points |= cmap_codepoints(table)
    return frozenset(points)


def missing(char: str) -> list[str]:
    """Fonts that cannot draw the character (empty when both can)."""
    return [font for font in FONTS if ord(char) not in coverage(font)]


def ranges(points: frozenset[int]) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for c in sorted(points):
        if out and c == out[-1][1] + 1:
            out[-1] = (out[-1][0], c)
        else:
            out.append((c, c))
    return out


def main() -> None:
    args = sys.argv[1:]
    try:
        if args == ["--ranges"]:
            for font in FONTS:
                spans = ", ".join(f"U+{a:04X}" if a == b else f"U+{a:04X}–{b:04X}" for a, b in ranges(coverage(font)))
                print(f"{font}: {spans}\n")
            return
        if not args or args[0] in ("-h", "--help"):
            print(__doc__)
            return
        chars = [c for c in dict.fromkeys("".join(args)) if not c.isspace()]
        bad = False
        print(f"{'char':<6}{'code':<9}{'B612':<7}{'Mono':<7}name")
        for c in chars:
            gone = missing(c)
            bad = bad or bool(gone)
            marks = ["no" if f in gone else "yes" for f in FONTS]
            print(f"{c:<6}U+{ord(c):04X}   {marks[0]:<7}{marks[1]:<7}{unicodedata.name(c, '?')}")
        if bad:
            print("\nMissing glyphs fall back to a system font. Draw them as an inline SVG (UX guide, Typography).")
            sys.exit(1)
    except (FileNotFoundError, ValueError) as err:
        sys.exit(f"glyphs.py: {err}")


if __name__ == "__main__":
    main()
