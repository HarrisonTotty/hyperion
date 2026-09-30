#!/usr/bin/env python3
"""Reduce Bahramian et al.'s (2013) encounter rates to `gamma.csv` (plan 15, P15.T8.c).

Reads `crates/hyperion-fit/data/cache/bahramian_2013/J_ApJ_766_136.tsv`, VizieR's tab-separated
export of catalogue J/ApJ/766/136 (all columns), and writes per cluster its key (the name, lower
case without spaces, hyphens or underscores), heliocentric distance (kpc, empty where the
catalogue gives none), Γ normalised to 47 Tucanae's 1,000 with its lower and upper bounds, which of
the paper's estimates it is (`gamma`, their adopted one; or where it has none, `gamma2` from
ρ² r³ ÷ σ, else `gamma3` from ρ^1.5 r²), and Harris's core-collapse flag (1 for `c` or `c:`).
"""
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "bahramian_2013" / "J_ApJ_766_136.tsv"


def main() -> None:
    lines = [l for l in SOURCE.read_text(encoding="utf-8").splitlines() if l and not l.startswith("#")]
    header = [h.strip() for h in lines[0].split("\t")]
    col = {h: i for i, h in enumerate(header)}
    rows = []
    for line in lines[3:]:
        cells = [c.strip() for c in line.split("\t")]
        name = re.sub(r"[\s\-_]", "", cells[col["Name"]]).lower()
        cc = 1 if cells[col["CC"]].startswith("c") else 0
        for estimate, (g, lo, hi) in (
            ("gamma", ("Gamma", "e_Gamma", "E_Gamma")),
            ("gamma2", ("Gamma2", "e_Gamma2", "E_Gamma2")),
            ("gamma3", ("Gamma3", "e_Gamma3", "E_Gamma3")),
        ):
            if cells[col[g]]:
                break
        dist = cells[col["Dist"]]
        rows.append(
            (
                name,
                dist,
                float(cells[col[g]]),
                float(cells[col[lo]]),
                float(cells[col[hi]]),
                estimate,
                cc,
            )
        )
    rows.sort()
    out = ["key,distance_kpc,gamma,gamma_lower,gamma_upper,estimate,core_collapsed"]
    out += [f"{k},{d},{g},{lo},{hi},{e},{cc}" for k, d, g, lo, hi, e, cc in rows]
    (HERE / "gamma.csv").write_text("\n".join(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
