#!/usr/bin/env python3
"""Reduce the CMC Cluster Catalog's model table to `models.csv` (plan 15, P15.T8.a).

Reads Table A1 of Kremer et al. (2020, ApJS 247, 48; "Initial cluster parameters and various
populations at end of simulation for all model GCs") from `main.tex` in the paper's arXiv source,
`crates/hyperion-fit/data/cache/cmc/1911.00018.tar.gz` (https://arxiv.org/e-print/1911.00018,
version 2), and writes one row per model that reached the end of the simulation, 14 Gyr: its name;
initial N (10⁵), virial radius (pc), galactocentric distance (kpc) and metallicity (Z☉); and at 14
Gyr its mass (10⁵ M☉), core radius (pc, the paper's theoretical r_c), half-light radius (pc), and
its numbers of neutron stars and black holes. Disrupted models and those stopped at a collisional
runaway (marked with a dagger) have no final state and are left out.
"""
import pathlib
import re
import tarfile

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "cmc" / "1911.00018.tar.gz"


def main() -> None:
    with tarfile.open(SOURCE) as tar:
        tex = tar.extractfile("main.tex").read().decode("utf-8")
    start = tex.index(r"\label{table:models}")
    body = tex[tex.index(r"\startdata", start) : tex.index(r"\enddata", start)]
    rows = []
    for line in body.splitlines():
        m = re.match(r"\s*\d+\s*&\s*\\textsc\{([^}]*)\}(\$\^\\dagger\$)?\s*&(.*)\\\\", line)
        if not m or m.group(2):
            continue
        cells = [c.strip() for c in m.group(3).split("&")]
        if len(cells) != 14 or "disrupted" in m.group(3):
            continue
        n, rv, rgc, z = cells[0:4]
        mass, rc, rh, _sigma, _ms, _g, _wd, ns, bh, _final = cells[4:14]
        rows.append((m.group(1), n, rv, rgc, z, mass, rc, rh, ns, bh))
    lines = ["name,n_1e5,rv_pc,rgc_kpc,z_solar,mass_1e5,rc_pc,rh_pc,neutron_stars,black_holes"]
    lines += [",".join(r) for r in rows]
    (HERE / "models.csv").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
