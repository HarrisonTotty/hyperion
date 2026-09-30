#!/usr/bin/env python3
"""Reduce Freire's table of pulsars in globular clusters to `counts.csv` (plan 15, P15.T8.c).

Reads `crates/hyperion-fit/data/cache/gc_pulsars/GCpsr.html` (fetched from
https://www3.mpifr-bonn.mpg.de/staff/pfreire/GCpsr.html) and writes, per cluster with a known
pulsar, its key (the NGC number or the name, lower case without spaces or hyphens, as the other
cluster datasets are keyed), the page's heliocentric distance (kpc) and its pulsar count.
"""
import html
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "gc_pulsars" / "GCpsr.html"


def key(name: str) -> str:
    inner = re.search(r"\((NGC [0-9]+)\)", name)
    base = inner.group(1) if inner else name
    return re.sub(r"[\s\-]", "", base).lower()


def main() -> None:
    text = SOURCE.read_text(encoding="utf-8", errors="replace")
    start = text.find("Globular cluster table")
    table = text[start : text.find("</table>", start)]
    rows = []
    for row in re.findall(r"<tr[^>]*>(.*?)</tr>", table, re.S):
        cells = [
            html.unescape(re.sub(r"<[^>]*>", "", c)).strip()
            for c in re.findall(r"<t[dh][^>]*>(.*?)</t[dh]>", row, re.S)
        ]
        if len(cells) < 7 or not re.fullmatch(r"[0-9]+", cells[6]):
            continue
        rows.append((key(cells[0]), float(cells[5]), int(cells[6])))
    rows.sort()
    lines = ["key,distance_kpc,pulsars"] + [f"{k},{d},{n}" for k, d, n in rows]
    (HERE / "counts.csv").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
