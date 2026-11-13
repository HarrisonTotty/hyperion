#!/usr/bin/env python3
"""Reduce Gerakines, Materese and Hudson's (2023) crystalline CO ice to `nk_25k.csv` (plan 14,
P14.T49.i).

Reads `all.csv`, NASA's Optical Constants Database export of their dataset 2 (alpha-CO deposited
and measured at 25 K; tab-separated UTF-16 text, header lines, then rows of wavenumber (cm^-1), n,
wavenumber again and k, every 0.06 cm^-1 over 1500-3000 cm^-1), from
`crates/hyperion-fit/data/cache/gerakines_2023_co/` or the path given as the first argument, and
writes beside this file, or to the path given as the second argument, the rows from 1990 cm^-1
(5.03 um) to the table's end, averaged over consecutive blocks of 8 rows (0.48 cm^-1, the
measurement's stated resolution of 0.5 cm^-1): per block the mean wavenumber (cm^-1), n and k, a
trailing block of fewer than 8 rows dropped. Negative k (noise about zero between bands) is kept
as measured.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "gerakines_2023_co" / "all.csv"
OUT = HERE / "nk_25k.csv"
FROM_PER_CM = 1990.0
BLOCK = 8


def main(argv: list[str]) -> None:
    source = pathlib.Path(argv[1]) if len(argv) > 1 else SOURCE
    out = pathlib.Path(argv[2]) if len(argv) > 2 else OUT
    text = source.read_bytes().decode("utf-16")
    if 'Temperature:\t"25K"' not in text or "Crystalline" not in text:
        raise ValueError("the export is not the 25 K crystalline dataset")
    rows = []
    for line in text.splitlines():
        cells = [c.strip() for c in line.split("\t")]
        if len(cells) < 5:
            continue
        try:
            nu, n, nu_k, k = float(cells[0]), float(cells[1]), float(cells[3]), float(cells[4])
        except ValueError:
            continue
        if nu != nu_k:
            raise ValueError(f"n's and k's wavenumbers differ at {cells[0]}")
        rows.append((nu, n, k))
    if any(b[0] <= a[0] for a, b in zip(rows, rows[1:])):
        raise ValueError("the wavenumbers do not rise strictly")
    rows = [r for r in rows if r[0] >= FROM_PER_CM]
    lines = ["wavenumber_per_cm,n,k"]
    for start in range(0, len(rows) - BLOCK + 1, BLOCK):
        block = rows[start : start + BLOCK]
        mean = [sum(r[i] for r in block) / BLOCK for i in range(3)]
        lines.append(f"{mean[0]:.4f},{mean[1]:.6g},{mean[2]:.6g}")
    out.write_text("\n".join(lines) + "\n", encoding="ascii")


if __name__ == "__main__":
    main(sys.argv)
