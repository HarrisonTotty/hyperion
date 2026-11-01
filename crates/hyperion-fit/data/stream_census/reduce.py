#!/usr/bin/env python3
"""Reduce the galstreams library to `census.csv`, the known streams P15.T11.b counts (plan 15).

Reads galstreams v1.2.1 (Mateu 2023, MNRAS 520, 5225; https://github.com/cmateu/galstreams, the
tag `v1.2.1`) unpacked under `crates/hyperion-fit/data/cache/stream_census/galstreams-1.2.1/`: its
`galstreams/lib/master_log.txt` for the stream tracks it switches on (`On` = 1, one track per
distinct stream, 141), and from `galstreams/tracks/` each track's summary (the mid-point's
distance, the width in phi2) and its points (ra, dec, heliocentric distance).

For each stream it writes its name, its track and the track's reference, the mid-point's distance
(kpc; empty where the track's distances are galstreams' 1 kpc placeholder, the STREAMFINDER tracks
of Ibata et al. 2024), the width (deg, the master log's or the summary's where the log gives -1),
the width in parsecs at the mid-point's distance (empty where either is unknown), whether it is
globular-like, its kind and why, and whether it lies inside the fit's cut:

- **kind**, in this order: `globular` for a named globular progenitor (GC below); `dwarf` for a
  dwarf-galaxy progenitor, named or shown by a spread in metallicity (DWARF); `disc` for a structure
  of the disc (DISC); `wide` for a stream wider than 300 pc at its mid-point's distance with none of
  those, the thin streams' limit of Bonaca and Price-Whelan's census (2024, arXiv:2405.19410, Table
  A.1, as plan 10's records cite it); and `globular` for every other, thin or of unknown width: a
  thin stream with no known dwarf progenitor is taken for a dissolved globular. Mass does not split
  them: a dwarf's stream may be as light as a globular's.
- **in_cut**: some point of the track lies within 30 kpc of the Sun and more than 15 degrees from
  the Galactic plane, the fit's cut (`manifests/streams.toml`). A track whose distances are the
  placeholder is in the cut by its latitude alone: STREAMFINDER's Gaia DR3 streams lie well within
  30 kpc. No mass is cut, since galstreams gives none.

Latitudes are from the ICRS to Galactic rotation of the Hipparcos catalogue (ESA 1997, vol. 1,
§1.5.3, eq. 1.5.11).
"""
import csv
import math
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
SOURCE = HERE.parent / "cache" / "stream_census" / "galstreams-1.2.1" / "galstreams"
OUT = HERE / "census.csv"

MAX_DISTANCE_KPC = 30.0
MIN_LATITUDE_DEG = 15.0
WIDE_PC = 300.0

# ICRS to Galactic (ESA 1997, eq. 1.5.11).
A_G = [
    [-0.0548755604162154, -0.8734370902348850, -0.4838350155487132],
    [0.4941094278755837, -0.4448296299600112, 0.7469822444972189],
    [-0.8676661490190047, -0.1980763734312015, 0.4559837761750669],
]

GC = {
    "C-19": "a dissolved globular below the metallicity floor (Martin et al. 2022)",
    "Eridanus": "the Eridanus globular (Myeong et al. 2017)",
    "M2": "NGC 7089 (galstreams)",
    "M3-Svol": "NGC 5272 (galstreams: Svol)",
    "M30": "NGC 7099 (galstreams)",
    "M5": "NGC 5904 (galstreams)",
    "M68-Fjorm": "NGC 4590 (galstreams)",
    "M92": "NGC 6341 (galstreams)",
    "NGC1261": "NGC 1261",
    "NGC1851": "NGC 1851",
    "NGC2298": "NGC 2298",
    "NGC288": "NGC 288",
    "NGC3201-Gjoll": "NGC 3201 (galstreams: Gjoll)",
    "NGC5053": "NGC 5053",
    "NGC5466": "NGC 5466",
    "NGC6362": "NGC 6362",
    "NGC6397": "NGC 6397",
    "NGC7492": "NGC 7492",
    "OmegaCen-Fimbulthul": "omega Cen or NGC 5139 (galstreams: Fimbulthul)",
    "Pal13": "Palomar 13",
    "Pal15": "Palomar 15",
    "Pal5": "Palomar 5",
    "Phoenix": "a dissolved metal-poor globular (Wan et al. 2020)",
    "Svol": "NGC 5272 (galstreams: M3)",
}
DWARF = {
    "Cetus": "the Cetus dwarf's stream (Newberg et al. 2009; Yuan et al. 2021)",
    "Cetus-New": "the Cetus dwarf's stream (Yuan et al. 2021)",
    "Cetus-Palca": "the Cetus dwarf's stream (Yuan et al. 2021)",
    "Elqui": "a dwarf by its spread in metallicity (Ji et al. 2020)",
    "Indus": "a dwarf by its spread in metallicity (Ji et al. 2020; Hansen et al. 2021)",
    "Jhelum-a": "a dwarf by its spread in metallicity (Ji et al. 2020)",
    "Jhelum-b": "a dwarf by its spread in metallicity (Ji et al. 2020)",
    "LMS-1": "a dwarf (Yuan et al. 2020; Malhan et al. 2021)",
    "Orphan-Chenab": "a dwarf by its spread in metallicity (Ji et al. 2020; Koposov et al. 2019)",
    "Palca": "part of the Cetus dwarf's stream (Yuan et al. 2021)",
    "Sagittarius": "the Sagittarius dwarf (Ibata et al. 1994)",
    "Styx": "the Bootes III dwarf (Grillmair 2009)",
    "TucanaIII": "the Tucana III ultra-faint dwarf (Drlica-Wagner et al. 2015; Li et al. 2018)",
}
DISC = {
    "ACS": "a disc structure of the anticentre (Ramos et al. 2021)",
    "Monoceros": "a disc structure of the anticentre (Ramos et al. 2021)",
}


def latitude_deg(ra: float, dec: float) -> float:
    ra, dec = math.radians(ra), math.radians(dec)
    u = [math.cos(dec) * math.cos(ra), math.cos(dec) * math.sin(ra), math.sin(dec)]
    sin_b = sum(A_G[2][j] * u[j] for j in range(3))
    return math.degrees(math.asin(max(-1.0, min(1.0, sin_b))))


def ecsv_rows(path: pathlib.Path) -> list:
    with path.open(encoding="utf-8") as f:
        return list(csv.DictReader([x for x in f if not x.startswith("#")]))


def main() -> None:
    rows = []
    with (SOURCE / "lib" / "master_log.txt").open(encoding="utf-8") as f:
        header = f.readline().lstrip("#").split()
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            entry = dict(zip(header, line.split()))
            if entry["On"] == "1":
                rows.append(entry)
    out = []
    for entry in rows:
        name, ref = entry["Name"], entry["TrackRefs"]
        base = f"track.{entry['Imp']}.{name}.{ref}"
        summary = ecsv_rows(SOURCE / "tracks" / f"{base}.summary.ecsv")[0]
        points = ecsv_rows(SOURCE / "tracks" / f"{base}.ecsv")
        width = float(entry["width_phi2"])
        if width < 0:
            width = float(summary["width_phi2"])
        distances = [float(p["distance"]) for p in points]
        placeholder = all(abs(d - 1.0) < 1e-9 for d in distances)
        mid = float(summary["mid.distance"])
        width_pc = None
        if width > 0.002 and not placeholder:
            width_pc = math.radians(width) * mid * 1000.0
        latitudes = [abs(latitude_deg(float(p["ra"]), float(p["dec"]))) for p in points]
        if placeholder:
            in_cut = any(b > MIN_LATITUDE_DEG for b in latitudes)
        else:
            in_cut = any(
                d <= MAX_DISTANCE_KPC and b > MIN_LATITUDE_DEG
                for d, b in zip(distances, latitudes)
            )
        if name in GC:
            kind, reason = "globular", "progenitor " + GC[name]
        elif name in DWARF:
            kind, reason = "dwarf", DWARF[name]
        elif name in DISC:
            kind, reason = "disc", DISC[name]
        elif width_pc is not None and width_pc > WIDE_PC:
            kind, reason = "wide", f"{width_pc:.0f} pc wide with no progenitor known"
        elif width_pc is None:
            kind, reason = "globular", "no dwarf progenitor known; width unknown"
        else:
            kind, reason = "globular", f"thin at {width_pc:.0f} pc with no dwarf progenitor known"
        out.append(
            {
                "name": name,
                "track": entry["TrackName"],
                "ref": ref,
                "distance_mid_kpc": "" if placeholder else f"{mid:.2f}",
                "width_deg": f"{width:.3f}",
                "width_pc": "" if width_pc is None else f"{width_pc:.0f}",
                "globular_like": "yes" if kind == "globular" else "no",
                "kind": kind,
                "reason": reason,
                "in_cut": "yes" if in_cut else "no",
            }
        )
    for row in out:
        for value in row.values():
            if "," in value:
                sys.exit(f"a comma in {row['name']}: {value!r}")
    out.sort(key=lambda r: r["name"].lower())
    with OUT.open("w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=list(out[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(out)
    globular = [r for r in out if r["globular_like"] == "yes"]
    print(
        f"{len(out)} streams, {len(globular)} globular-like, "
        f"{sum(r['in_cut'] == 'yes' for r in globular)} of them inside the cut"
    )


if __name__ == "__main__":
    main()
